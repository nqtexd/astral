# Astral changelog

## [0.13.0]
Artwork, deformation and editable voice studies

- Add self-contained raster/image-sequence tracks, independent painted frames, animated transforms and mesh pins.
- Import bounded 8-bit RGB PSD layers/composites, preserve the source asset and report unsupported features.
- Extract short video clips to images; preview local template-tracking camera proposals before accepting.
- Record a bounded microphone clip on desktop; generate approximate, editable mouth cues from WAV audio and an optional transcript.
- Render exports with an offscreen physical wgpu GPU or bounded CPU workers; expose renderer, adapter and readback metrics.
- Propose motion polish without changing authored/contact anchors; keep review and acceptance explicit.
- Detect shaped-text overflow, fit/place dialogue and include deformed raster bounds in composition checks.
- Convert supported legacy authored lines/circles to native cels while retaining the original archive.
- Supply three real animated README examples and downloadable editable tester projects.
- Preserve schema upgrades across mixed feature edits; delete raster assets without losing shared references; keep paint edits independent.

Windows early access continues. This is not full Moho parity or a signed mobile/store release. See [workflow and limits](ASTRAL-ARTWORK.md).

## [0.12.0]
Project management and a more flexible studio

### Added
- Rename projects, inspect recent changes and move saved projects to a restorable Trash.
- See separate edited/opened times and revisions, with library refresh and name/date sorting.
- Customize accents, canvas surround, contrast, spacing, corners, frame cards, panels and recovery timing.
- Preview Flow through authored poses and clearer dialogue placement before accepting changes.
- Style editable dialogue borders, corners, speaker labels and dark captions.
- Assemble rendered shots and corrected endings with trims and synchronized audio.
- Inspect layout warnings in previews, review tools, validation and storyboard shot reviews.
- Select a separately installed encoder from Settings.

### Fixed
- A locked preview filename now produces a new version while preserving the viewed file.
- Structural validation no longer implies that a scene passed visual review.

### Performance
- Short exports can finish faster while retaining frame order and cancellation. Results depend on scene and encoder.

### Release scope
- Windows 11 x64 early access with source and dependency notices. Unsigned; FFmpeg is not bundled.
- Android remains a preview; Apple builds and full Moho Debut parity are unfinished.

Earlier notes: [development history](ASTRAL-DEVELOPMENT-HISTORY.md). The root changelog describes upstream StykAnimate, not this native release.
