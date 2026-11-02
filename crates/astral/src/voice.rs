use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Voice {
    pub id: String,
    pub name: String,
    pub language: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct VoiceOptions {
    pub provider: Option<crate::voice_provider::Provider>,
    pub voice: Option<String>,
    pub rate: f64,
    pub pitch: f64,
    pub volume: f64,
}
impl Default for VoiceOptions {
    fn default() -> Self {
        Self {
            provider: None,
            voice: None,
            rate: 1.,
            pitch: 1.,
            volume: 1.,
        }
    }
}
#[cfg(windows)]
fn initialized<T>(action: impl FnOnce() -> Result<T>) -> Result<T> {
    use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
    // Called only on fresh worker threads, and uninitialized after every operation.
    unsafe {
        RoInitialize(RO_INIT_MULTITHREADED)?;
    }
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                RoUninitialize();
            }
        }
    }
    let _apartment = Apartment;
    action()
}
#[cfg(windows)]
pub fn voices() -> Result<Vec<Voice>> {
    initialized(|| {
        let list = windows::Media::SpeechSynthesis::SpeechSynthesizer::AllVoices()?;
        let mut out = vec![];
        for index in 0..list.Size()? {
            let v = list.GetAt(index)?;
            out.push(Voice {
                id: v.Id()?.to_string(),
                name: v.DisplayName()?.to_string(),
                language: v.Language()?.to_string(),
            });
        }
        Ok(out)
    })
}
#[cfg(not(windows))]
pub fn voices() -> Result<Vec<Voice>> {
    anyhow::bail!("Offline speech currently requires Windows")
}
pub fn synthesize(text: &str, options: &VoiceOptions) -> Result<Vec<u8>> {
    synthesize_cancellable(text, options, || false)
}
pub fn synthesize_cancellable(
    text: &str,
    options: &VoiceOptions,
    cancelled: impl Fn() -> bool,
) -> Result<Vec<u8>> {
    let config = crate::voice_provider::load();
    synthesize_with_config(text, options, &config, cancelled)
}
pub fn synthesize_with_config(
    text: &str,
    options: &VoiceOptions,
    config: &crate::voice_provider::Config,
    cancelled: impl Fn() -> bool,
) -> Result<Vec<u8>> {
    ensure!(
        !text.trim().is_empty() && text.chars().count() <= 5000,
        "Voice text must contain 1–5000 characters"
    );
    ensure!(
        options.rate.is_finite()
            && (0.5..=2.).contains(&options.rate)
            && options.pitch.is_finite()
            && (0.5..=2.).contains(&options.pitch)
            && options.volume.is_finite()
            && (0.0..=1.).contains(&options.volume),
        "Invalid speech rate, pitch or volume"
    );
    if options.provider.unwrap_or(config.provider) != crate::voice_provider::Provider::Offline {
        return crate::voice_provider::synthesize(text, options, config, cancelled);
    }
    #[cfg(windows)]
    {
        use windows::{
            Media::SpeechSynthesis::SpeechSynthesizer, Storage::Streams::DataReader, core::HSTRING,
        };
        initialized(|| {
            let synth = SpeechSynthesizer::new()?;
            if let Some(id) = &options.voice {
                let voices = SpeechSynthesizer::AllVoices()?;
                let mut found = false;
                for index in 0..voices.Size()? {
                    let voice = voices.GetAt(index)?;
                    if voice.Id()? == id.as_str() {
                        synth.SetVoice(&voice)?;
                        found = true;
                        break;
                    }
                }
                ensure!(found, "The selected Windows voice is no longer installed");
            }
            let settings = synth.Options()?;
            settings.SetSpeakingRate(options.rate)?;
            settings.SetAudioPitch(options.pitch)?;
            settings.SetAudioVolume(options.volume)?;
            let operation = synth.SynthesizeTextToStreamAsync(&HSTRING::from(text))?;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
            while operation.Status()?.0 == 0 {
                if cancelled() || std::time::Instant::now() > deadline {
                    let _ = operation.Cancel();
                    anyhow::bail!("Speech synthesis cancelled or timed out");
                }
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            let stream = operation.join()?;
            let length = stream.Size()?;
            ensure!(
                length > 0 && length <= 64 * 1024 * 1024,
                "Synthesized voice exceeds 64 MiB"
            );
            let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
            let loaded = reader.LoadAsync(length as u32)?.join()?;
            ensure!(loaded == length as u32, "Speech stream was truncated");
            let mut bytes = vec![0; length as usize];
            reader.ReadBytes(&mut bytes)?;
            Ok(bytes)
        })
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("Offline speech currently requires Windows")
    }
}
