//! Offline WAV mixing. Source clips stay independently editable in the document.
use anyhow::{Context, Result, ensure};
use astral_core::*;
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
        "Audio export currently limited to ten minutes"
    );
    let mut mixed = vec![[0f32; 2]; (seconds * SAMPLE_RATE as f64).ceil() as usize];
    for clip in &project.audio {
        let asset = project
            .assets
            .iter()
            .find(|a| a.id == clip.asset)
            .context("Audio asset missing")?;
        let bytes = workspace.read(&asset.path)?;
        let mut reader = hound::WavReader::new(Cursor::new(bytes)).with_context(|| {
            format!("{}: import PCM WAV audio for the native mixer", asset.name)
        })?;
        let spec = reader.spec();
        ensure!(
            (1..=2).contains(&spec.channels) && spec.sample_rate > 0,
            "Only mono/stereo WAV is supported"
        );
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => reader
                .samples::<f32>()
                .collect::<std::result::Result<_, _>>()?,
            hound::SampleFormat::Int => {
                ensure!(
                    (1..=32).contains(&spec.bits_per_sample),
                    "Invalid WAV depth"
                );
                let scale = 2f32.powi(spec.bits_per_sample as i32 - 1);
                reader
                    .samples::<i32>()
                    .map(|v| v.map(|v| v as f32 / scale))
                    .collect::<std::result::Result<_, _>>()?
            }
        };
        ensure!(
            samples.iter().all(|v| v.is_finite()),
            "Non-finite audio samples"
        );
        let source_frames = samples.len() / spec.channels as usize;
        ensure!(source_frames > 0, "Empty WAV file");
        let start = (clip.start as f64 / project.fps as f64 * SAMPLE_RATE as f64).round() as usize;
        let count = (clip.duration_seconds * SAMPLE_RATE as f64).round() as usize;
        for i in 0..count.min(mixed.len().saturating_sub(start)) {
            let time = i as f64 / SAMPLE_RATE as f64;
            let source = (clip.offset_seconds + time) * spec.sample_rate as f64;
            let source = if clip.looping {
                source % source_frames as f64
            } else {
                if source >= source_frames as f64 {
                    break;
                }
                source
            };
            let f = source.floor() as usize;
            let next = (f + 1).min(source_frames - 1);
            let t = (source - f as f64) as f32;
            let fade_in = if clip.fade_in > 0. {
                (time as f32 / clip.fade_in).min(1.)
            } else {
                1.
            };
            let fade_out = if clip.fade_out > 0. {
                ((clip.duration_seconds - time) as f32 / clip.fade_out).clamp(0., 1.)
            } else {
                1.
            };
            for (channel, output) in mixed[start + i].iter_mut().enumerate() {
                let ch = channel.min(spec.channels as usize - 1);
                let a = samples[f * spec.channels as usize + ch];
                let b = samples[next * spec.channels as usize + ch];
                *output += (a + (b - a) * t) * clip.gain * fade_in * fade_out;
            }
        }
    }
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(
            &mut bytes,
            hound::WavSpec {
                channels: 2,
                sample_rate: SAMPLE_RATE,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )?;
        for frame in mixed {
            for sample in frame {
                writer.write_sample((sample.clamp(-1., 1.) * i16::MAX as f32) as i16)?;
            }
        }
        writer.finalize()?;
    }
    Ok(bytes.into_inner())
}

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
    let mut reader = hound::WavReader::new(Cursor::new(bytes))?;
    let spec = reader.spec();
    let samples: Vec<f32> = if spec.sample_format == hound::SampleFormat::Float {
        reader
            .samples::<f32>()
            .collect::<std::result::Result<_, _>>()?
    } else {
        let scale = 2f32.powi(spec.bits_per_sample as i32 - 1);
        reader
            .samples::<i32>()
            .map(|v| v.map(|v| v as f32 / scale))
            .collect::<std::result::Result<_, _>>()?
    };
    let mut peaks = vec![0f32; bins];
    for (i, s) in samples.iter().enumerate() {
        let b = i * bins / samples.len().max(1);
        peaks[b] = peaks[b].max(s.abs());
    }
    Ok(peaks)
}
