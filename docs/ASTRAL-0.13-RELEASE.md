# Astral 0.13.0 — Windows early access

Native artwork workflows and original editable animation studies.

- Import images, image sequences and supported 8-bit RGB PSD layers; paint individual bitmap frames and animate deformation pins.
- Extract short videos into editable images and preview local template-tracking camera proposals.
- Record desktop voice clips and create approximate, editable mouth cues from WAV audio/transcripts.
- Choose CPU or offscreen wgpu GPU export; reuse exact held frames without dropping video frames or shifting sound.
- Review shaped-text overflow and raster bounds, fit/place editable dialogue, and preview conservative motion polish.
- Convert supported legacy authored strokes/circles to drawing cels while retaining the original.
- Open the README's three real animation studies. The showcase ZIP includes editable projects, audio assets and full-timing MP4 previews; the fight is 60 FPS.

## Downloads

- **Windows x64 ZIP:** extract everything, then open `Astral-Studio.exe`. `astral.exe` provides CLI/MCP.
- **Showcase ZIP:** extract it and open projects from its `projects` folder. Keep the adjacent `assets` folder for audio; MP4 previews are in `previews`.
- **Android APK:** development touch subset for ARM64/x86-64, signed with a test identity. No physical device was connected for this revision.
- **Source ZIP and SHA256SUMS:** source and artifact verification. GPL-3.0-or-later, retained upstream credits and dependency notices are included.

FFmpeg is separately installed; it is not bundled. PNG export works without it. New raster/mouth-track projects use schema 8 and require Astral 0.13 or later.

## Evidence and scope

159 local workspace tests and strict Clippy passed. Real external MCP checks cover authoring, frame independence, revision rollback, previews, waveform/audio, export cancellation, project management and native resized PNG without FFmpeg. The new video operation was also exercised through MCP with four extracted/rendered frames. The release documentation includes actual CPU/GPU measurements, not a universal speedup claim.

Mac ARM, Intel and Apple simulator development builds passed on GitHub runners at the source commits recorded in [platform checks](https://github.com/nqtexd/astral/blob/v0.13.0/docs/ASTRAL-PLATFORM-CHECKS.md). Those artifacts have development scope; they are not production-signed/store releases or proof of physical device readiness.

**Full Moho Debut parity remains unfinished.** Advanced deformation/correctives, broader PSD/legacy semantics, pressure brushes, phoneme-level lip sync and other inventory gaps remain. Physical microphone/device tests, production signing and store publication are unfinished. Review tools cannot certify acting, composition or continuity. See the [workflow and limits](https://github.com/nqtexd/astral/blob/v0.13.0/docs/ASTRAL-ARTWORK.md) and [readiness inventory](https://github.com/nqtexd/astral/blob/v0.13.0/platforms/release/readiness.json).

Made with AI coding assistance; original project examples and AI provenance are documented in the repository.
