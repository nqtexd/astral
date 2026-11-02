use super::ExportSpec;
use anyhow::{Result, ensure};
use astral_core::Project;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, clap::ValueEnum,
)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    #[default]
    Original,
    InstagramReels,
    InstagramStories,
    InstagramPortrait,
    InstagramSquare,
    #[value(name = "youtube-1080p", alias = "youtube1080p")]
    #[serde(rename = "youtube_1080p")]
    Youtube1080p,
    #[value(name = "youtube-4k", alias = "youtube4k")]
    #[serde(rename = "youtube_4k")]
    Youtube4k,
    YoutubeShorts,
    Tiktok,
}
impl Preset {
    pub const ALL: [Self; 9] = [
        Self::Original,
        Self::InstagramReels,
        Self::InstagramStories,
        Self::InstagramPortrait,
        Self::InstagramSquare,
        Self::Youtube1080p,
        Self::Youtube4k,
        Self::YoutubeShorts,
        Self::Tiktok,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Original => "Project original",
            Self::InstagramReels => "Instagram Reels",
            Self::InstagramStories => "Instagram Stories",
            Self::InstagramPortrait => "Instagram portrait",
            Self::InstagramSquare => "Instagram square",
            Self::Youtube1080p => "YouTube 1080p",
            Self::Youtube4k => "YouTube 4K",
            Self::YoutubeShorts => "YouTube Shorts",
            Self::Tiktok => "TikTok",
        }
    }
    pub fn dimensions(self, p: &Project) -> (u32, u32) {
        match self {
            Self::Original => (p.width, p.height),
            Self::InstagramReels | Self::InstagramStories | Self::YoutubeShorts | Self::Tiktok => {
                (1080, 1920)
            }
            Self::InstagramPortrait => (1080, 1350),
            Self::InstagramSquare => (1080, 1080),
            Self::Youtube1080p => (1920, 1080),
            Self::Youtube4k => (3840, 2160),
        }
    }
    pub fn fps(self, p: &Project) -> u32 {
        match self {
            Self::Original | Self::Youtube1080p | Self::Youtube4k => p.fps,
            _ => 30,
        }
    }
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, clap::ValueEnum,
)]
#[serde(rename_all = "snake_case")]
pub enum Framing {
    #[default]
    Fit,
    Fill,
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, clap::ValueEnum,
)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Draft,
    #[default]
    Standard,
    High,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Avi,
    Mp4,
    Webm,
    Gif,
    Png,
    Jpeg,
    PngSequence,
    Wav,
}
impl Format {
    pub const ALL: [Self; 8] = [
        Self::Avi,
        Self::Mp4,
        Self::Webm,
        Self::Gif,
        Self::Png,
        Self::Jpeg,
        Self::PngSequence,
        Self::Wav,
    ];
    pub fn extension(self) -> &'static str {
        match self {
            Self::Avi => "avi",
            Self::Mp4 => "mp4",
            Self::Webm => "webm",
            Self::Gif => "gif",
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::PngSequence => "zip",
            Self::Wav => "wav",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Avi => "AVI · native MJPEG + PCM (no FFmpeg)",
            Self::Mp4 => "MP4 · H.264 + AAC",
            Self::Webm => "WebM · VP9 + Opus",
            Self::Gif => "Animated GIF · silent",
            Self::Png => "PNG · current frame",
            Self::Jpeg => "JPEG · current frame",
            Self::PngSequence => "PNG sequence + audio · ZIP",
            Self::Wav => "WAV · audio mix",
        }
    }
    pub fn still(self) -> bool {
        matches!(self, Self::Png | Self::Jpeg)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub frames: u32,
    pub first: u32,
    pub last: u32,
    pub format: Format,
    pub video_kbps: u32,
    pub audio_kbps: u32,
    pub warnings: Vec<String>,
    pub duration_seconds: f64,
}
impl Plan {
    pub fn new(p: &Project, s: &ExportSpec) -> Result<Self> {
        let ext = std::path::Path::new(&s.path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let format = match ext.as_str() {
            "avi" => Format::Avi,
            "mp4" => Format::Mp4,
            "webm" => Format::Webm,
            "gif" => Format::Gif,
            "png" => Format::Png,
            "jpg" | "jpeg" => Format::Jpeg,
            "zip" => Format::PngSequence,
            "wav" => Format::Wav,
            _ => anyhow::bail!("Use .avi, .mp4, .webm, .gif, .png, .jpg, .zip or .wav"),
        };
        let first = s.start.unwrap_or(0);
        let last = s.end.unwrap_or(p.duration - 1);
        ensure!(first <= last && last < p.duration, "Invalid export range");
        let (mut width, mut height) = s.preset.dimensions(p);
        if format == Format::Gif && s.width.is_none() && s.height.is_none() {
            let scale = (720. / width.max(height) as f64).min(1.);
            width = (width as f64 * scale).round().max(1.) as u32;
            height = (height as f64 * scale).round().max(1.) as u32;
        }
        width = s.width.unwrap_or(width);
        height = s.height.unwrap_or(height);
        ensure!(
            (1..=8192).contains(&width)
                && (1..=8192).contains(&height)
                && width as u64 * height as u64 <= 32 * 1024 * 1024,
            "Export dimensions exceed 8192 or 32 megapixels"
        );
        if matches!(format, Format::Mp4 | Format::Webm) {
            ensure!(
                width.is_multiple_of(2) && height.is_multiple_of(2),
                "Video output dimensions must be even"
            );
        }
        let fps = s.fps.unwrap_or(if format == Format::Gif {
            15
        } else {
            s.preset.fps(p)
        });
        ensure!((1..=240).contains(&fps), "Export fps must be 1–240");
        ensure!(
            format != Format::Gif || fps <= 50,
            "GIF export is limited to 50 fps"
        );
        let duration_seconds = (last - first + 1) as f64 / p.fps as f64;
        let frames = if format.still() {
            1
        } else {
            (duration_seconds * fps as f64 - 1e-9).ceil().max(1.) as u32
        };
        ensure!(
            frames <= 1_000_000,
            "Export exceeds one million output frames; split the selected range"
        );
        let base = if width.max(height) >= 3840 {
            if fps > 30 { 60000 } else { 40000 }
        } else if fps > 30 {
            12000
        } else {
            8000
        };
        let video_kbps = match s.quality {
            Quality::Draft => base / 2,
            Quality::Standard => base,
            Quality::High => base * 3 / 2,
        };
        let audio_kbps = match s.preset {
            Preset::InstagramReels
            | Preset::InstagramStories
            | Preset::InstagramPortrait
            | Preset::InstagramSquare => 128,
            Preset::Youtube1080p | Preset::Youtube4k | Preset::YoutubeShorts => 384,
            _ => 192,
        };
        let mut warnings = vec![];
        if s.preset == Preset::YoutubeShorts && (duration_seconds > 180. || width > height) {
            warnings.push("Shorts recognition requires a square or vertical video of at most three minutes. This export keeps the full selected range.".into());
        }
        if s.preset == Preset::InstagramStories && duration_seconds > 60. {
            warnings.push("Long Stories may be split or limited by the publishing route; the selected range is preserved.".into());
        }
        if format == Format::Gif && !p.audio.is_empty() {
            warnings.push("GIF has no audio; choose MP4 or WebM to include sound.".into());
        }
        if !matches!(s.preset, Preset::Original) && format != Format::Mp4 {
            warnings.push("Social presets describe framing. MP4 is the recommended delivery format for social uploads.".into());
        }
        if !matches!(
            s.preset,
            Preset::Original | Preset::Youtube1080p | Preset::Youtube4k
        ) && !(23..=60).contains(&fps)
        {
            warnings.push("This social preset normally uses 23–60 fps. Custom frame rate retained; check your publishing route.".into());
        }
        Ok(Self {
            width,
            height,
            fps,
            frames,
            first,
            last,
            format,
            video_kbps,
            audio_kbps,
            warnings,
            duration_seconds,
        })
    }
    pub fn source_frame(&self, p: &Project, index: u32) -> f64 {
        (self.first as f64 + index as f64 * p.fps as f64 / self.fps as f64).min(self.last as f64)
    }
}

pub fn catalog() -> serde_json::Value {
    serde_json::json!({"presets":Preset::ALL.map(|preset|{
        let dimensions=match preset {
            Preset::Original=>None,
            Preset::InstagramReels|Preset::InstagramStories|Preset::YoutubeShorts|Preset::Tiktok=>Some([1080,1920]),
            Preset::InstagramPortrait=>Some([1080,1350]),Preset::InstagramSquare=>Some([1080,1080]),
            Preset::Youtube1080p=>Some([1920,1080]),Preset::Youtube4k=>Some([3840,2160]),
        };
        let fps=match preset {Preset::Original|Preset::Youtube1080p|Preset::Youtube4k=>None,_=>Some(30)};
        serde_json::json!({"id":preset,"name":preset.label(),"dimensions":dimensions,"fps":fps,"inherits_source_fps":fps.is_none()})
    }),"formats":Format::ALL.map(|format|serde_json::json!({"id":format,"name":format.label(),"extension":format.extension()})),"framing":["fit","fill"],"quality":["draft","standard","high"]})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn social_timing_and_validation() {
        let mut p = astral_core::fight_demo().unwrap();
        p.fps = 24;
        p.duration = 240;
        let s = ExportSpec {
            path: "reels.mp4".into(),
            preset: Preset::InstagramReels,
            ..Default::default()
        };
        let plan = Plan::new(&p, &s).unwrap();
        assert_eq!(
            (plan.width, plan.height, plan.fps, plan.frames),
            (1080, 1920, 30, 300)
        );
        assert!((plan.source_frame(&p, 150) - 120.).abs() < 1e-9);
        assert_eq!(plan.source_frame(&p, 299), 239.);
        assert!(
            Plan::new(
                &p,
                &ExportSpec {
                    width: Some(1079),
                    ..s
                }
            )
            .is_err()
        );
    }
    #[test]
    fn formats_ranges_and_gif_defaults() {
        let p = astral_core::fight_demo().unwrap();
        let s = ExportSpec {
            path: "clip.GIF".into(),
            start: Some(12),
            end: Some(23),
            ..Default::default()
        };
        let plan = Plan::new(&p, &s).unwrap();
        assert!(plan.width.max(plan.height) <= 720);
        assert_eq!(plan.fps, 15);
        assert_eq!(plan.first, 12);
        assert!(
            Plan::new(
                &p,
                &ExportSpec {
                    fps: Some(60),
                    ..s.clone()
                }
            )
            .is_err()
        );
        assert!(
            Plan::new(
                &p,
                &ExportSpec {
                    end: Some(p.duration),
                    ..s.clone()
                }
            )
            .is_err()
        );
        assert!(
            Plan::new(
                &p,
                &ExportSpec {
                    path: "bad.exe".into(),
                    ..s
                }
            )
            .is_err()
        );
    }
}
