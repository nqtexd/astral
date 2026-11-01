//! User-initiated, bounded microphone capture. No device is opened until Start.
use anyhow::{Context, Result, ensure};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
pub struct Recording {
    stream: cpal::Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    error: Arc<Mutex<Option<String>>>,
    pub rate: u32,
    pub channels: u16,
}
pub struct Capture {
    pub samples: Vec<f32>,
    pub rate: u32,
    pub channels: u16,
}
impl Recording {
    pub fn start() -> Result<Self> {
        let device = cpal::default_host()
            .default_input_device()
            .context("No microphone available")?;
        let supported = device.default_input_config()?;
        let config: cpal::StreamConfig = supported.clone().into();
        ensure!(
            (1..=2).contains(&config.channels) && config.sample_rate.0 <= 192000,
            "Unsupported microphone configuration"
        );
        let samples = Arc::new(Mutex::new(Vec::new()));
        let error = Arc::new(Mutex::new(None));
        let capacity = config.sample_rate.0 as usize * config.channels as usize * 60;
        macro_rules! build {
            ($t:ty,$convert:expr) => {{
                let output = samples.clone();
                let errors = error.clone();
                device.build_input_stream::<$t, _, _>(
                    &config,
                    move |data, _| {
                        if let Ok(mut out) = output.try_lock() {
                            let remaining = capacity.saturating_sub(out.len());
                            out.extend(data.iter().take(remaining).map($convert));
                        }
                    },
                    move |e| {
                        if let Ok(mut slot) = errors.lock() {
                            *slot = Some(e.to_string());
                        }
                    },
                    None,
                )?
            }};
        }
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => build!(f32, |v: &f32| v.clamp(-1., 1.)),
            cpal::SampleFormat::I16 => build!(i16, |v: &i16| *v as f32 / 32768.),
            cpal::SampleFormat::U16 => build!(u16, |v: &u16| (*v as f32 - 32768.) / 32768.),
            _ => anyhow::bail!("Choose a microphone using 16-bit PCM or float audio"),
        };
        stream.play()?;
        Ok(Self {
            stream,
            samples,
            error,
            rate: config.sample_rate.0,
            channels: config.channels,
        })
    }
    pub fn seconds(&self) -> f64 {
        self.samples.lock().map_or(0., |s| {
            s.len() as f64 / self.rate as f64 / self.channels as f64
        })
    }
    pub fn stop(self) -> Result<Vec<u8>> {
        let capture = self.finish()?;
        wav(&capture.samples, capture.rate, capture.channels)
    }
    pub fn finish(self) -> Result<Capture> {
        drop(self.stream);
        if let Some(error) = self
            .error
            .lock()
            .map_err(|_| anyhow::anyhow!("Capture unavailable"))?
            .take()
        {
            anyhow::bail!("Microphone disconnected: {error}");
        }
        let mut samples = self
            .samples
            .lock()
            .map_err(|_| anyhow::anyhow!("Capture unavailable"))?;
        ensure!(!samples.is_empty(), "No microphone samples captured");
        Ok(Capture {
            samples: std::mem::take(&mut *samples),
            rate: self.rate,
            channels: self.channels,
        })
    }
}
pub fn wav(samples: &[f32], rate: u32, channels: u16) -> Result<Vec<u8>> {
    ensure!(
        rate > 0
            && channels > 0
            && samples.len().is_multiple_of(channels as usize)
            && samples.iter().all(|s| s.is_finite()),
        "Invalid capture samples"
    );
    let mut output = std::io::Cursor::new(vec![]);
    {
        let mut writer = hound::WavWriter::new(
            &mut output,
            hound::WavSpec {
                channels,
                sample_rate: rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )?;
        for value in samples {
            writer.write_sample((value.clamp(-1., 1.) * 32767.).round() as i16)?;
        }
        writer.finalize()?;
    }
    Ok(output.into_inner())
}
