use crate::{
    service::{Request, Service},
    voice::VoiceOptions,
};
use anyhow::{Context, Result, ensure};
use astral_core::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::Write;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "task", rename_all = "snake_case")]
pub enum AssistRequest {
    ListVoices,
    ModelInfo,
    TrainModel {
        project: Id,
        expected_revision: u64,
        samples: usize,
        epochs: usize,
    },
    Recognize {
        project: Id,
        path: String,
    },
    ImportRig {
        project: Id,
        expected_revision: u64,
        path: String,
        detection: astral_assist::recognition::Detection,
        name: String,
        center: Point,
        height: f32,
        frame: u32,
    },
    CompletePose {
        project: Id,
        expected_revision: u64,
        character: Id,
        frame: u32,
        observed: Vec<String>,
        model_path: Option<String>,
        apply: bool,
    },
    SuggestStroke {
        project: Id,
        layer: Id,
        frame: u32,
        strength: f32,
    },
    CompleteFrame {
        project: Id,
        expected_revision: u64,
        layer: Id,
        sources: Vec<u32>,
        frame: u32,
        smoothing: f64,
        apply: bool,
    },
    GenerateFrames {
        project: Id,
        expected_revision: u64,
        layer: Id,
        sources: Vec<u32>,
        start: u32,
        end: u32,
        smoothing: f64,
        overwrite: bool,
    },
    Voice {
        project: Id,
        expected_revision: u64,
        text: String,
        start: u32,
        character: Option<Id>,
        options: VoiceOptions,
        dialogue: bool,
        formatting: TextFormatting,
        speaker: String,
    },
}
fn write_new(workspace: &astral_storage::Workspace, path: &str, bytes: &[u8]) -> Result<()> {
    let target = workspace.path(path)?;
    let parent = target.parent().context("Output folder missing")?;
    std::fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(target).map_err(|e| e.error)?;
    Ok(())
}
impl Service {
    pub(crate) fn background_task(
        &self,
        project: Id,
        revision: u64,
        task: impl FnOnce(&crate::export::Job) -> Result<Value> + Send + 'static,
    ) -> Result<Value> {
        let mut store = self.store.lock().unwrap();
        ensure!(
            store.jobs.values().filter(|j| !j.finished()).count() < 2,
            "Two background jobs are already running"
        );
        if store.jobs.len() >= 32
            && let Some(id) = store
                .jobs
                .iter()
                .find(|(_, j)| j.finished())
                .map(|(id, _)| *id)
        {
            store.jobs.remove(&id);
        }
        let job = crate::export::background(project, revision, task);
        let id = job.id;
        store.jobs.insert(id, job);
        Ok(json!({"job":id}))
    }
    fn checked_project(&self, id: Id, revision: u64) -> Result<Project> {
        let p = self.snapshot(id)?;
        ensure!(
            p.revision == revision,
            "Stale revision: expected {}, received {revision}",
            p.revision
        );
        Ok(p)
    }
    pub fn assist(&self, request: AssistRequest) -> Result<Value> {
        match request {
            AssistRequest::ListVoices => Ok(
                json!({"voices":std::thread::spawn(crate::voice::voices).join().map_err(|_|anyhow::anyhow!("Voice enumeration failed"))??}),
            ),
            AssistRequest::ModelInfo => {
                let model = astral_assist::network::PoseNetwork::bundled()?;
                let report: Value = serde_json::from_slice(include_bytes!(
                    "../../../models/pose-prior-v1-report.json"
                ))?;
                Ok(
                    json!({"available":true,"parameters":model.w1.len()+model.b1.len()+model.w2.len()+model.b2.len(),"report":report}),
                )
            }
            AssistRequest::TrainModel {
                project,
                expected_revision,
                samples,
                epochs,
            } => {
                self.checked_project(project, expected_revision)?;
                let service = self.clone();
                let workspace = self.project_workspace(project);
                ensure!(
                    (512..=32768).contains(&samples) && (1..=300).contains(&epochs),
                    "Training budget is outside supported bounds"
                );
                self.background_task(project,expected_revision,move|job|{
                    job.check()?;job.update(json!({"state":"training","progress":0.05,"project":project}));
                    let(model,report)=astral_assist::network::train_with_progress(samples,epochs,0x41535452414c,|fraction|{job.check()?;job.update(json!({"state":"training","progress":fraction}));Ok(())})?;job.check()?;
                    let path=format!("models/pose-{}.json",job.id);write_new(&workspace,&path,&serde_json::to_vec(&model)?)?;
                    let asset=Asset{id:Id::new_v4(),name:"Trained pose model".into(),path:path.clone(),kind:"model".into(),tags:vec!["masked_pose_mlp_v1".into(),"synthetic".into()]};
                    job.check()?;let receipt=service.request(Request::ApplyCommands{batch:Batch{project,expected_revision,commands:vec![Command::AddAsset{asset}]}})?;
                    Ok(json!({"path":path,"report":report,"receipt":receipt,"revision":receipt["revision"]}))
                })
            }
            AssistRequest::Recognize { project, path } => {
                self.snapshot(project)?;
                let image_path = self.project_workspace(project).path(&path)?;
                ensure!(
                    std::fs::metadata(&image_path)?.len() <= 64 * 1024 * 1024,
                    "Image exceeds 64 MiB"
                );
                let bytes = std::fs::read(image_path)?;
                let detection = astral_assist::recognition::recognize(&bytes)?;
                Ok(
                    json!({"detection":detection,"path":path,"method":"segmentation_and_skeleton_topology","review_required":true}),
                )
            }
            AssistRequest::ImportRig {
                project,
                expected_revision,
                path,
                detection,
                name,
                center,
                height,
                frame,
            } => {
                let p = self.checked_project(project, expected_revision)?;
                ensure!(frame < p.duration, "Start frame is outside the project");
                let workspace = self.project_workspace(project);
                ensure!(
                    workspace.path(&path)?.is_file(),
                    "Source image does not exist"
                );
                let rig = astral_assist::recognition::rig(&detection, name, center, height, frame)?;
                let id = rig.id;
                let mut commands = vec![];
                if !p.assets.iter().any(|a| a.path == path) {
                    commands.push(Command::AddAsset {
                        asset: Asset {
                            id: Id::new_v4(),
                            name: "Stickman reference".into(),
                            path,
                            kind: "image".into(),
                            tags: vec!["reference".into(), format!("rig:{id}")],
                        },
                    });
                }
                commands.push(Command::AddRig { character: rig });
                let mut value = self.request(Request::ApplyCommands {
                    batch: Batch {
                        project,
                        expected_revision,
                        commands,
                    },
                })?;
                value["character"] = json!(id);
                value["warnings"] = json!(detection.warnings);
                Ok(value)
            }
            AssistRequest::CompletePose {
                project,
                expected_revision,
                character,
                frame,
                observed,
                model_path,
                apply,
            } => {
                let p = self.checked_project(project, expected_revision)?;
                ensure!(frame < p.duration, "Frame is outside the project");
                let c = p.character(character).context("Character not found")?;
                ensure!(!c.locked, "Unlock the character first");
                let model = if let Some(path) = model_path {
                    let model_path = self.project_workspace(project).path(path)?;
                    ensure!(
                        std::fs::metadata(&model_path)?.len() <= 1024 * 1024,
                        "Pose model exceeds 1 MiB"
                    );
                    let bytes = std::fs::read(model_path)?;
                    ensure!(bytes.len() <= 1024 * 1024, "Pose model exceeds 1 MiB");
                    let m: astral_assist::network::PoseNetwork = serde_json::from_slice(&bytes)?;
                    m.validate()?;
                    m
                } else {
                    astral_assist::network::PoseNetwork::bundled()?
                };
                let pose = evaluate(&p, frame as f64)
                    .characters
                    .into_iter()
                    .find(|c| c.id == character)
                    .context("Character is hidden")?
                    .pose;
                let result = astral_assist::network::complete_pose(&model, c, &pose, &observed)?;
                let commands = vec![Command::SetPose {
                    character,
                    frame,
                    pose: result.clone(),
                    easing: Easing::Smooth,
                }];
                if apply {
                    self.request(Request::ApplyCommands {
                        batch: Batch {
                            project,
                            expected_revision,
                            commands,
                        },
                    })
                } else {
                    Ok(
                        json!({"project":project,"revision":p.revision,"pose":result,"character":character,"frame":frame,"commands":commands,"method":"masked_pose_mlp","observed_preserved":true}),
                    )
                }
            }
            AssistRequest::SuggestStroke {
                project,
                layer,
                frame,
                strength,
            } => {
                let p = self.snapshot(project)?;
                ensure!(frame < p.duration, "Frame is outside the project");
                let cel = p.cel_at(layer, frame).context("Draw a stroke first")?;
                let last = cel.strokes.last().context("Draw a stroke first")?;
                let suggestion = astral_assist::drawing::suggest_stroke(last, strength);
                if let Some(s) = suggestion {
                    let mut strokes = cel.strokes.clone();
                    *strokes.last_mut().unwrap() = s.stroke;
                    Ok(
                        json!({"project":project,"revision":p.revision,"layer":layer,"frame":frame,"label":s.label,"score":s.score,"ghost":[s.ghost],"commands":[Command::SetCel{layer,frame,strokes}],"method":"geometric_stroke_fit"}),
                    )
                } else {
                    Ok(json!({"project":project,"revision":p.revision,"suggestion":null}))
                }
            }
            AssistRequest::CompleteFrame {
                project,
                expected_revision,
                layer,
                sources,
                frame,
                smoothing,
                apply,
            } => {
                let p = self.checked_project(project, expected_revision)?;
                ensure!(
                    p.layers.iter().any(|l| l.id == layer && !l.locked),
                    "Select an unlocked drawing layer"
                );
                let learned =
                    astral_assist::drawing::CelLearner::train(&p, layer, &sources, smoothing)?;
                let strokes = learned.complete(&p, layer, frame)?;
                let existing = p
                    .cels
                    .iter()
                    .find(|c| c.layer == layer && c.frame == frame)
                    .map_or(0, |c| c.strokes.len());
                let ghost = strokes[existing..].to_vec();
                let commands = vec![Command::SetCel {
                    layer,
                    frame,
                    strokes,
                }];
                if apply {
                    self.request(Request::ApplyCommands {
                        batch: Batch {
                            project,
                            expected_revision,
                            commands,
                        },
                    })
                } else {
                    Ok(
                        json!({"project":project,"revision":p.revision,"layer":layer,"frame":frame,"ghost":ghost,"commands":commands,"label":"Complete drawing from examples","method":"trained_RBF_stroke_trajectories"}),
                    )
                }
            }
            AssistRequest::GenerateFrames {
                project,
                expected_revision,
                layer,
                sources,
                start,
                end,
                smoothing,
                overwrite,
            } => {
                let p = self.checked_project(project, expected_revision)?;
                ensure!(
                    p.layers.iter().any(|l| l.id == layer && !l.locked),
                    "Select an unlocked drawing layer"
                );
                let service = self.clone();
                self.background_task(project,expected_revision,move|job|{
                    job.check()?;job.update(json!({"state":"learning","progress":0.1}));
                    let learned=astral_assist::drawing::CelLearner::train(&p,layer,&sources,smoothing)?;let commands=learned.generate(&p,layer,start,end,overwrite)?;let count=commands.len();job.check()?;
                    let receipt=service.request(Request::ApplyCommands{batch:Batch{project,expected_revision,commands}})?;
                    Ok(json!({"generated_frames":count,"receipt":receipt,"revision":receipt["revision"],"method":"trained_RBF_stroke_trajectories","source_frames_preserved":true}))
                })
            }
            AssistRequest::Voice {
                project,
                expected_revision,
                text,
                start,
                character,
                options,
                dialogue,
                formatting,
                speaker,
            } => {
                let p = self.checked_project(project, expected_revision)?;
                ensure!(start < p.duration, "Voice starts outside the project");
                if let Some(id) = character {
                    ensure!(p.character(id).is_some(), "Dialogue character not found");
                }
                let service = self.clone();
                let workspace = self.project_workspace(project);
                self.background_task(project,expected_revision,move|job|{
                    job.check()?;job.update(json!({"state":"synthesizing","progress":0.15}));
                    let bytes=crate::voice::synthesize_cancellable(&text,&options,||job.cancelled.load(std::sync::atomic::Ordering::Relaxed))?;job.check()?;let seconds=astral_audio::wav_duration(&bytes)?;ensure!(seconds>0.&&seconds<=600.,"Voice clip must be at most ten minutes");
                    let frames=(seconds*p.fps as f64).ceil().max(1.) as u32;let end=start.checked_add(frames).context("Voice range overflow")?;ensure!(end<=1_000_000,"Voice exceeds timeline limit");
                    let path=format!("assets/voice/{}.wav",job.id);write_new(&workspace,&path,&bytes)?;
                    let asset=Id::new_v4();let clip=Id::new_v4();let mut commands=vec![];if end>p.duration{commands.push(Command::SetProject{name:None,duration:Some(end),background:None});}
                    commands.push(Command::AddAsset{asset:Asset{id:asset,name:if speaker.is_empty(){"Voice".into()}else{speaker.clone()},path:path.clone(),kind:"audio".into(),tags:vec!["speech".into(),"offline_windows_voice".into()]}});
                    commands.push(Command::AddAudio{clip:AudioClip{id:clip,name:"Voice".into(),asset,start,offset_seconds:0.,duration_seconds:seconds,gain:1.,fade_in:0.,fade_out:0.,looping:false}});
                    let dialogue_size=Point::new((p.width as f32*0.7).clamp(80.,1000.),(p.height as f32*0.22).clamp(60.,180.));
                    let dialogue_position=if let Some(character)=character.filter(|_|dialogue){
                        let scene=evaluate(&p,start as f64);let c=scene.characters.iter().find(|c|c.id==character).context("Dialogue character is hidden at the start frame")?;
                        let head_index=p.character(character).and_then(|c|c.joints.iter().position(|j|j.name=="head")).unwrap_or(0);let head=c.pose.root+c.pose.joints[head_index];
                        Point::new((head.x-dialogue_size.x/2.).clamp(8.,(p.width as f32-dialogue_size.x-8.).max(8.)),(head.y-dialogue_size.y-24.).clamp(8.,(p.height as f32-dialogue_size.y-8.).max(8.)))-c.pose.root
                    }else{Point::new(p.width as f32*0.1,p.height as f32*0.08)};
                    let dialogue_id=if dialogue{let id=Id::new_v4();commands.push(Command::AddDialogue{dialogue:Dialogue{id,name:"Voice dialogue".into(),text,speaker,start,end:end-1,position:dialogue_position,size:dialogue_size,font_size:(p.height as f32*0.035).clamp(12.,30.),color:[250,250,250,255],text_color:[30,32,37,255],appearance:DialogueAppearance::default(),style:if character.is_some(){DialogueStyle::Speech}else{DialogueStyle::Caption},character,typewriter:0.,formatting}});Some(id)}else{None};
                    job.check()?;let receipt=service.request(Request::ApplyCommands{batch:Batch{project,expected_revision,commands}})?;
                    Ok(json!({"path":path,"asset":asset,"clip":clip,"dialogue":dialogue_id,"duration_seconds":seconds,"receipt":receipt,"revision":receipt["revision"]}))
                })
            }
        }
    }
}
