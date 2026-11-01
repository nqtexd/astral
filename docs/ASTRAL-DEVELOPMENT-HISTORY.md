# Astral

Native Rust 2D animation studio, built alongside the existing GPL StykAnimate application. Astral has its own native executable and data directory; the legacy application is retained for compatibility while the port is developed.

Windows development updates add Flow pose interpolation, composition review/placement proposals, editable dialogue borders, locked-preview versioning, bounded parallel export and rendered-shot assembly. [Workflow and limitations](docs/ASTRAL-FEEDBACK-FIXES.md).

Astral 0.11.0-dev adds an Android touch preview and Apple build scaffolding. This is not a final release or Moho Debut feature parity. Apple builds have not been executed. See the [platform readiness and build guide](docs/ASTRAL-RELEASE-READINESS.md) for supported preview workflows and outstanding requirements.

Astral 0.10 defaults manual drawing to independent single frames, with explicit exposure controls for holds. MCP adds `draw_frame` and `auto_animate`: seeded editable stickman scenes, inspection, bounded length repairs and short grounded continuation. Motion studio provides playable Auto Create/Improve/Continue proposals. [Manual frames and director guide](docs/ASTRAL-AUTO-ANIMATE.md).

Astral 0.9.1 adds revision-checked scene interaction cues to MCP: verify hand-to-prop contact, graphic states and actual doorway crossings. Running arms preserve their original elbow branch and blend in without a startup snap. After the Siren now visibly opens the service door and removes both relay interlocks, with forearm-aligned hands and blended motion transitions. [Interaction workflow](docs/ASTRAL-INTERACTIONS.md).

Astral 0.9 adds custom editable vector contours, camera-aware artwork culling, and natural-pace walk/run defaults for MCP and the desktop. The original After the Siren production includes a causal storyboard, new furniture/character artwork, sustained chases and contact-directed aerial combat. [Art and motion guide](docs/ASTRAL-ART-DIRECTION.md).

Astral 0.8 adds a persistent Storyboard workspace and MCP storyboard/review tools, distance-driven walking with planted stance feet, contact-solved fight beats, and live graphic-to-rig bindings. [Storyboard and contact workflow](docs/ASTRAL-STORYBOARD.md).

Astral 0.7 adds a Motion studio with constrained posing, editable fight choreography, a motion brush, coaching, frame-range editing, reusable animation libraries, impact packs, sound waveforms and camera direction. Suggestions have a playable comparison and one-step undo. [Motion studio guide](docs/ASTRAL-STUDIO-TOOLS.md).

Astral 0.6.1 fixes populated project-card stacking and narrow editor alignment.

Astral 0.6 refreshes the native interface with a graphite/blue palette, original vector toolbar icons, compact frame/layer controls, responsive sheets and optional automatic UI scale fitting. [Interface guide](docs/ASTRAL-DESIGN.md).

## Run

Rust 1.95+, the Windows MSVC build tools, and a working graphics driver are required.

```powershell
cargo run -p astral -- --workspace D:/AstralProjects studio
```

The studio opens a project library. Create a blank project or open **Motion lab**, an editable example with learned pose/stroke inbetweens, dialogue, drifting clouds, keyed shapes and a lower third. Use Space to play. **Rig** poses characters, **Draw** authors and edits strokes, **Dialogue** adds speech/thought bubbles and captions, and **Graphics** creates animated vector artwork. Save writes a `.astral` project inside the configured workspace.

The offline machine-learning assistant learns from **3–64 authored poses or corresponding strokes**. It fills the intermediate frames and can bake them into ordinary editable keys/drawings. [Workflow guide](docs/ASTRAL-MOTION-TOOLS.md).

For a console-free desktop launch, run `target/release/astral-studio.exe` after a release build. The portable package names this launcher `Astral-Studio.exe`; `astral.exe` provides the CLI and MCP server.

```powershell
cargo test --workspace
cargo run -p astral -- --workspace D:/AstralProjects demo
cargo run -p astral -- --workspace D:/AstralProjects render motion-lab.astral preview.png --frame 75
cargo run -p astral -- --workspace D:/AstralProjects validate motion-lab.astral
```

For a smaller headless build, add `--no-default-features` before Cargo's `--` separator.

## Drawing assistance and offline voice

Version 0.5 adds two MCP profiles for projects and animation, reversible stroke suggestions, example-based cel completion/inbetweens, a trained small stick-pose model, PNG/JPEG stickman-to-rig mapping with landmark review, native Settings, richer dialogue typography and offline Windows text-to-speech. [Usage and limitations](docs/ASTRAL-ASSISTANCE.md), [trained model card](models/MODEL_CARD.md).

## Agent control

Run `astral --workspace D:/AstralProjects mcp` to attach an MCP client to the running Windows studio. Run `astral --workspace D:/AstralProjects mcp --headless` to use a separate headless session. Both use the same document commands and render display list.

See [agent API](docs/ASTRAL-AGENTS.md), [architecture and implementation status](docs/ASTRAL-IMPLEMENTATION.md), and [validation](docs/ASTRAL-VALIDATION.md).

## Project manager and frame editor

Astral 0.4 starts at a project library with New/Open, recent thumbnails, search and favorites. Create a blank project with your canvas size, fps, duration and folder. Save, Save As, document tabs, unsaved-change prompts, project settings and per-project autosaves manage the editable work. Save As carries registered media into the destination folder.

The default drawing view has layers, a thumbnail frame strip, blank/duplicate/copy/paste/clear drawing controls, holds, onion skins, pencil, stroke eraser, drawing movement and pan/zoom. Editing a held frame makes an independent cel. The detailed timeline and existing rig/action/VFX/audio tools remain available. The interface uses neutral surfaces, a simple A monogram and plain status labels. See the [editor guide](docs/ASTRAL-EDITOR.md) for workflows, shortcuts, agent commands and schema-3 compatibility.

## Export

Astral 0.3 adds Instagram Reels, Stories, portrait and square exports, YouTube 1080p/4K and Shorts, and TikTok. The **Export** window previews fit/crop framing and offers MP4, WebM, GIF, PNG, JPEG, PNG-sequence ZIP and WAV, with custom dimensions, frame rates and source-frame ranges. The native UI now has embedded Noto Sans typography, rounded controls, vector toolbar icons, workspace buttons and a timeline ruler. See [export guide](docs/ASTRAL-EXPORTS.md) for settings, CLI/MCP examples and platform sources.

Video, GIF, image sequence and resized image export require FFmpeg. Set `ASTRAL_FFMPEG` to its executable, place it in `resources/ffmpeg.exe` beside Astral, or use the existing `scripts/stage-video-encoder.py` workflow. Astral also discovers `build/video-encoder/ffmpeg.exe` during development. The package script copies a staged encoder and its notices.

```powershell
cargo run -p astral -- --workspace D:/AstralProjects export motion-lab.astral renders/reel.mp4 --preset instagram-reels --framing fit --hardware
```

Hardware mode tests NVENC, QSV and AMF before falling back to software encoding. Offscreen rendering currently uses a CPU rasterizer; the viewport is GPU-backed through eframe/wgpu. A 2× export improvement has **not** been established.

## Migration status

This is an initial native implementation, not a completed replacement for every legacy feature. Legacy files can be decoded and archived with their original bytes, but their geometry/timelines are not yet converted into native editable scenes. Image assets can be registered but are not rendered yet. Full legacy VFX, movieclips, text, sprite and drawing parity, advanced docking, shortcut remapping, and release installers remain work in progress. Explicit capability results expose these limits to agents.

Keep using `npm start` for the existing editor. Its behavior and file compatibility remain available during migration.

## Attribution

Astral derives from StykAnimate by Filippo Calippo and contributors. Existing source attribution is retained. Project metadata declares GPL-3.0-or-later; see [COPYING](COPYING). Rust dependencies and packaged tools retain their respective licenses. No upstream release account or update feed is used for Astral.
