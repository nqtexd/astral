# Astral media export

Open **Export studio** in the desktop header. Select a destination and format, inspect the framing preview, and choose **Render export**. The render queue reports progress, warnings and errors and lets you cancel. Editing can continue while a snapshot of the document exports. Existing output is replaced only after success. Paths are relative to the configured Astral workspace.

## Destination defaults

| Destination | Resolution | Frame rate | Standard video / audio |
|---|---|---|---|
| Project original | Project dimensions | Project fps | 8 Mbps / 192 kbps at up to 1080p30 |
| Instagram Reels / Stories | 1080 × 1920 | 30 fps | 8 Mbps / 128 kbps |
| Instagram portrait | 1080 × 1350 | 30 fps | 8 Mbps / 128 kbps |
| Instagram square | 1080 × 1080 | 30 fps | 8 Mbps / 128 kbps |
| YouTube 1080p | 1920 × 1080 | Project fps | 8 Mbps through 30 fps; 12 Mbps above / 384 kbps |
| YouTube 4K | 3840 × 2160 | Project fps | 40 Mbps through 30 fps; 60 Mbps above / 384 kbps |
| YouTube Shorts | 1080 × 1920 | 30 fps | 8 Mbps / 384 kbps |
| TikTok | 1080 × 1920 | 30 fps | 8 Mbps / 192 kbps |

MP4 uses progressive H.264 High, 4:2:0 video, BT.709 metadata, fast-start layout, and AAC stereo at 48 kHz when sound is present. Vectors are rasterized directly at the delivery resolution; video conversion uses a BT.709 matrix. Bitrates are encoder targets; static animation often compresses below the target. Draft halves video bitrate and uses a faster software preset; High increases bitrate by 50%. Hardware mode trial-encodes the requested dimensions and frame rate with NVENC, QSV and AMF, then falls back to libx264 if trials fail.

**Fit** preserves the entire scene with black letterboxing. **Fill** crops the edges to fill the destination. Neither stretches the animation. The preview uses the current playhead and shows the same centered composition. Custom dimensions, frame rate, quality and inclusive source-frame range can be set in the export window. Export size is capped at 8192 pixels per edge and 32 megapixels; MP4/WebM dimensions must be even.

Instagram portrait and square are composition choices. Current publishing routes may present video as Reels; these presets do not select an Instagram publishing surface. Stories over 60 seconds and Shorts over three minutes or with horizontal custom dimensions produce advisory warnings. Astral preserves the selected range rather than trimming it automatically. Duration and upload limits vary by publishing route and account.

## File formats

- `.mp4`: social delivery, H.264 + AAC, optional hardware encoding.
- `.webm`: VP9 + Opus for web playback, software encoding.
- `.gif`: silent looping animation, per-frame palette, default maximum edge 720 and 15 fps; explicit rates up to 50 fps.
- `.png` and `.jpg` / `.jpeg`: first frame of the selected range; the desktop selects the current frame. PNG retains source alpha within the scene; fit padding is black. JPEG is opaque.
- `.zip`: PNG sequence under `frames/frame_000000.png`, a `manifest.json` with dimensions, fps, range, document ID and revision, and a separate mixed `audio.wav` when sound is present. ZIP uses stored compression because PNG is already compressed.
- `.wav`: the selected range's stereo 48 kHz audio mix, including silence where no clips play.

Frame-rate conversion evaluates the editable animation at fractional source times. The selected final source frame is held when needed. Output frame count rounds up to cover the selected duration, so video length may exceed the source range by less than one output frame. Audio is mixed on the source timeline. WebM currently uses opaque 4:2:0 output. H.264/WebM/GIF/sequence and resized still-image outputs require FFmpeg; original-size PNG uses the native renderer directly. Offscreen drawing is CPU rasterized before encoding.

## CLI and agents

```powershell
astral --workspace D:/AstralProjects export-presets
astral --workspace D:/AstralProjects export motion-lab.astral renders/reel.mp4 --preset instagram-reels --framing fit --hardware
astral --workspace D:/AstralProjects export motion-lab.astral renders/short.mp4 --preset youtube-shorts --framing fill --start 30 --end 299
astral --workspace D:/AstralProjects export motion-lab.astral renders/frames.zip --fps 60
astral --workspace D:/AstralProjects export motion-lab.astral renders/preview.gif --width 640 --height 360
astral --workspace D:/AstralProjects export motion-lab.astral renders/mix.wav
```

MCP exposes `list_export_presets` and `export_media`. Output extension chooses the format. Preset IDs use underscores: `original`, `instagram_reels`, `instagram_stories`, `instagram_portrait`, `instagram_square`, `youtube_1080p`, `youtube_4k`, `youtube_shorts`, `tiktok`. CLI names use hyphens as shown above. The older `export_video` tool is retained. Both use the same export engine. Poll `job_status` or call `cancel_job`.

```json
{
  "project": "PROJECT_UUID",
  "spec": {
    "path": "renders/reel.mp4",
    "preset": "instagram_reels",
    "framing": "fit",
    "quality": "standard",
    "hardware": true,
    "start": 0,
    "end": 299
  }
}
```

## Requirements research

Defaults were checked on 28 October 2026 against official documentation. Astral's defaults are delivery choices, not guarantees that a particular account or publishing API accepts every customized file.

- [Instagram Reel size and aspect ratio](https://help.instagram.com/1038071743007909): public guidance includes at least 30 fps and 720 pixels. Astral chooses 1080 × 1920 at 30 fps.
- [Meta Instagram media reference](https://developers.facebook.com/documentation/instagram-platform/instagram-graph-api/reference/ig-user/media): publishing-route-specific codec, bitrate and duration constraints; API limits are not imposed as general manual-export limits.
- [YouTube recommended upload encoding settings](https://support.google.com/youtube/answer/1722171?hl=en): H.264, AAC at 48 kHz, source frame rate, progressive scan, BT.709 and resolution-dependent bitrates.
- [YouTube three-minute Shorts](https://support.google.com/youtube/answer/15424877?hl=en): square or vertical videos up to three minutes.
- [TikTok media transfer guide](https://developers.tiktok.com/docs/en/content-posting-api-media-transfer-guide?enter_method=left_navigation): codec and frame-rate guidance; account-dependent publishing duration.
- [FFmpeg codecs](https://ffmpeg.org/ffmpeg-codecs.html) and [filters](https://ffmpeg.org/ffmpeg-filters.html): VP9/Opus, scaling, padding, cropping and GIF palette generation.

Run `python scripts/smoke-astral-exports.py` with Pillow installed for actual encoding/decoding, all social preset sizes, framing pixels, frame counts, audio and ZIP integrity checks. Run `node scripts/smoke-astral.mjs` for the MCP authoring/export workflow. `ASTRAL_BINARY` selects a packaged executable for either smoke script.
