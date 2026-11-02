use anyhow::{Context, Result, ensure};
use astral_core::*;
use astral_storage::Workspace;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

mod assembly;
mod avi;
mod native;
pub use assembly::*;
mod presets;
pub use presets::*;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, clap::ValueEnum,
)]
#[serde(rename_all = "snake_case")]
pub enum RenderBackend {
    #[default]
    Auto,
    Cpu,
    Gpu,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct ExportSpec {
    pub path: String,
    #[serde(default)]
    pub renderer: RenderBackend,
    #[serde(default)]
    pub hardware: bool,
    #[serde(default)]
    pub start: Option<u32>,
    #[serde(default)]
    pub end: Option<u32>,
    #[serde(default)]
    pub preset: Preset,
    #[serde(default)]
    pub framing: Framing,
    #[serde(default)]
    pub quality: Quality,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub fps: Option<u32>,
}
pub struct Job {
    pub id: Id,
    pub cancelled: AtomicBool,
    state: Mutex<Value>,
    pub(crate) child: Mutex<Option<Child>>,
    done: AtomicBool,
}
impl Job {
    pub fn status(&self) -> Value {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
    pub fn finished(&self) -> bool {
        self.done.load(Ordering::Relaxed)
    }
    pub fn cancel(&self) {
        if !self.finished() {
            self.cancelled.store(true, Ordering::Relaxed);
            if let Ok(mut child) = self.child.lock()
                && let Some(child) = child.as_mut()
            {
                let _ = child.kill();
            }
        }
    }
    pub(crate) fn check(&self) -> Result<()> {
        ensure!(!self.cancelled.load(Ordering::Relaxed), "Export cancelled");
        Ok(())
    }
    pub(crate) fn update(&self, value: Value) {
        *self.state.lock().unwrap_or_else(|p| p.into_inner()) = value;
    }
}
pub(crate) fn background(
    project: Id,
    revision: u64,
    task: impl FnOnce(&Job) -> Result<Value> + Send + 'static,
) -> Arc<Job> {
    let job = Arc::new(Job {
        id: Id::new_v4(),
        cancelled: AtomicBool::new(false),
        state: Mutex::new(
            json!({"state":"queued","progress":0.,"project":project,"revision":revision}),
        ),
        child: Mutex::new(None),
        done: AtomicBool::new(false),
    });
    let worker = job.clone();
    std::thread::spawn(move || {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(||task(&worker))).unwrap_or_else(|_|Err(anyhow::anyhow!("Background task panicked; inspect the current project revision and local diagnostics"))){Ok(mut value)=>{value["state"]="completed".into();value["progress"]=1.0.into();value["job"]=json!(worker.id);worker.update(value);},Err(e)=>worker.update(json!({"job":worker.id,"state":if worker.cancelled.load(Ordering::Relaxed){"cancelled"}else{"failed"},"error":format!("{e:#}")}))}
        if let Some(mut child) = worker
            .child
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
        worker.done.store(true, Ordering::Release);
    });
    job
}
fn hidden(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}
pub fn encoder_settings_path() -> std::path::PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("HOME"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Astral/encoder.json")
}
pub fn configure_encoder(path: &std::path::Path) -> Result<()> {
    ensure!(
        path.is_absolute() && path.is_file(),
        "Choose an installed FFmpeg executable"
    );
    let destination = encoder_settings_path();
    let root = Workspace::new(destination.parent().context("Settings folder missing")?)?;
    root.atomic_write("encoder.json", &serde_json::to_vec(&json!({"path":path}))?)
}
pub fn ffmpeg() -> String {
    if let Ok(path) = std::env::var("ASTRAL_FFMPEG") {
        return path;
    }
    if let Ok(bytes) = std::fs::read(encoder_settings_path())
        && bytes.len() < 64 * 1024
        && let Ok(value) = serde_json::from_slice::<Value>(&bytes)
        && let Some(path) = value["path"].as_str()
        && std::path::Path::new(path).is_absolute()
        && std::path::Path::new(path).is_file()
    {
        return path.into();
    }
    let filename = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        let bundled = parent.join("resources").join(filename);
        if bundled.is_file() {
            return bundled.to_string_lossy().into_owned();
        }
    }
    let staged = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../build/video-encoder")
        .join(filename);
    if staged.is_file() {
        return staged.to_string_lossy().into_owned();
    }
    "ffmpeg".into()
}

pub fn probe_encoder(hardware: bool) -> (String, Vec<String>) {
    probe_encoder_for(hardware, 128, 128, 30)
}
fn probe_encoder_for(hardware: bool, width: u32, height: u32, fps: u32) -> (String, Vec<String>) {
    let mut warnings = vec![];
    if hardware {
        for encoder in ["h264_nvenc", "h264_qsv", "h264_amf"] {
            let result = hidden(
                Command::new(ffmpeg())
                    .args([
                        "-hide_banner",
                        "-loglevel",
                        "error",
                        "-f",
                        "lavfi",
                        "-i",
                        &format!("color=c=black:s={width}x{height}:r={fps}"),
                        "-frames:v",
                        "1",
                        "-c:v",
                        encoder,
                        "-profile:v",
                        "high",
                        "-pix_fmt",
                        "yuv420p",
                        "-f",
                        "null",
                        "-",
                    ])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null()),
            )
            .spawn();
            if let Ok(mut child) = result {
                let timeout = Instant::now() + Duration::from_secs(8);
                loop {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            if status.success() {
                                return (encoder.into(), warnings);
                            }
                            break;
                        }
                        Ok(None) if Instant::now() < timeout => {
                            std::thread::sleep(Duration::from_millis(25))
                        }
                        _ => {
                            let _ = child.kill();
                            let _ = child.wait();
                            break;
                        }
                    }
                }
            }
        }
        warnings.push("Hardware encoder trial failed or no supported device; using libx264".into());
    }
    ("libx264".into(), warnings)
}

pub fn start(workspace: Workspace, project: Project, spec: ExportSpec) -> Result<Arc<Job>> {
    project.validate()?;
    let plan = Plan::new(&project, &spec)?;
    workspace.path(&spec.path)?;
    let job = Arc::new(Job {
        id: Id::new_v4(),
        cancelled: AtomicBool::new(false),
        state: Mutex::new(
            json!({"state":"queued","progress":0.,"project":project.id,"revision":project.revision}),
        ),
        child: Mutex::new(None),
        done: AtomicBool::new(false),
    });
    let worker = job.clone();
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run(&workspace, &project, &spec, &worker, &plan)
        }))
        .unwrap_or_else(|_| {
            Err(anyhow::anyhow!(
                "Export worker panicked; previous output was preserved; inspect local diagnostics"
            ))
        });
        if let Err(error) = result {
            worker.update(json!({"job":worker.id,"state":if worker.cancelled.load(Ordering::Relaxed){"cancelled"}else{"failed"},"error":format!("{error:#}")}));
        }
        // Reap encoder even on broken pipe or cancellation.
        if let Ok(mut guard) = worker.child.lock()
            && let Some(mut child) = guard.take()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
        worker.done.store(true, Ordering::Release);
    });
    Ok(job)
}
fn run(
    workspace: &Workspace,
    project: &Project,
    spec: &ExportSpec,
    job: &Job,
    plan: &Plan,
) -> Result<()> {
    let first = plan.first;
    let last = plan.last;
    let started = Instant::now();
    job.check()?;
    if plan.format == Format::Png {
        astral_render::validate_artwork(project)?;
        let mut pixels = vec![];
        let (render_seconds, workers, renderer, adapter, readback, cache_hits) =
            render_frames(project, spec, plan, job, |_, rgba| {
                pixels = rgba.to_vec();
                Ok(())
            })?;
        let output = astral_render::color::png(&pixels, plan.width, plan.height)?;
        job.check()?;
        workspace.atomic_write(&spec.path, &output)?;
        job.update(json!({"job":job.id,"state":"completed","progress":1.,"path":spec.path,"plan":plan,"revision":project.revision,"metrics":{"render_seconds":render_seconds,"render_workers":workers,"renderer":renderer,"adapter":adapter,"gpu_readback_seconds":readback,"reused_frames":cache_hits,"frames":1,"total_seconds":started.elapsed().as_secs_f64()}}));
        return Ok(());
    }
    if matches!(
        plan.format,
        Format::Avi | Format::Jpeg | Format::PngSequence | Format::Wav
    ) {
        return native::run(workspace, project, spec, job, plan);
    }
    let target = workspace.path(&spec.path)?;
    let parent = target.parent().context("Output parent missing")?;
    std::fs::create_dir_all(parent)?;
    let output = tempfile::Builder::new()
        .suffix(&format!(".{}", plan.format.extension()))
        .tempfile_in(parent)?;
    let (encoder, mut warnings) = match plan.format {
        Format::Mp4 => probe_encoder_for(spec.hardware, plan.width, plan.height, plan.fps),
        Format::Webm => ("libvpx-vp9".into(), vec![]),
        Format::Gif => ("gif".into(), vec![]),
        Format::Jpeg => ("mjpeg".into(), vec![]),
        _ => ("png".into(), vec![]),
    };
    warnings.extend(plan.warnings.clone());
    match composition::review(project,first,last,((last-first)/120).max(1)) {
        Ok(issues) if !issues.is_empty() => warnings.push(format!("Composition review found {} layout warnings; use review_animation for details and inspect moving previews",issues.len())),
        Err(error) => warnings.push(format!("Composition review incomplete: {error}")),
        _ => {}
    }
    job.check()?;
    // A selected range uses a shifted copy for audio, preserving source offsets.
    let mut audio_project = project.clone();
    audio_project.duration = last - first + 1;
    let first_seconds = first as f64 / project.fps as f64;
    audio_project.audio.retain_mut(|a| {
        let start = a.start as f64 / project.fps as f64;
        let end = start + a.duration_seconds;
        let last_seconds = (last + 1) as f64 / project.fps as f64;
        if end <= first_seconds || start >= last_seconds {
            return false;
        }
        let trim = (first_seconds - start).max(0.);
        a.offset_seconds += trim * a.speed as f64;
        a.duration_seconds = (end.min(last_seconds) - start.max(first_seconds)).max(0.);
        a.start = a.start.saturating_sub(first);
        true
    });
    if plan.format == Format::Wav {
        let bytes = astral_audio::mix(&audio_project, workspace)?;
        job.check()?;
        workspace.atomic_write(&spec.path, &bytes)?;
        job.update(json!({"job":job.id,"state":"completed","progress":1.,"path":spec.path,"plan":plan,"seconds":started.elapsed().as_secs_f64()}));
        return Ok(());
    }
    let mut audio_file = None;
    if !audio_project.audio.is_empty()
        && matches!(
            plan.format,
            Format::Mp4 | Format::Webm | Format::PngSequence
        )
    {
        let mut file = tempfile::Builder::new()
            .suffix(".wav")
            .tempfile_in(parent)?;
        astral_audio::mix_range_to(
            project,
            workspace,
            first_seconds,
            (last + 1) as f64 / project.fps as f64,
            &mut file,
            || job.check(),
        )?;
        audio_file = Some(file);
    }
    let mut command = Command::new(ffmpeg());
    command.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "-s",
        &format!("{}x{}", plan.width, plan.height),
        "-r",
        &plan.fps.to_string(),
        "-i",
        "pipe:0",
    ]);
    if let Some(file) = &audio_file
        && plan.format != Format::PngSequence
    {
        command.arg("-i").arg(file.path());
    }
    let filter = if matches!(plan.format, Format::Mp4 | Format::Webm) {
        "scale=iw:ih:out_color_matrix=bt709:out_range=tv,format=yuv420p,setsar=1"
    } else {
        "setsar=1"
    };
    command.args(["-c:v", &encoder]);
    match plan.format {
        Format::Mp4 => {
            command.args([
                "-vf",
                filter,
                "-pix_fmt",
                "yuv420p",
                "-profile:v",
                "high",
                "-b:v",
                &format!("{}k", plan.video_kbps),
                "-movflags",
                "+faststart",
                "-color_primaries",
                "bt709",
                "-color_trc",
                "bt709",
                "-colorspace",
                "bt709",
            ]);
            if encoder == "libx264" {
                command.args([
                    "-preset",
                    if spec.quality == Quality::Draft {
                        "veryfast"
                    } else {
                        "fast"
                    },
                ]);
            }
            if audio_file.is_some() {
                command.args([
                    "-c:a",
                    "aac",
                    "-b:a",
                    &format!("{}k", plan.audio_kbps),
                    "-ar",
                    "48000",
                    "-ac",
                    "2",
                ]);
            }
        }
        Format::Webm => {
            command.args([
                "-vf",
                filter,
                "-pix_fmt",
                "yuv420p",
                "-b:v",
                &format!("{}k", plan.video_kbps),
                "-deadline",
                "good",
                "-cpu-used",
                "4",
                "-row-mt",
                "1",
            ]);
            if audio_file.is_some() {
                command.args(["-c:a", "libopus", "-b:a", "128k", "-ar", "48000"]);
            }
        }
        Format::Gif => {
            command.args(["-filter_complex",&format!("{filter},split[a][b];[a]palettegen=stats_mode=single[p];[b][p]paletteuse=new=1:dither=sierra2_4a"),"-loop","0"]);
        }
        Format::Jpeg => {
            command.args([
                "-vf",
                filter,
                "-q:v",
                if spec.quality == Quality::Draft {
                    "5"
                } else {
                    "2"
                },
                "-frames:v",
                "1",
                "-update",
                "1",
            ]);
        }
        Format::Png => {
            command.args(["-vf", filter, "-frames:v", "1", "-update", "1"]);
        }
        Format::PngSequence => {
            command.args(["-vf", filter, "-start_number", "0"]);
        }
        Format::Wav | Format::Avi => unreachable!(),
    }
    // The encoder writes only into private temporary storage until every frame succeeds.
    let sequence = tempfile::tempdir_in(parent)?;
    if plan.format == Format::PngSequence {
        command.arg(sequence.path().join("frame_%06d.png"));
    } else {
        command.arg(output.path());
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = hidden(&mut command)
        .spawn()
        .context("FFmpeg not found. Set ASTRAL_FFMPEG to a trusted FFmpeg executable")?;
    let mut stdin = child.stdin.take().context("Encoder input unavailable")?;
    let stderr = child
        .stderr
        .take()
        .context("Encoder diagnostics unavailable")?;
    let diagnostics = std::thread::spawn(move || {
        let mut reader = stderr;
        let mut bytes = Vec::new();
        let mut chunk = [0; 4096];
        while let Ok(n) = reader.read(&mut chunk) {
            if n == 0 {
                break;
            }
            if bytes.len() < 64 * 1024 {
                bytes.extend_from_slice(&chunk[..n.min(64 * 1024 - bytes.len())]);
            }
        }
        String::from_utf8_lossy(&bytes).into_owned()
    });
    *job.child.lock().unwrap_or_else(|p| p.into_inner()) = Some(child);
    let mut transfer_seconds = 0.;
    astral_render::validate_artwork(project)?;
    let (render_seconds, render_workers, render_backend, adapter, readback_seconds, cache_hits) =
        render_frames(project, spec, plan, job, |index, rgba| {
            let tick = Instant::now();
            stdin.write_all(rgba).context("Encoder input failed")?;
            transfer_seconds += tick.elapsed().as_secs_f64();
            job.update(json!({"job":job.id,"state":"rendering","progress":(index+1) as f64/plan.frames as f64,"frame":plan.source_frame(project,index),"encoder":encoder,"warnings":warnings,"plan":plan}));
            Ok(())
        })?;
    drop(stdin);
    loop {
        job.check()?;
        let status = {
            let mut child = job.child.lock().unwrap_or_else(|p| p.into_inner());
            child.as_mut().context("Encoder disappeared")?.try_wait()?
        };
        if let Some(status) = status {
            let detail = diagnostics.join().unwrap_or_default();
            ensure!(status.success(), "FFmpeg failed: {detail}");
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    job.check()?;
    if plan.format == Format::PngSequence {
        use zip::{ZipWriter, write::SimpleFileOptions};
        let mut archive = ZipWriter::new(output.reopen()?);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for index in 0..plan.frames {
            job.check()?;
            let name = format!("frame_{index:06}.png");
            archive.start_file(format!("frames/{name}"), options)?;
            std::io::copy(
                &mut std::fs::File::open(sequence.path().join(name))?,
                &mut archive,
            )?;
        }
        archive.start_file("manifest.json", options)?;
        archive.write_all(&serde_json::to_vec_pretty(&json!({"version":1,"plan":plan,"source_project":project.id,"revision":project.revision,"frame_pattern":"frames/frame_%06d.png","audio":audio_file.as_ref().map(|_|"audio.wav")}))?)?;
        if let Some(file) = &audio_file {
            archive.start_file("audio.wav", options)?;
            std::io::copy(&mut file.reopen()?, &mut archive)?;
        }
        archive.finish()?.sync_all()?;
    }
    job.check()?;
    output.as_file().sync_all()?;
    output.persist(&target).map_err(|e| e.error)?;
    job.update(json!({"job":job.id,"state":"completed","progress":1.,"path":spec.path,"encoder":encoder,"warnings":warnings,"plan":plan,"metrics":{"render_seconds":render_seconds,"render_workers":render_workers,"renderer":render_backend,"adapter":adapter,"gpu_readback_seconds":readback_seconds,"reused_frames":cache_hits,"encoder_transfer_seconds":transfer_seconds,"total_seconds":started.elapsed().as_secs_f64(),"frames":plan.frames},"revision":project.revision}));
    Ok(())
}

// GPU startup/readback can cost more than a small sprite. Reserve Auto for sustained,
// substantial raster coverage; explicit GPU remains available for comparison.
fn prefer_gpu(project: &Project, plan: &Plan) -> bool {
    if plan.frames < 48 {
        return false;
    }
    let frame = plan.first as f64;
    let scene = evaluate(project, frame);
    let area: f32 = project
        .rasters
        .iter()
        .filter(|r| r.visible && frame >= r.start as f64 && frame <= r.end as f64)
        .map(|r| {
            let key = r.at(frame);
            if key.opacity < 0.01 {
                return 0.;
            }
            r.size.x * r.size.y * (key.scale.x * key.scale.y).abs() * scene.zoom * scene.zoom
        })
        .sum();
    area > project.width as f32 * project.height as f32 * 0.5
}

// Each worker has one queued frame. Strided output channels keep frame order
// deterministic while bounding memory; disconnect all receivers before joining
// on any encoder failure or cancellation.
fn render_frames(
    project: &Project,
    spec: &ExportSpec,
    plan: &Plan,
    job: &Job,
    mut consume: impl FnMut(u32, &[u8]) -> Result<()>,
) -> Result<(f64, usize, String, Option<String>, f64, u32)> {
    let try_gpu = spec.renderer == RenderBackend::Gpu
        || (spec.renderer == RenderBackend::Auto && prefer_gpu(project, plan));
    if try_gpu {
        match astral_render::gpu::GpuRenderer::new(plan.width, plan.height) {
            Ok(mut gpu) => {
                let mut seconds = 0.;
                let mut cache = astral_render::FrameCache::default();
                for index in 0..plan.frames {
                    job.check()?;
                    let tick = Instant::now();
                    let list =
                        astral_render::display_list(project, plan.source_frame(project, index));
                    let rgba = cache.render(
                        list,
                        plan.width,
                        plan.height,
                        spec.framing == Framing::Fill,
                        |list| gpu.render(list, spec.framing == Framing::Fill),
                    )?;
                    seconds += tick.elapsed().as_secs_f64();
                    consume(index, &rgba)?;
                }
                return Ok((
                    seconds,
                    1,
                    "gpu".into(),
                    Some(gpu.adapter),
                    gpu.readback_seconds,
                    cache.hits,
                ));
            }
            Err(error) if spec.renderer == RenderBackend::Gpu => {
                return Err(error.context("Requested GPU renderer unavailable; choose CPU or Auto"));
            }
            Err(_) => {} // Auto uses bounded CPU workers if no physical GPU is available.
        }
    }
    let bytes = plan.width as usize * plan.height as usize * 4;
    let max_workers = if project.compositions.iter().any(|g| g.enabled) {
        1
    } else {
        4
    };
    let workers = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(max_workers)
        .min(plan.frames as usize)
        .min((192 * 1024 * 1024usize / (bytes.max(1) * 3)).max(1));
    std::thread::scope(|scope| {
        let mut receivers = vec![];
        for worker in 0..workers {
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            receivers.push(rx);
            scope.spawn(move || {
                let mut cache = astral_render::FrameCache::default();
                for index in (worker as u32..plan.frames).step_by(workers) {
                    let tick = Instant::now();
                    let image = job.check().and_then(|_| {
                        let list =
                            astral_render::display_list(project, plan.source_frame(project, index));
                        let previous_hits = cache.hits;
                        let pixels = cache.render(
                            list,
                            plan.width,
                            plan.height,
                            spec.framing == Framing::Fill,
                            |list| {
                                astral_render::rgba_list_framed(
                                    list,
                                    plan.width,
                                    plan.height,
                                    spec.framing == Framing::Fill,
                                )
                            },
                        )?;
                        Ok((pixels, cache.hits > previous_hits))
                    });
                    let failed = image.is_err();
                    if tx.send((image, tick.elapsed().as_secs_f64())).is_err() || failed {
                        break;
                    }
                }
            });
        }
        let result = (|| {
            let mut seconds = 0.;
            let mut reused = 0;
            for index in 0..plan.frames {
                job.check()?;
                let (image, elapsed) = receivers[index as usize % workers]
                    .recv()
                    .context("Frame worker stopped")?;
                seconds += elapsed;
                let (image, hit) = image?;
                reused += u32::from(hit);
                consume(index, &image)?;
            }
            Ok((
                seconds,
                workers,
                if try_gpu { "cpu_fallback" } else { "cpu" }.into(),
                None,
                0.,
                reused,
            ))
        })();
        drop(receivers);
        result
    })
}

/// Run a bounded, internally constructed encoder/decode command with cancellation.
pub(crate) fn run_encoder(job: &Job, mut command: Command) -> Result<()> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = hidden(&mut command)
        .spawn()
        .context("FFmpeg unavailable; configure an installed encoder")?;
    let stderr = child.stderr.take().context("Missing encoder diagnostics")?;
    *job.child.lock().unwrap_or_else(|p| p.into_inner()) = Some(child);
    let reader = std::thread::spawn(move || {
        let mut source = stderr;
        let mut out = vec![];
        let mut chunk = [0; 4096];
        while let Ok(n) = source.read(&mut chunk) {
            if n == 0 {
                break;
            }
            let budget = (64 * 1024usize).saturating_sub(out.len());
            out.extend_from_slice(&chunk[..n.min(budget)]);
        }
        String::from_utf8_lossy(&out).into_owned()
    });
    let result = (|| -> Result<_> {
        loop {
            job.check()?;
            let state = job
                .child
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .as_mut()
                .context("Encoder stopped")?
                .try_wait()?;
            if let Some(status) = state {
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
    let detail = reader.join().unwrap_or_default();
    ensure!(result?.success(), "Media decoding failed: {detail}");
    Ok(())
}

#[cfg(test)]
mod worker_reliability_tests {
    use super::*;
    #[test]
    fn panicked_background_job_finishes_with_diagnostic_instead_of_sticking() {
        let job = background(Id::new_v4(), 0, |_| panic!("synthetic worker failure"));
        let began = Instant::now();
        while !job.finished() && began.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(job.finished());
        assert_eq!(job.status()["state"], "failed");
        assert!(job.status()["error"].as_str().unwrap().contains("panicked"));
    }
}
