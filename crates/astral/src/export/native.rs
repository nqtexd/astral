use super::*;
use std::io::Cursor;
pub(super) fn jpeg(rgba: &[u8], width: u32, height: u32, quality: u8) -> Result<Vec<u8>> {
    let rgb: Vec<u8> = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [0, 1, 2].map(|i| ((p[i] as u16 * p[3] as u16 + 127) / 255) as u8))
        .collect();
    let mut bytes = vec![];
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality).encode(
        &rgb,
        width,
        height,
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(bytes)
}
pub(super) fn run(
    workspace: &Workspace,
    project: &Project,
    spec: &ExportSpec,
    job: &Job,
    plan: &Plan,
) -> Result<()> {
    let started = Instant::now();
    astral_render::validate_artwork(project)?;
    let target = workspace.path(&spec.path)?;
    let parent = target.parent().context("Output directory missing")?;
    std::fs::create_dir_all(parent)?;
    let output = tempfile::NamedTempFile::new_in(parent)?;
    let mut audio = None;
    if !project.audio.is_empty() && plan.format != Format::Jpeg {
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        astral_audio::mix_range_to(
            project,
            workspace,
            plan.first as f64 / project.fps as f64,
            (plan.last + 1) as f64 / project.fps as f64,
            &mut file,
            || job.check(),
        )?;
        audio = Some(file);
    }
    let mut metrics = None;
    let quality = match spec.quality {
        Quality::Draft => 65,
        Quality::Standard => 85,
        Quality::High => 95,
    };
    match plan.format {
        Format::Wav => {
            if let Some(file) = audio {
                std::io::copy(&mut file.reopen()?, &mut output.reopen()?)?;
            } else {
                astral_audio::mix_range_to(
                    project,
                    workspace,
                    plan.first as f64 / project.fps as f64,
                    (plan.last + 1) as f64 / project.fps as f64,
                    output.reopen()?,
                    || job.check(),
                )?;
            }
        }
        Format::Jpeg => {
            metrics = Some(render_frames(project, spec, plan, job, |_, rgba| {
                output
                    .reopen()?
                    .write_all(&jpeg(rgba, plan.width, plan.height, quality)?)?;
                Ok(())
            })?);
        }
        Format::PngSequence => {
            use zip::{ZipWriter, write::SimpleFileOptions};
            let mut archive = ZipWriter::new(output.reopen()?);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            metrics = Some(render_frames(project, spec, plan, job, |index, rgba| {
                archive.start_file(format!("frames/frame_{index:06}.png"), options)?;
                let image = image::RgbaImage::from_raw(plan.width, plan.height, rgba.to_vec())
                    .context("Invalid image dimensions")?;
                let mut png = Cursor::new(vec![]);
                image.write_to(&mut png, image::ImageFormat::Png)?;
                archive.write_all(&png.into_inner())?;
                Ok(())
            })?);
            archive.start_file("manifest.json", options)?;
            archive.write_all(&serde_json::to_vec_pretty(&json!({"version":1,"plan":plan,"source_project":project.id,"revision":project.revision,"frame_pattern":"frames/frame_%06d.png","audio":audio.as_ref().map(|_|"audio.wav"),"color_space":"sRGB","alpha":"straight"}))?)?;
            if let Some(file) = audio {
                archive.start_file("audio.wav", options)?;
                std::io::copy(&mut file.reopen()?, &mut archive)?;
            }
            archive.finish()?.sync_all()?;
        }
        Format::Avi => {
            let mut avi = super::avi::Avi::new(
                output.reopen()?,
                plan.width,
                plan.height,
                plan.fps,
                plan.frames,
                audio.is_some(),
            )?;
            let mut wav = audio
                .as_ref()
                .map(|f| hound::WavReader::open(f.path()))
                .transpose()?;
            metrics = Some(render_frames(project, spec, plan, job, |index, rgba| {
                let pcm = if let Some(reader) = wav.as_mut() {
                    let a = index as u64 * 48000 / plan.fps as u64;
                    let b = (index + 1) as u64 * 48000 / plan.fps as u64;
                    let mut samples = reader.samples::<i16>();
                    let mut bytes = Vec::with_capacity((b - a) as usize * 4);
                    for _ in 0..(b - a) * 2 {
                        bytes.extend_from_slice(
                            &samples.next().transpose()?.unwrap_or(0).to_le_bytes(),
                        );
                    }
                    Some(bytes)
                } else {
                    None
                };
                avi.frame(
                    &jpeg(rgba, plan.width, plan.height, quality)?,
                    pcm.as_deref(),
                )?;
                Ok(())
            })?);
            avi.finish()?.sync_all()?;
        }
        _ => anyhow::bail!("Not a native export format"),
    }
    job.check()?;
    output.as_file().sync_all()?;
    output.persist(&target).map_err(|e| e.error)?;
    let metrics=metrics.map(|(render,workers,renderer,adapter,readback,reused)|json!({"render_seconds":render,"render_workers":workers,"renderer":renderer,"adapter":adapter,"gpu_readback_seconds":readback,"reused_frames":reused,"frames":plan.frames,"total_seconds":started.elapsed().as_secs_f64()})).unwrap_or_else(||json!({"total_seconds":started.elapsed().as_secs_f64()}));
    job.update(json!({"job":job.id,"state":"completed","progress":1.,"path":spec.path,"plan":plan,"revision":project.revision,"encoder":"astral_native","metrics":metrics,"warnings":plan.warnings}));
    Ok(())
}
