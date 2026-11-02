//! Offline WAV mixing. Source clips stay independently editable in the document.
use anyhow::{Context, Result, ensure};
use astral_core::*;
mod decode;
pub use decode::{Pcm, decode, normalize_wav};
pub fn wav_duration(bytes: &[u8]) -> Result<f64> {
    let reader = hound::WavReader::new(std::io::Cursor::new(bytes))?;
    let spec = reader.spec();
    ensure!(
        spec.sample_rate > 0 && spec.channels > 0,
        "Invalid audio format"
    );
    Ok(reader.len() as f64 / spec.channels as f64 / spec.sample_rate as f64)
}
use astral_storage::Workspace;
use std::io::Cursor;
pub const SAMPLE_RATE: u32 = 48000;

pub fn mix(project: &Project, workspace: &Workspace) -> Result<Vec<u8>> {
    let seconds = project.duration as f64 / project.fps as f64;
    ensure!(
        seconds <= 600.,
        "In-memory mix is limited to ten minutes; use streaming export for longer projects"
    );
    let mut output = Cursor::new(Vec::new());
    mix_range_to(project, workspace, 0., seconds, &mut output, || Ok(()))?;
    Ok(output.into_inner())
}
mod stream;
pub use stream::mix_range_to;

/// A reproducible, local impact sound used by the example project.
pub fn impact_wav() -> Result<Vec<u8>> {
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(
            &mut bytes,
            hound::WavSpec {
                channels: 1,
                sample_rate: SAMPLE_RATE,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )?;
        let mut seed = 17u32;
        for i in 0..12000 {
            let t = i as f32 / SAMPLE_RATE as f32;
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let noise = (seed >> 8) as f32 / 16777215. * 2. - 1.;
            let sample =
                ((t * 120. * std::f32::consts::TAU).sin() * 0.5 + noise * 0.4) * (-t * 28.).exp();
            writer.write_sample((sample * i16::MAX as f32) as i16)?;
        }
        writer.finalize()?;
    }
    Ok(bytes.into_inner())
}
pub fn waveform(bytes: &[u8], bins: usize) -> Result<Vec<f32>> {
    ensure!((1..=4096).contains(&bins), "Invalid waveform bins");
    Ok(analyze(bytes, bins)?.1)
}
pub fn analyze(bytes: &[u8], bins: usize) -> Result<(f64, Vec<f32>)> {
    ensure!((1..=4096).contains(&bins), "Invalid waveform bins");
    let pcm = decode(bytes)?;
    let duration = pcm.samples.len() as f64 / pcm.channels as f64 / pcm.rate as f64;
    let samples = pcm.samples;
    let mut peaks = vec![0f32; bins];
    for (i, s) in samples.iter().enumerate() {
        let b = i * bins / samples.len().max(1);
        peaks[b] = peaks[b].max(s.abs());
    }
    Ok((duration, peaks))
}
