//! Explicit, user-configured speech providers. Credentials never enter projects or MCP results.
use anyhow::{Context, Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    io::{Cursor, Read},
    sync::{Mutex, OnceLock},
    time::Duration,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    #[default]
    Offline,
    #[serde(rename = "elevenlabs", alias = "eleven_labs")]
    ElevenLabs,
    Compatible,
}
impl Provider {
    pub fn label(self) -> &'static str {
        match self {
            Self::Offline => "Windows offline",
            Self::ElevenLabs => "ElevenLabs",
            Self::Compatible => "Compatible speech API",
        }
    }
    fn credential(self, url: &str) -> Result<String> {
        Ok(match self {
            Self::Offline => "Astral/Voice/Offline".into(),
            Self::ElevenLabs => "Astral/Voice/ElevenLabs".into(),
            Self::Compatible => format!(
                "Astral/Voice/Compatible/{}",
                endpoint(url)?.origin().ascii_serialization()
            ),
        })
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub provider: Provider,
    /// Full /audio/speech endpoint, not an arbitrary command.
    pub endpoint: String,
    pub model: String,
    pub voice: String,
    pub network_enabled: bool,
    saved_profiles: std::collections::BTreeMap<String, Profile>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Profile {
    endpoint: String,
    model: String,
    voice: String,
}
impl Config {
    fn for_provider(&self, provider: Provider) -> Self {
        let mut config = self.clone();
        if provider != config.provider {
            let previous = config.provider;
            config.provider = provider;
            config.switch_from(previous);
        }
        config
    }
    pub fn switch_from(&mut self, previous: Provider) {
        if previous != Provider::Offline {
            self.saved_profiles.insert(
                previous.label().into(),
                Profile {
                    endpoint: self.endpoint.clone(),
                    model: self.model.clone(),
                    voice: self.voice.clone(),
                },
            );
        }
        if let Some(profile) = self.saved_profiles.get(self.provider.label()) {
            self.endpoint = profile.endpoint.clone();
            self.model = profile.model.clone();
            self.voice = profile.voice.clone();
        } else {
            self.endpoint = "https://api.openai.com/v1/audio/speech".into();
            self.model = if self.provider == Provider::ElevenLabs {
                "eleven_multilingual_v2".into()
            } else {
                String::new()
            };
            self.voice.clear();
        }
    }
}
impl Default for Config {
    fn default() -> Self {
        Self {
            provider: Provider::Offline,
            endpoint: "https://api.openai.com/v1/audio/speech".into(),
            model: String::new(),
            voice: String::new(),
            network_enabled: false,
            saved_profiles: Default::default(),
        }
    }
}
fn path() -> std::path::PathBuf {
    crate::export::encoder_settings_path().with_file_name("voice.json")
}
pub fn load() -> Config {
    std::fs::read(path())
        .ok()
        .filter(|b| b.len() < 16384)
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
pub fn save(config: &Config) -> Result<()> {
    if config.provider != Provider::Offline {
        ensure!(
            config.model.len() <= 256 && config.voice.len() <= 512,
            "Model or voice ID is too long"
        );
        if config.provider == Provider::Compatible {
            endpoint(&config.endpoint)?;
        }
    }
    let destination = path();
    astral_storage::Workspace::new(
        destination
            .parent()
            .context("Voice settings folder missing")?,
    )?
    .atomic_write("voice.json", &serde_json::to_vec_pretty(config)?)
}
fn session() -> &'static Mutex<std::collections::HashMap<String, String>> {
    static KEYS: OnceLock<Mutex<std::collections::HashMap<String, String>>> = OnceLock::new();
    KEYS.get_or_init(Default::default)
}
/// Windows uses Credential Manager; other platforms support session keys and environment variables.
pub fn set_key(provider: Provider, key: &str, endpoint: &str) -> Result<()> {
    let target_name = provider.credential(endpoint)?;
    ensure!(
        provider != Provider::Offline && key.len() <= 4096 && !key.chars().any(char::is_control),
        "Invalid API key"
    );
    #[cfg(windows)]
    {
        use windows::{Win32::Security::Credentials::*, core::PWSTR};
        let mut target: Vec<u16> = target_name.encode_utf16().chain(Some(0)).collect();
        if key.is_empty() {
            // Missing credential is already the requested state.
            let removed = unsafe {
                CredDeleteW(
                    windows::core::PCWSTR(target.as_ptr()),
                    CRED_TYPE_GENERIC,
                    None,
                )
            };
            if let Err(error) = removed
                && error.code()
                    != windows::core::HRESULT::from_win32(
                        windows::Win32::Foundation::ERROR_NOT_FOUND.0,
                    )
            {
                return Err(error).context("Remove voice key from Windows Credential Manager");
            }
        } else {
            let credential = CREDENTIALW {
                Type: CRED_TYPE_GENERIC,
                TargetName: PWSTR(target.as_mut_ptr()),
                CredentialBlobSize: key.len() as u32,
                CredentialBlob: key.as_ptr() as *mut u8,
                Persist: CRED_PERSIST_LOCAL_MACHINE,
                ..Default::default()
            };
            unsafe {
                CredWriteW(&credential, 0)
                    .context("Save voice key to Windows Credential Manager")?;
            }
        }
    }
    let mut keys = session()
        .lock()
        .map_err(|_| anyhow::anyhow!("Voice key store unavailable"))?;
    if key.is_empty() {
        keys.remove(&target_name);
    } else {
        keys.insert(target_name, key.into());
    }
    Ok(())
}
fn key(provider: Provider, url: &str) -> Option<String> {
    let target_name = provider.credential(url).ok()?;
    let variable = if provider == Provider::ElevenLabs {
        "ELEVENLABS_API_KEY"
    } else {
        "ASTRAL_VOICE_API_KEY"
    };
    if let Ok(value) = std::env::var(variable)
        && !value.is_empty()
    {
        return Some(value);
    }
    if let Some(value) = session().lock().ok()?.get(&target_name) {
        return Some(value.clone());
    }
    #[cfg(windows)]
    {
        use windows::Win32::Security::Credentials::*;
        let target = windows::core::HSTRING::from(target_name);
        let mut credential = std::ptr::null_mut();
        if unsafe { CredReadW(&target, CRED_TYPE_GENERIC, None, &mut credential) }.is_ok() {
            let result = unsafe {
                let c = &*credential;
                let out = if c.CredentialBlobSize <= 4096 {
                    String::from_utf8(
                        std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize)
                            .to_vec(),
                    )
                    .ok()
                } else {
                    None
                };
                CredFree(credential.cast());
                out
            };
            return result;
        }
    }
    None
}
pub fn has_key(provider: Provider, url: &str) -> bool {
    key(provider, url).is_some()
}
fn endpoint(value: &str) -> Result<reqwest::Url> {
    ensure!(value.len() <= 2048, "Speech endpoint is too long");
    let url = reqwest::Url::parse(value).context("Enter a full speech API URL")?;
    ensure!(
        url.username().is_empty() && url.password().is_none() && url.fragment().is_none(),
        "Speech URL cannot contain credentials or fragments"
    );
    let local = matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    );
    ensure!(
        url.scheme() == "https" || (local && url.scheme() == "http"),
        "Speech API requires HTTPS; HTTP is allowed only for a local server"
    );
    ensure!(
        url.query().is_none(),
        "Put API keys in the key field, never in the URL"
    );
    Ok(url)
}
pub fn synthesize(
    text: &str,
    options: &crate::voice::VoiceOptions,
    config: &Config,
    cancelled: impl Fn() -> bool,
) -> Result<Vec<u8>> {
    let provider = options.provider.unwrap_or(config.provider);
    let config = config.for_provider(provider);
    ensure!(
        provider != Provider::Compatible || text.chars().count() <= 4096,
        "Compatible speech requests are limited to 4096 characters"
    );
    ensure!(
        config.network_enabled,
        "Enable external speech requests in Settings → Voice first"
    );
    ensure!(
        (options.pitch - 1.).abs() < 0.001,
        "External speech does not support Astral's offline pitch control; use pitch 1.0"
    );
    ensure!(!cancelled(), "Speech generation cancelled");
    let voice = options
        .voice
        .as_deref()
        .filter(|v| !v.is_empty())
        .unwrap_or(&config.voice);
    ensure!(
        !voice.trim().is_empty() && !config.model.trim().is_empty(),
        "Set the provider model and voice IDs in Settings → Voice"
    );
    if provider == Provider::ElevenLabs {
        ensure!(
            (0.7..=1.2).contains(&options.rate),
            "ElevenLabs speed must be between 0.7 and 1.2"
        );
    }
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let mut request = if provider == Provider::ElevenLabs {
        ensure!(
            voice
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
            "Invalid ElevenLabs voice ID"
        );
        let secret = key(provider, &config.endpoint)
            .context("Add an ElevenLabs API key in Settings → Voice")?;
        client.post(format!("https://api.elevenlabs.io/v1/text-to-speech/{voice}?output_format=pcm_24000")).header("xi-api-key",secret).json(&serde_json::json!({"text":text,"model_id":config.model,"voice_settings":{"speed":options.rate}}))
    } else {
        client.post(endpoint(&config.endpoint)?).json(&serde_json::json!({"input":text,"model":config.model,"voice":voice,"response_format":"wav","speed":options.rate}))
    };
    if provider == Provider::Compatible
        && let Some(secret) = key(provider, &config.endpoint)
    {
        request = request.bearer_auth(secret);
    }
    let response = request.send().map_err(|_| {
        anyhow::anyhow!("Speech request failed or timed out; check the endpoint and connection")
    })?;
    let status = response.status();
    // Do not echo provider bodies: they may contain keys, text or account details.
    ensure!(
        status.is_success(),
        "Speech provider returned HTTP {}. Check the key, model, voice and provider quota.",
        status.as_u16()
    );
    ensure!(
        response
            .content_length()
            .is_none_or(|n| n <= 64 * 1024 * 1024),
        "Speech response exceeds 64 MiB"
    );
    let mut data = Vec::new();
    let mut reader = response.take(64 * 1024 * 1024 + 1);
    let mut block = [0; 16384];
    loop {
        ensure!(!cancelled(), "Speech generation cancelled");
        let n = reader.read(&mut block).context("Read speech audio")?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&block[..n]);
        ensure!(
            data.len() <= 64 * 1024 * 1024,
            "Speech response exceeds 64 MiB"
        );
    }
    let bytes = if provider == Provider::ElevenLabs {
        pcm_wav(&data, 24000)?
    } else {
        data
    };
    apply_gain(&bytes, options.volume)
}
fn pcm_wav(pcm: &[u8], sample_rate: u32) -> Result<Vec<u8>> {
    ensure!(
        !pcm.is_empty() && pcm.len().is_multiple_of(2),
        "Provider returned invalid PCM audio"
    );
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(
            &mut bytes,
            hound::WavSpec {
                channels: 1,
                sample_rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )?;
        for v in pcm.as_chunks::<2>().0 {
            writer.write_sample(i16::from_le_bytes([v[0], v[1]]))?;
        }
        writer.finalize()?;
    }
    Ok(bytes.into_inner())
}
fn apply_gain(bytes: &[u8], volume: f64) -> Result<Vec<u8>> {
    let pcm = astral_audio::decode(bytes).context("Speech provider returned unsupported audio")?;
    let mut out = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(
            &mut out,
            hound::WavSpec {
                channels: pcm.channels as u16,
                sample_rate: pcm.rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )?;
        for sample in pcm.samples {
            writer.write_sample(
                (sample as f64 * volume * 32768.)
                    .round()
                    .clamp(i16::MIN as f64, i16::MAX as f64) as i16,
            )?;
        }
        writer.finalize()?;
    }
    Ok(out.into_inner())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_profiles_and_credentials_are_scoped() {
        let mut config = Config {
            provider: Provider::ElevenLabs,
            voice: "speaker-1".into(),
            model: "model-1".into(),
            ..Default::default()
        };
        config.provider = Provider::Compatible;
        config.switch_from(Provider::ElevenLabs);
        config.voice = "local".into();
        config.model = "kokoro".into();
        let eleven = config.for_provider(Provider::ElevenLabs);
        assert_eq!(eleven.voice, "speaker-1");
        assert_eq!(eleven.model, "model-1");
        assert_eq!(config.model, "kokoro");
        assert_ne!(
            Provider::Compatible
                .credential("https://a.example/v1/audio/speech")
                .unwrap(),
            Provider::Compatible
                .credential("https://b.example/v1/audio/speech")
                .unwrap()
        );
        assert_eq!(
            Provider::Compatible
                .credential("https://a.example/voice")
                .unwrap(),
            Provider::Compatible
                .credential("https://a.example/v1/audio/speech")
                .unwrap()
        );
    }
    #[test]
    fn rejects_leaky_and_insecure_endpoints() {
        for url in [
            "http://example.com/speech",
            "https://key@example.com/speech",
            "https://example.com/speech?key=secret",
            "file:///voice.wav",
        ] {
            assert!(endpoint(url).is_err());
        }
        assert!(endpoint("http://127.0.0.1:8880/v1/audio/speech").is_ok());
    }
    #[test]
    fn pcm_roundtrip_and_gain() {
        let wav = pcm_wav(&[0x10, 0x27, 0xf0, 0xd8], 24000).unwrap();
        let scaled = apply_gain(&wav, 0.5).unwrap();
        let samples: Vec<i16> = hound::WavReader::new(Cursor::new(scaled))
            .unwrap()
            .samples()
            .map(Result::unwrap)
            .collect();
        assert_eq!(samples, [5000, -5000]);
        assert!(apply_gain(b"not audio", 1.).is_err());
        assert!(pcm_wav(&[1], 24000).is_err());
    }
    #[test]
    fn compatible_server_receives_speech_and_returns_editable_wav() {
        use std::{io::Write, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/v1/audio/speech", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut buf = [0; 4096];
            loop {
                let n = socket.read(&mut buf).unwrap();
                request.extend_from_slice(&buf[..n]);
                if let Some(i) = request.windows(4).position(|b| b == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&request[..i]);
                    let len: usize = header
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|n| n.parse().ok())
                        })
                        .unwrap();
                    if request.len() >= i + 4 + len {
                        let body: serde_json::Value =
                            serde_json::from_slice(&request[i + 4..]).unwrap();
                        assert_eq!(body["input"], "Hello");
                        assert_eq!(body["response_format"], "wav");
                        break;
                    }
                }
            }
            let wav = pcm_wav(&[0, 0, 0, 0], 24000).unwrap();
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                wav.len()
            )
            .unwrap();
            socket.write_all(&wav).unwrap();
        });
        let config = Config {
            provider: Provider::Compatible,
            endpoint,
            model: "local-model".into(),
            voice: "speaker".into(),
            network_enabled: true,
            saved_profiles: Default::default(),
        };
        let bytes = synthesize("Hello", &Default::default(), &config, || false).unwrap();
        assert!(bytes.starts_with(b"RIFF"));
        server.join().unwrap();
    }
}
