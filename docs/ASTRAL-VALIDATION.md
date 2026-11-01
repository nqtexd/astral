# Astral validation record

Local Windows development checks, 28 October 2026. These are development results, not a release certification.

## 0.13 artwork and renderer revision — 1 November 2026

- [Windows CI](https://github.com/nqtexd/astral/actions/runs/41326513531) passed on `bc3e334`, including the new native artwork MCP check. Local `cargo test --workspace --locked`: 157 tests passed; strict workspace/all-target Clippy and formatting passed. Coverage includes image-sequence painting isolation, undo/storage sharing, PSD composite/error rollback, editable mouth cues, shaped text fitting, tracking proposals, and original legacy pose timing.
- Physical RTX 4050: offscreen GPU alpha/interior-colour checks passed. No GPU timing or artistic-quality inference is made from a test pass.
- Real stdio MCP: 50 tools overall, 46 animation-profile tools and 15 project-profile tools. The existing external workflow, feedback/preview/assembly checks and profile handoff passed. New artwork calls verified imported image sequences, small inspection payloads, independent painting, stale-revision rejection, save/reopen and resized PNG output with FFmpeg deliberately unavailable.
- Live Windows editor: opened the raster study, selected Artwork, inspected controls and source-paint layout. The sidebar was refined after this check. Microphone WAV formatting is tested with synthetic stereo data; physical microphone/disconnect testing remains unfinished.
- Three original editable studies and their real Astral GIF previews are included in the README. The separate showcase ZIP includes full-timing MP4 previews with separate editable audio assets. These are demonstrations, not acting/production certification.
- Remote Apple build evidence is recorded in [platform checks](ASTRAL-PLATFORM-CHECKS.md). Physical Android/Apple devices, production signing and full edition parity remain unverified.

## 0.8 storyboard and contact revision — 29 October 2026

- `cargo test --workspace --release`: 118 tests passed. Added coverage for storyboard persistence/undo/atomic rejection, continuity notes, missed contacts, torso overlaps, live rig bindings and planted gait. After resizing the preview, its compact-layout test passed again.
- Real stdio MCP: 41 tools discovered; `storyboard` and `review_animation` exercised with stale revisions and undo. The five-minute LAST BELL project was reviewed shot by shot at every frame: 39 contact cues pass, no torso-envelope overlaps detected. See the production's `MOTION-REVIEW.json` for exact results.
- Live Windows UI: checked the populated project library, storyboard menu, shot list, rendered shot preview and character planning fields. The preview was then reduced and its actions moved above it to keep controls accessible in shorter windows.
- These checks do not certify narrative or artistic quality. The film remains stylized rig/cutout animation. Contact checks are joint distances and torso envelopes, not physical collision simulation.

## Legacy baseline

- Before dependency setup: JavaScript 3,780 tests, 3,747 passed, 9 failed and 24 skipped. Failures involved missing installed npm dependencies/browser codec runtime.
- After `npm ci --no-audit --no-fund`: JavaScript 3,798 tests, 3,772 passed, 0 failed and 26 skipped.
- Python: 161 tests, 0 failures, 47 skipped. Existing resource warnings were emitted by subprocess tests.
- Legacy browser/packaged smoke suites and full codec fixture corpus were not executed. Skipped tests are not treated as coverage.

## Native 0.1 baseline checks

- Rust workspace: 28 tests passing, covering atomic rollback, stale/cross-project revisions, undo/redo, clip overlap, targeted replacement, posing during actions, invalid rigs/poses, locks, named poses, retime collision handling, deterministic evaluation, two-bone IK, duplicate IDs, path confinement, container corruption, size limits, atomic replacement/recovery, preview output, audio mixing, the shortcut-lock regression and protection of existing assets when starting demos.
- `cargo check -p astral` passed with the default desktop feature.
- Real stdio MCP acceptance passed: 15 discoverable tools; external client authored a ten-second scene with 2 characters, 12 action clips, 3 effects, camera keys and 3 audio cues. A follow-up replaced one action while preserving unrelated tracks. Stale edits and partially invalid transactions were rejected.
- The resulting `.astral` file reopened and validated through the CLI. MCP and CLI PNG previews at frame 104 had identical SHA-256: `cac5c1c5f2eb94fa0bb398366edf6c17283cfe41c692d8d1a2c5098de8f21127`.
- Native desktop launched and exposed its controls through AccessKit. Live named-pipe project inspection succeeded. UI inspection found and corrected a nested context-lock startup deadlock in the shortcut handler.
- A 300-frame 1920×1080 MP4 with audio exported successfully using `h264_amf`: debug-build render time 49.80 seconds, encoder transfer/backpressure 0.51 seconds, total 53.20 seconds. These values include CPU rasterization and are **not** comparable to the unmeasured legacy baseline.
- The optimized build exported the same 300-frame scene with audio using AMF in 5.89 seconds: 2.98 seconds of CPU rendering and 0.89 seconds of encoder transfer/backpressure. The output decoded as 300 frames / 10 seconds, H.264 video plus stereo audio. This is a simple-scene measurement, not proof of the planned legacy speedup.
- A scaled 3840×2160 version of the scene exported 300 frames with audio through AMF in 14.87 seconds, including 10.79 seconds of CPU rendering. This remains a simple-scene measurement.
- Export cancellation preserved a pre-existing complete destination file. Clippy passed for the workspace and all targets with warnings denied.
- The transitional legacy decoder successfully archived a generated STYK fixture with its original source bytes unchanged and explicit native-conversion warnings.

Evidence logs are ignored local `artifacts-*.log`; generated MCP project, preview, report and video are under `artifacts/astral-smoke/`. Run `node scripts/smoke-astral.mjs` after building to reproduce the external-agent authoring checks.

## Native 0.3 checks

- Rust workspace: 50 tests pass. This includes learned pose/stroke models, invalidation/undo, exact save/reopen and baked-frame previews, vector glyphs and ring geometry, typewriter reveal, export preset sizing, frame-rate mapping, GIF defaults and invalid range/dimension rejection. A framed-render regression checks that off-canvas artwork cannot leak into letterboxing and that subpixel source details become sharp at delivery resolution. Strict Clippy passes with warnings denied.
- Real stdio MCP acceptance passes with 21 discoverable tools, including export preset discovery and a portrait PNG through `export_media`. The ten-second two-character scene and targeted revisions still pass. Learned pose and stroke examples bake into 31 editable frames each, with identical exported pixels before and after baking and after reopening. Cancellation preserves a pre-existing destination.
- Real CLI/FFmpeg integration passes 16 exports: all eight social destinations, fill framing, WebM, GIF, JPEG, PNG, PNG-sequence ZIP, WAV and 60 fps MP4. Decoders verified dimensions including 3840 × 2160; pixel assertions distinguish fit letterboxing from fill cropping; the 60 fps output decodes to eight frames for a four-frame 30 fps source range. GIF decodes to two frames at its default 15 fps. ZIP CRC, four PNGs, manifest timing and separate audio pass. Trimmed WAV has 6,400 stereo frames at 48 kHz and contains an audible cue.
- Commands: `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `node scripts/smoke-astral.mjs`, `python scripts/smoke-astral-exports.py`. Export reports are under `artifacts/astral-exports/report.json`; MCP evidence is under `artifacts/astral-smoke/mcp-report.json`.
- The refreshed UI compiles. Interactive visual/accessibility validation of the 0.3 layout is still pending; automated export checks do not establish desktop interaction quality.

## Native 0.4 checks

- 69 Rust tests pass. New checks cover held-cel independence, blank keys, layer locking and deletion/undo, exposure timing, key collisions, stable key identity, older schema defaults, layer ordering/opacity/visibility in rendered pixels, and identical cel frames after save/reopen.
- Save As tests verify copied media and identical mixed audio after reopening, refusal to overwrite unrelated conflicting assets, and agent paths confined to the configured document root. UI-state tests cover dirty-close guards, save/close/reopen and preserving unsaved state when opening an already-open tab.
- Headless egui pointer-gesture tests reproduce fast pen and move gestures and fast eraser sweeps. They verify the start point is retained, edits on held frames are independent, and undo restores the drawing in one step. These are component behavior tests, not a substitute for an exhaustive desktop audit.
- Strict workspace Clippy passes with warnings denied. The real MCP smoke test still discovers 21 tools and preserves the earlier fight, motion-learning and targeted-edit workflows. It additionally creates a layered schema-3 cel project, edits one held frame without changing earlier keys, moves a key with stable identity, changes exposure, saves/reopens pixel-identical PNGs and encodes 12 cel frames through the media exporter.
- All 16 actual media export checks pass after the editor changes. Existing rig/drawing-model PNG hashes remain unchanged.
- Live desktop checks traversed the project library, create dialog and drawing editor. They caught disabled native clipboard support, an inaccessible lower layer-panel section and the missing pen-start issue. Clipboard support is enabled, the panel scrolls, and the gesture fixes have regression coverage. The last layout refinements and clipboard integration have not received a complete manual/accessibility audit.
- The GUI test used an isolated workspace/settings folder. Its open document was backed up under artifacts/astral-v04/qa-backup. Release checks and reports remain local development evidence; clean-machine validation is pending.

## Native 0.5 assistance checks

- Rust workspace: **81 tests pass**, including the trained model on fresh synthetic held-out seeds, observed-joint preservation, bounded/cancellable training, clear and transparent stickman mapping, rejected invalid/blank/dense images, corresponding-stroke cel learning, partial-frame preservation, rich-text validation/rendering/round-trip, managed project lifecycle and real offline Windows speech. Strict all-target workspace Clippy passes with warnings denied.
- Pen/move/eraser regression tests caught suggestion widgets changing the canvas interaction ID; the canvas now has a stable ID. A separate regression confirms identical screen gestures produce identical world points before and after suggestion controls appear. Completion is explicit, undoable and rejects a stale revision.
- Real stdio MCP discovers **38 tools**. The full acceptance script creates a managed project, maps eleven landmarks from a stickman PNG into an editable rig, adds an action, previews pose completion without mutating, generates 17 editable cel keys while retaining source/partial keys, trains and reloads a project model, cancels a training job without editing the document, rejects a training result after a concurrent edit, synthesizes offline voice and saves styled dialogue in schema 4. Existing fight, targeted replacement, cancellation, archive and cel export checks remain in the same script.
- Profile discovery: ten project tools and 34 animation tools. Separate headless profiles successfully hand off a saved project. Against an isolated running studio, both profiles find and edit the same live document/revision through the named pipe.
- All **16 actual media export checks** pass. Live UI checks opened Settings, drawing assistance, generated MCP configuration, a new blank project, geometric ghost completion and the voice box. Accepting the ghost and generating a WAV plus matching editable dialogue succeeded through the UI. The QA documents were saved under `artifacts/astral-v05-ui`.
- The bundled neural model was actually trained: 8,192 synthetic samples, 100 epochs, 1,024 held-out poses, 3,606 parameters. Missing-coordinate MSE 0.00969554 versus 0.06569623 for unchanged rest pose (85.24% lower). This is a synthetic coordinate benchmark, not evidence of real-artwork reconstruction, temporal quality, superiority to interpolation, or an export speedup. Weights, report, model card and reproducible trainer are included in the repository.
- Image mapping remains a reviewed heuristic for one clear stick figure. Text coverage follows bundled fonts; offline speech uses installed Windows voices. Clean-machine, broad artwork, accessibility and performance validation are still outstanding.

## Native 0.6 interface checks

- **85 Rust tests pass**, including new AccessKit/egui coverage for icon names, 32-point control bounds and onscreen controls at 800 × 520, 960 × 620 and 1280 × 800. The new-project Create action stays within a short viewport. Scale fitting preserves a usable logical viewport and handles invalid stored scale values. Palette tests require a contrast ratio of at least 4.5 for normal/secondary text, selected text and primary-action text. Strict workspace/all-target Clippy passes with warnings denied.
- Live native review traversed the project library, new-project form, a saved drawing project, Settings and Export. The initial review caught a default 400-point modal constraint that hid creation controls. The form now scrolls independently of its footer, and Create opened the saved project successfully through the UI. Export keeps its action outside its scrolling options.
- A requested 200% UI scale applied 154% on the available 1280 × 800 client area, keeping the sheet and controls in view; the applied scale is shown in Settings. The setting can be disabled. The QA workspace/settings are isolated under `artifacts/astral-v06-ui`.
- Pen start, held-cel independence, move, eraser sweep, suggestion acceptance/undo and stable canvas coordinates continue passing. Document schemas, rendering semantics, training models and MCP commands were not redesigned by the UI update.
- The full interaction/accessibility audit and clean-machine verification remain outstanding. The earlier 0.5 package had one non-reproduced export timeout; the full 16-export rerun and three additional repetitions passed. This UI release does not claim to resolve that intermittent encoder issue.

## Native 0.6.1 alignment regression

- Reproduced the populated-library problem: a card frame inherited its horizontal row's layout, so the thumbnail, labels and actions shared a row. Cards now create an explicitly vertical child UI with a bounded content width, consistent row gaps and truncated titles/paths. Column widths are calculated inside the scroll viewport. The status footer is anchored outside the scrolling grid.
- **87 Rust tests pass** and all-target workspace Clippy passes with warnings denied. New populated-library cases use 1, 4 and 7 projects, long titles and folder names at different viewport sizes. They assert that titles follow thumbnails, actions follow metadata, cards do not overlap, widths remain bounded, and the status footer stays visible.
- Editor coverage now includes pairwise toolbar bounds, long project/layer names, minimum-width drawing panels with onion skin enabled, and every non-drawing inspector at a requested 210-point width. Controls/captions remain inside the window without widening the panel. Narrow slider captions stack above controls; inspector action rows and text-alignment controls wrap, and onion counts use aligned rows.
- Live native review used four real saved projects with thumbnails and a long title. The cards rendered in separate vertical columns with metadata and actions below each preview. Opening the long-title project showed the drawing editor without toolbar/layer overlap. Maximizing the test window exposed a separate offscreen initial-window position; fresh profiles now start maximized within the OS work area. Existing saved window geometry remains supported.
- Test files/settings are isolated under `artifacts/astral-v061-ui`. No animation document schema or MCP contract changed.

## Gates still outstanding

- Native legacy conversion and differential visual/document parity.
- GPU offscreen export, device-loss behavior and preview/export compositing parity for all legacy effects.
- Matched-quality comparison with legacy exports on AMD integrated and RTX 4050 graphics; 60 Hz interaction, p95 input response and 2× export target remain unverified.
- Full native interaction/accessibility audit, missing-media workflows, fault injection and packaging verification on clean Windows machines.
- macOS/Linux builds, signing, installer and update delivery.


## Native 0.7 motion studio

- **112 Rust tests pass** in the final optimized workspace run. Strict release-profile workspace/all-target Clippy passes with warnings denied. Coverage includes reachable IK and rest-length constraints, style variants, choreography and contact markers, motion-path resampling, slide/length coaching, foot contacts, pose and drawing batch edits, conflict rejection, snapshot clipboard behavior, reusable combos/symbols/motions, retargeting, FPS/timing, camera keys, marker persistence, library file confinement and before-commit preview validation. Compact-window tests cover all eight motion tabs and the visible comparison transport/Accept action.
- A real MCP startup exposed a tagged-enum schema without a root object type. The production request now explicitly declares that type; a regression constructs the complete MCP router. Live review also exposed a fast motion-brush gesture losing its endpoints. Capture now retains primary-button press/release events, with a single-frame gesture regression that also checks non-mutation.
- The packaged executable exposes **39 MCP tools**. Full stdio acceptance creates a ten-second, five-beat, two-character fight; adds camera keys, 17 effect tracks and a separate sound cue; obtains two comparison images and a 128-column waveform; exports/imports a reusable combo; edits only one character's motion path while retaining other tracks; inspects coaching/trajectories; and verifies undo/redo. Stale revisions and invalid previews with apply enabled are rejected without mutation. Existing image mapping, cel generation, local training/cancellation, offline voice, rich dialogue, archive, targeted edits, save/reopen and rendering checks also pass.
- Both MCP profiles pass: **10 project tools / 35 animation tools**. Tests verify headless file handoff and connection to the running packaged Windows desktop through its document service. The animation profile also returns a non-mutating Smart Pose preview with two images through the live bridge.
- All **16 actual media export checks** pass using the packaged CLI and FFmpeg: Instagram/YouTube/TikTok delivery dimensions, fit/fill framing, audible MP4/WebM tracks, GIF frames, PNG/JPEG, PNG-sequence ZIP and WAV range mixing. Package verification checks all 469 file hashes, ZIP CRC/content and the archive checksum, including models and MCP configuration.
- Live native review opens/saves a project, traverses motion/curve controls, plays and cancels the original/proposed comparison, captures a quick hand path, previews 25 editable poses and accepts one transaction. Document-service inspection confirms a single revision increment and unchanged audio/effects; one toolbar undo restores the entire original document apart from its revision counter. Audio-mode contextual controls and individual waveform tracks were also inspected. QA data and reports are isolated under `artifacts/astral-v07-ui`.
- Coaching and choreography remain heuristic/templates. These checks do not establish clean-machine compatibility, full legacy conversion, comprehensive accessibility, or the original 60 Hz/p95/export-speed performance targets. The existing remaining gates still apply.
