# Astral changelog

## Unreleased — comic 2D workflows

- Add editable RGB split, seeded time-varying glitch bands and rotated luminance halftone processing on arbitrary composited artwork, shared by preview/export.
- Animate effect strength with validated linear keys; expose desktop sliders and playhead key controls.
- Add independently stepped drawing/raster/graphic/character cadence, keeping camera and audio on the project clock.
- Add MCP `comic_tools` for validated look proposals, actual frame images and bounded pixel-difference refinement with revision checks and undo.
- Add schema 12 validation, alpha/cadence/cache/transaction regressions and an original editable illustrated courier demo. [Workflow and research](ASTRAL-COMIC-2D.md).

## [0.15.0]
Native editing, finishing and production gates

- Add independent root/joint Hermite channels with draggable time/value keys, numeric slopes, timing-curve handles and transactional undo.
- Add a virtualized exposure sheet with range holds/blanks and dialogue/audio/marker notes; add live shared-library instances, transform keys and bounded nested local time.
- Add waveform seek/trim/slip and sample-continuous split commands outside fade ranges; retain explicit reviewed draft application.
- Add stable-ID media relinking and cancellable, hash-verified `.astralpack` packaging/extraction with no-overwrite destinations and confined paths.
- Preserve higher-revision recovery snapshots, prefer newer autosaves on ordinary opens and keep recovered documents unsaved until explicitly saved.
- Add opt-in drawing stabilization, alpha-preserving clipping nodes, scene-input composition controls, RGB ICC normalization and sRGB-tagged PNG/JPEG exports.
- Add a signer-pinned Windows update verification script that refuses unsigned installers; actual release signing remains blocked by the missing certificate.
- Add editable compositing DAGs, bitmap/luminance masks, supported blend modes, gradients, blur and color adjustments; reject cycles/missing references/oversized graphs atomically.
- Add advanced 8-bit RGB PSD decoding into native groups/masks/blends/basic text, with an isolated pinned parser runtime in Windows packages and explicit conversion warnings.
- Add offline English/phonetic viseme recognition, nine source mouth codes, editable keys and bitmap mouth assignments; retain envelope timing as an explicit compatibility mode.
- Add pressure-aware procedural textured brushes and persistent custom brush presets; Boolean path operations preserve original shapes.
- Add built-in AVI/MJPEG+PCM, JPEG, PNG-sequence and WAV export paths; avoid FFmpeg for these formats.
- Stream export audio in bounded blocks, preserving original trim/speed/fade phase; bound long ranges and native AVI output.
- Retain five distinct prior saves, hide backups from recent-project discovery, validate new image payloads before commit and expose non-mutating MCP transaction previews.
- Catch background-job panics, retain bounded local diagnostics and add stress/readiness/installer tooling. Physical device, signing, complete localization and eight-hour desktop gates remain open.

See [the 0.15 workflow](ASTRAL-0.15-WORKFLOW.md). This is not full Moho/Photoshop/legacy-source parity or production certification.

## [0.14.0]
Native editing and configurable voice providers

- Add pressure samples, marker/dashed/stipple brushes, closed fills, cubic smoothing and vector-node editing.
- Add MLS similarity deformation, cubic curvers and bone-angle-driven raster corrective offsets to shared evaluation/rendering.
- Migrate legacy geometry through the established source evaluator at project FPS; convert supported shapes, joins, source in-betweens, native pose controls, skinned cels and embedded image quads.
- Import embedded legacy audio with native tracks, gain/mute/solo results, fades and trims; retain complete original data in immutable shared archives and omit it from MCP inspection results.
- Add native WAV/AIFF/MP3/M4A decoding, full-length background audio import, pitch/speed and waveform alignment.
- Add optional ElevenLabs and compatible speech endpoints to Settings → Voice, with provider profiles, explicit external-request opt-in and Windows Credential Manager storage scoped to provider/origin.
- Add object-following tracking proposals with optional second-point rotation/scale, preserving adjacent frames.
- Add an 18-action shortcut editor and English/Spanish/French core-control catalogs.
- Use schema 9 for advanced features; migrate old inline legacy archives when a document enters the command service.

This extends the parity work. It does not establish full Moho equivalence, lossless source semantics, all-language localization or physically certified tablet input. See [the 0.14 workflow](ASTRAL-0.14-WORKFLOW.md).

## [0.13.0]
Artwork, deformation and editable voice studies

- Add self-contained raster/image-sequence tracks, independent painted frames, animated transforms and mesh pins.
- Import bounded 8-bit RGB PSD layers/composites, preserve the source asset and report unsupported features.
- Extract short video clips to images; preview local template-tracking camera proposals before accepting.
- Record a bounded microphone clip on desktop; generate approximate, editable mouth cues from WAV audio and an optional transcript.
- Reuse unchanged evaluated frames while retaining every video frame/audio cue; invalidate on geometry, image, colour, size or framing changes.
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
