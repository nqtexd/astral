//! Finishing already-rendered shots without requiring an external shell command.
use super::*;
use std::io::{BufRead, BufReader};
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AssemblyClip {
    pub path: String,
    #[serde(default)]
    pub start_seconds: f64,
    pub end_seconds: f64,
    /// Disable for silent source files; a silent track is inserted to retain sync.
    #[serde(default = "audio_enabled")]
    pub audio: bool,
}
fn audio_enabled() -> bool {
    true
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AssemblySpec {
    pub path: String,
    pub clips: Vec<AssemblyClip>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}
impl AssemblySpec {
    pub fn validate(&self, workspace: &Workspace) -> Result<()> {
        ensure!((2..=32).contains(&self.clips.len()), "Assemble 2–32 shots");
        ensure!(
            self.width >= 16
                && self.height >= 16
                && self.width <= 3840
                && self.height <= 3840
                && self.width.is_multiple_of(2)
                && self.height.is_multiple_of(2)
                && self.width as u64 * self.height as u64 <= 8_294_400
                && (1..=60).contains(&self.fps),
            "Invalid assembly dimensions or FPS"
        );
        ensure!(
            std::path::Path::new(&self.path)
                .extension()
                .is_some_and(|e| e == "mp4"),
            "Assembly output must be MP4"
        );
        let target = workspace.path(&self.path)?;
        let mut total = 0.;
        for clip in &self.clips {
            ensure!(
                clip.start_seconds.is_finite()
                    && clip.end_seconds.is_finite()
                    && clip.start_seconds >= 0.
                    && clip.end_seconds > clip.start_seconds
                    && clip.end_seconds - clip.start_seconds <= 600.,
                "Invalid shot trim; maximum 10 minutes per shot"
            );
            let path = workspace.path(&clip.path)?;
            ensure!(
                path.is_file() && path != target,
                "Shot must exist and differ from the output"
            );
            ensure!(
                path.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(
                        |e| ["mp4", "mov", "webm", "mkv"].contains(&e.to_lowercase().as_str())
                    ),
                "Use local video shots, not playlists"
            );
            total += clip.end_seconds - clip.start_seconds;
        }
        ensure!(total <= 3600., "Maximum assembly duration is one hour");
        Ok(())
    }
}
pub fn assemble(
    workspace: Workspace,
    project: Id,
    revision: u64,
    spec: AssemblySpec,
) -> Result<Arc<Job>> {
    spec.validate(&workspace)?;
    Ok(background(project, revision, move |job| {
        let target = workspace.path(&spec.path)?;
        let parent = target.parent().context("Output parent missing")?;
        std::fs::create_dir_all(parent)?;
        let output = tempfile::Builder::new()
            .suffix(".mp4")
            .tempfile_in(parent)?;
        let mut command = Command::new(ffmpeg());
        command.args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"]);
        for clip in &spec.clips {
            command
                .args([
                    "-protocol_whitelist",
                    "file,pipe",
                    "-ss",
                    &clip.start_seconds.to_string(),
                    "-t",
                    &(clip.end_seconds - clip.start_seconds).to_string(),
                    "-i",
                ])
                .arg(workspace.path(&clip.path)?);
        }
        let mut filters = vec![];
        let mut streams = String::new();
        let mut duration = 0.;
        for (i, clip) in spec.clips.iter().enumerate() {
            let length = clip.end_seconds - clip.start_seconds;
            duration += length;
            filters.push(format!("[{i}:v:0]fps={},scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2:color=black,setsar=1,format=yuv420p,tpad=stop_mode=clone:stop_duration={length},trim=duration={length},setpts=PTS-STARTPTS[v{i}]",spec.fps,spec.width,spec.height,spec.width,spec.height));
            if clip.audio {
                filters.push(format!("[{i}:a:0]aresample=48000,aformat=sample_fmts=fltp:channel_layouts=stereo,apad=whole_dur={length},atrim=duration={length},asetpts=PTS-STARTPTS[a{i}]"));
            } else {
                filters.push(format!(
                    "anullsrc=r=48000:cl=stereo,atrim=duration={length},asetpts=PTS-STARTPTS[a{i}]"
                ));
            }
            streams += &format!("[v{i}][a{i}]");
        }
        filters.push(format!(
            "{streams}concat=n={}:v=1:a=1[v][a]",
            spec.clips.len()
        ));
        command
            .args([
                "-filter_complex",
                &filters.join(";"),
                "-map",
                "[v]",
                "-map",
                "[a]",
                "-c:v",
                "libx264",
                "-preset",
                "fast",
                "-crf",
                "18",
                "-c:a",
                "aac",
                "-b:a",
                "192k",
                "-movflags",
                "+faststart",
                "-progress",
                "pipe:1",
            ])
            .arg(output.path())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = hidden(&mut command).spawn().context("FFmpeg unavailable")?;
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        *job.child.lock().unwrap_or_else(|p| p.into_inner()) = Some(child);
        std::thread::scope(|scope| -> Result<Value> {
            let diagnostic = scope.spawn(move || {
                let mut bytes = vec![];
                let mut reader = stderr;
                let mut chunk = [0; 4096];
                while let Ok(n) = reader.read(&mut chunk) {
                    if n == 0 {
                        break;
                    }
                    let limit = (64 * 1024usize).saturating_sub(bytes.len());
                    bytes.extend_from_slice(&chunk[..n.min(limit)]);
                }
                String::from_utf8_lossy(&bytes).into_owned()
            });
            scope.spawn(move||{for line in BufReader::new(stdout).lines().map_while(std::result::Result::ok){if let Some(value)=line.strip_prefix("out_time_us=").and_then(|v|v.parse::<f64>().ok()){job.update(json!({"state":"assembling","progress":(value/1_000_000./duration).clamp(0.,0.99),"job":job.id}));}}});
            let result: Result<std::process::ExitStatus> = (|| {
                loop {
                    job.check()?;
                    let status = job
                        .child
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .as_mut()
                        .context("Assembly encoder missing")?
                        .try_wait()?;
                    if let Some(status) = status {
                        return Ok(status);
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            })();
            if result.is_err()
                && let Some(child) = job.child.lock().unwrap_or_else(|p| p.into_inner()).as_mut()
            {
                let _ = child.kill();
                let _ = child.wait();
            }
            let detail = diagnostic.join().unwrap_or_default();
            let status = result?;
            ensure!(
                status.success(),
                "Shot assembly failed (check audio flags and trims): {detail}"
            );
            job.check()?;
            output.as_file().sync_all()?;
            output.persist(&target).map_err(|e| e.error)?;
            Ok(
                json!({"path":spec.path,"duration_seconds":duration,"clips":spec.clips.len(),"revision":revision,"warnings":["Hard cuts; shots are normalized and re-encoded. This does not modify the editable project."]}),
            )
        })
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trim_and_path_validation_prevent_output_from_replacing_a_source() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = Workspace::new(dir.path()).unwrap();
        workspace.atomic_write("shot.mp4", b"fixture").unwrap();
        let clip = AssemblyClip {
            path: "shot.mp4".into(),
            start_seconds: 0.,
            end_seconds: 1.,
            audio: false,
        };
        let mut spec = AssemblySpec {
            path: "finished.mp4".into(),
            clips: vec![clip.clone(), clip],
            width: 640,
            height: 360,
            fps: 24,
        };
        assert!(spec.validate(&workspace).is_ok());
        spec.path = "shot.mp4".into();
        assert!(spec.validate(&workspace).is_err());
        spec.path = "../escape.mp4".into();
        assert!(spec.validate(&workspace).is_err());
        spec.path = "finished.mp4".into();
        spec.clips[0].end_seconds = f64::NAN;
        assert!(spec.validate(&workspace).is_err());
    }
}
