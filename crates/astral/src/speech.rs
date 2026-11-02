//! Offline speech recognition, timed visemes and user-selectable mouth mappings.
use crate::service::{Request, Service};
use anyhow::{Context, Result, ensure};
use astral_core::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::{Command as Process, Stdio},
    time::{Duration, Instant},
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Recognizer {
    #[default]
    Envelope,
    English,
    Phonetic,
}
pub fn executable() -> PathBuf {
    if let Some(path) = std::env::var_os("ASTRAL_RHUBARB") {
        return path.into();
    }
    let name = if cfg!(windows) {
        "rhubarb.exe"
    } else {
        "rhubarb"
    };
    let root = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default();
    let bundled = root.join("resources/rhubarb").join(name);
    if bundled.is_file() {
        return bundled;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../build/rhubarb")
        .join(name)
}
#[allow(clippy::too_many_arguments)]
pub fn recognize(
    service: &Service,
    project: Id,
    revision: u64,
    clip: Id,
    character: Id,
    text: String,
    engine: Recognizer,
    mapping: BTreeMap<String, MouthShape>,
) -> Result<Value> {
    let p = service.checked_project(project, revision)?;
    ensure!(p.character(character).is_some(), "Character missing");
    let clip = p
        .audio
        .iter()
        .find(|a| a.id == clip)
        .context("Audio clip missing")?
        .clone();
    ensure!(
        clip.duration_seconds <= 120. && text.len() <= 16384,
        "Recognize at most two minutes and 16 KiB of transcript"
    );
    ensure!(
        mapping
            .keys()
            .all(|k| ["A", "B", "C", "D", "E", "F", "G", "H", "X"].contains(&k.as_str())),
        "Invalid source viseme code"
    );
    let executable = executable();
    ensure!(
        executable.is_file(),
        "Offline recognizer is missing; use a package with Rhubarb or set ASTRAL_RHUBARB to its executable"
    );
    let service = service.clone();
    let workspace = service.project_workspace(project);
    service.clone().background_task(project,revision,move |job|{
        job.check()?;job.update(json!({"state":"preparing_speech","progress":0.05}));let mut analysis=p.clone();analysis.audio=vec![clip.clone()];analysis.audio[0].start=0;analysis.duration=(clip.duration_seconds*p.fps as f64).ceil().max(1.)as u32;
        let wav=astral_audio::mix(&analysis,&workspace)?;job.check()?;let temp=tempfile::tempdir()?;let input=temp.path().join("input.wav");let output=temp.path().join("visemes.json");let transcript=temp.path().join("dialogue.txt");std::fs::write(&input,wav)?;
        let mut command=Process::new(&executable);command.args(["-f","json","-r",if engine==Recognizer::English{"pocketSphinx"}else{"phonetic"},"--extendedShapes","GHX","--threads","2","--quiet","-o"]).arg(&output);
        if !text.trim().is_empty(){std::fs::write(&transcript,&text)?;command.arg("-d").arg(&transcript);}
        command.arg(&input).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(windows)]{use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
        *job.child.lock().unwrap_or_else(|e|e.into_inner())=Some(command.spawn().context("Start offline recognizer")?);
        let began=Instant::now();let status=loop{if let Err(error)=job.check(){if let Some(child)=job.child.lock().unwrap_or_else(|e|e.into_inner()).as_mut(){let _=child.kill();let _=child.wait();}return Err(error);}
            let mut guard=job.child.lock().unwrap_or_else(|e|e.into_inner());let child=guard.as_mut().context("Recognizer process missing")?;if let Some(status)=child.try_wait()?{guard.take();break status;}
            if began.elapsed()>Duration::from_secs(180){let _=child.kill();let _=child.wait();anyhow::bail!("Speech recognition timed out");}drop(guard);std::thread::sleep(Duration::from_millis(50));job.update(json!({"state":"recognizing_speech","progress":0.2}));
        };ensure!(status.success(),"Offline recognizer failed; inspect the recording format/language");job.check()?;
        ensure!(std::fs::metadata(&output)?.len()<=4*1024*1024,"Recognizer output exceeds budget");let data:Value=serde_json::from_slice(&std::fs::read(output)?)?;
        let keys=cue_keys(&data,p.fps,clip.start,(clip.start+(clip.duration_seconds*p.fps as f64).ceil()as u32).min(p.duration).saturating_sub(1),&mapping)?;
        let track=MouthTrack{id:Id::new_v4(),name:"Recognized speech".into(),character,start:clip.start,end:(clip.start+(clip.duration_seconds*p.fps as f64).ceil()as u32).min(p.duration).saturating_sub(1),size:Point::new(12.,10.),offset:Point::new(0.,5.),color:[245,240,235,255],keys,sprites:Default::default()};let id=track.id;let count=track.keys.len();
        let receipt=service.request(Request::ApplyCommands{batch:Batch{project,expected_revision:revision,commands:vec![Command::PutMouthTrack{track}]}})?;
        Ok(json!({"receipt":receipt,"track":id,"keys":count,"method":if engine==Recognizer::English{"rhubarb_pocketsphinx"}else{"rhubarb_phonetic"},"warnings":["Recognized timing remains editable. Review consonants, noisy speech, language and the assigned mouth artwork."]}))
    })
}
pub fn cue_keys(
    data: &Value,
    fps: u32,
    start: u32,
    end: u32,
    mapping: &BTreeMap<String, MouthShape>,
) -> Result<Vec<MouthKey>> {
    let cues = data["mouthCues"]
        .as_array()
        .context("Recognizer returned no mouth cues")?;
    ensure!(
        !cues.is_empty() && cues.len() <= 12000 && start <= end,
        "Invalid viseme range/budget"
    );
    let mut keys = BTreeMap::from([(start, MouthShape::Rest)]);
    let mut last = 0.;
    for cue in cues {
        let s = cue["start"].as_f64().context("Invalid viseme start")?;
        let e = cue["end"].as_f64().context("Invalid viseme end")?;
        ensure!(
            s.is_finite() && e.is_finite() && s >= last && e > s && e <= 121.,
            "Non-monotonic/out-of-range visemes"
        );
        last = e;
        let code = cue["value"].as_str().context("Invalid viseme code")?;
        let shape = mapping.get(code).copied().unwrap_or(match code {
            "A" => MouthShape::M,
            "B" => MouthShape::Consonant,
            "C" => MouthShape::E,
            "D" => MouthShape::Wide,
            "E" => MouthShape::O,
            "F" => MouthShape::U,
            "G" => MouthShape::F,
            "H" => MouthShape::L,
            "X" => MouthShape::Rest,
            _ => anyhow::bail!("Unknown viseme code"),
        });
        let frame = start.saturating_add((s * fps as f64).round() as u32);
        if frame <= end {
            keys.insert(frame, shape);
        }
        let close = start.saturating_add((e * fps as f64).round() as u32);
        if close <= end {
            keys.insert(close, MouthShape::Rest);
        }
    }
    let mut previous = None;
    Ok(keys
        .into_iter()
        .filter_map(|(frame, shape)| {
            if previous == Some(shape) {
                None
            } else {
                previous = Some(shape);
                Some(MouthKey { frame, shape })
            }
        })
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timed_phonemes_keep_distinct_shapes_and_clip_origin() {
        let cues = json!({"mouthCues":[{"start":0.,"end":0.2,"value":"A"},{"start":0.2,"end":0.5,"value":"H"},{"start":0.5,"end":0.8,"value":"F"}]});
        let keys = cue_keys(&cues, 30, 60, 89, &BTreeMap::new()).unwrap();
        assert_eq!(
            keys.iter().map(|k| (k.frame, k.shape)).collect::<Vec<_>>(),
            vec![
                (60, MouthShape::M),
                (66, MouthShape::L),
                (75, MouthShape::U),
                (84, MouthShape::Rest)
            ]
        );
        assert!(
            cue_keys(
                &json!({"mouthCues":[{"start":1.,"end":0.,"value":"X"}]}),
                24,
                0,
                10,
                &BTreeMap::new()
            )
            .is_err()
        );
    }
}
