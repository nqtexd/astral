# Astral implementation status

Astral 0.13 adds self-contained raster/image sequences, independent bitmap cels, basic PSD layers, animated deformation pins, bounded video extraction/template tracking, microphone capture, editable mouth cues and offscreen wgpu GPU exports. It retains the earlier project, storyboard, rig, drawing, audio and MCP tools. Full legacy/edition parity remains incomplete. [Artwork workflow and precise limits](ASTRAL-ARTWORK.md).

## Architecture

- `astral-core`: typed documents, stable UUIDs, revisioned atomic batches, undo/redo, validation, pose/keyframe evaluation, action clips, contacts, camera animation and drawings.
- `astral-assist`: synthetic masked-pose MLP, reproducible CPU training, bounded PNG/JPEG stickman mapping, geometric stroke suggestions and cel trajectory learning.
- `astral-ml`: bounded multi-output RBF kernel-ridge training/inference from authored pose/stroke examples; all coefficients and sources persist with the project.
- `astral-storage`: bounded checksummed ASTL containers, zlib payload, atomic replace, workspace confinement and recovery files.
- `astral-render`: deterministic display lists consumed by the GPU-backed egui canvas and tiny-skia offscreen rasterization. Offscreen wgpu exports share the same display list, with bounded texture/path caches, 4× MSAA, alpha-preserving readback and CPU fallback.
- `astral-audio`: mono/stereo WAV decode, resampling, gain, source trims, fades, looping, stereo mixing and peak extraction.
- `astral-codecs`: read-only adapters to existing Python and JS codecs, with original bytes and complete decoded documents archived in project metadata; supported authored strokes/circles also become editable drawing cels. Native conversion is pending.
- `astral`: desktop UI, CLI, MCP adapter, Windows named-pipe service, background render jobs and FFmpeg encoding.

Cargo.lock pins the dependency graph. Rust UI uses eframe 0.37.0, with its wgpu renderer and AccessKit. MCP uses official rmcp 3.8.0. The small bundled stick-pose model runs locally; no external model or API key is required.

## Implemented behavior

Documents use zero-based integer frame keys and deterministic fractional-frame evaluation. Pose commands target the visible pose and remove current additive motion before storing a base key. Named poses map joints by name; selected joints can be blended or mirrored. Actions expose timing, travel, direction, intensity and named markers. Contacts solve two-bone chains; all other joint targets use direct positioning.

Transactions clone and validate the candidate document before publication, so a failed command leaves document, revision and history intact. History is bounded to 100 snapshots and a 64 MiB structural/retired-artwork budget. Immutable embedded PNG allocations are shared and counted once, not reserialized after every stroke. This is a correctness-first implementation; sparse deltas are still needed for large-project performance.

Retime maps keys, actions, contacts, effects, camera keys, drawings and audio cue starts. It preserves audio pitch/source duration, rejects key collisions, and requires callers to extend project duration before shifting content beyond its end.

The desktop provides resizable side panels/timeline with persisted visibility and dimensions, workspace tabs, an action library, direct character/joint posing, drawing strokes, pose assets, effects, WAV import/playback and a render queue. It is a new UI, not a port of every legacy panel.

Version 0.2 adds three-example pose/stroke learning, source-edit invalidation, exact authored poses, editable baked frames, stroke point editing, vector cloud/shape/lower-third layers with keyed transforms/easing/draw-on, and speech/thought/caption clips with wrapped shaped text and typewriter reveal. Text uses embedded OFL Noto Sans, HarfRust shaping and shared vector outlines; the text geometry cache has a 32 MiB budget. GUI, CLI and the 21 MCP tools use the same commands and display list. See [workflow details](ASTRAL-MOTION-TOOLS.md).

MCP supports discovery, structured schemas, project inspection, typed edits, history, asset import, preview images, saves, export jobs and cancellation. Live requests and GUI edits share a session. Stale revisions fail rather than overwriting concurrent edits.

Social/media export now includes Instagram, YouTube and TikTok composition presets with fit/fill, frame-rate conversion, quality choices, GIF, VP9/Opus WebM, still images, PNG-sequence ZIP and WAV. The native UI has embedded fonts, vector toolbar icons, rounded controls, separate workspace navigation, an interactive timeline ruler and an export composition preview. See [export settings](ASTRAL-EXPORTS.md).

MP4 export snapshots the document, renders exact ticks into bounded per-frame buffers, applies backpressure to FFmpeg, mixes separate audio clips and commits output only after success. NVENC/QSV/AMF are trial-encoded at runtime. Encoder failures retain the previous destination and editable project. No zero-copy or speedup claim is made.

## Outstanding migration work

1. Native conversion and differential parity for legacy geometry, layer sheets, nested movieclips, sprites, text, drawing tiles, constraints and VFX. Archived originals can be recovered unchanged; supported authored strokes/circles convert to drawing cels, while semantic source rigs and complex features remain unsupported.
2. Real compositing filters for glow/blur, legacy stacking/alpha/color semantics, advanced source image/compositing semantics. Native raster/image sequences and offscreen wgpu export are implemented. Current glow/blur presets are radial overlays.
3. Full property graph editor, motion-path editing, rig-length-aware retargeting, advanced drawing brushes and authored rig control editing UI.
4. Audio timeline trim/fade/gain handles, cached waveform presentation, additional media decoders and sound-library search UI. The underlying WAV operations are implemented.
5. Arbitrary docking/floating panels, multiple named layout presets, shortcut remapping, localization and a complete keyboard/accessibility audit.
6. Asynchronous native open/save and visible recovery discovery. Per-document save paths, save-all close guards, and background recovery of open documents are implemented; recovery UI and fault-injection coverage remain pending.
7. Large-project sparse history, cache optimization, GPU device-loss handling and measured 60 Hz/p95 input/export targets.
8. Native codec packaging, signed installer, update hosting, complete dependency license bundle and Windows release validation.

The existing Electron application, its file associations and upstream update configuration are deliberately retained until these migration gates pass.

## Project and cel editor (0.4)

The desktop starts at a searchable project library with native New/Open/Save As, thumbnails, favorites, project folders, document tabs, settings and dirty-close guards. Each selected project configures a confined asset/export root. Save As copies registered media and rejects asset conflicts. The cel engine adds schema-3 layers, held drawings, independent edits, blank keys, exposure changes and collision-safe key movement. These use shared transactional history and the viewport/export display list. A default frame strip and drawing toolbar replace the crowded workspace row; the detailed timeline and advanced tools remain accessible. Native clipboard support is enabled. See [editor guide](ASTRAL-EDITOR.md).

## Version 0.5 assistance

See [the assistance guide](ASTRAL-ASSISTANCE.md) for model provenance, UI workflow, offline Windows voice and the split MCP profiles. Completion suggestions require acceptance; frame generation and voice use revision-checked background transactions. The model is for canonical vector rigs, not arbitrary image generation.

## Interface refresh (0.6)

Shared palette/spacing tokens, original vector icon controls with AccessKit metadata, compact frame/layer controls, viewport-aware sheets, pinned Create/Export actions and automatic UI scale fitting are implemented in the native desktop modules. The existing animation and document services are retained. See [ASTRAL-DESIGN.md](ASTRAL-DESIGN.md).


## Motion studio (0.7)

The core `studio` module produces bounded, validated command plans for Smart Pose, fight choreography, motion-brush paths, motion review, foot locking, mirroring, batch drawings/poses, timing/FPS, reusable assets, rig templates, impact packs and camera direction. The desktop and typed `motion_tools` MCP adapter share this planner. The review panel plays original/proposed documents before committing one revision-guarded transaction. Schema 5 stores opt-in rest-length constraints, new impact kinds and named markers; the project library uses validated versioned metadata. Library exchange files are workspace-confined and atomically written.

Coaching and choreography are deterministic heuristics and templates; the learned pose prior and example-based interpolation remain separate features. Combos store poses, attached effects and markers while audio stays in separate project tracks. Full legacy native conversion, GPU offscreen rendering and measured performance targets remain pending.

Development/test profiles omit debug symbols and incremental object caches to reduce local disk use. `scripts/clean-astral-build.ps1` invokes Cargo cleanup while retaining source, projects and packaged applications.
