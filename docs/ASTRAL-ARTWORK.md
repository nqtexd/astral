# Artwork, voice and rendering

Astral 0.13 adds native raster tracks alongside the existing vector frames and rigs. Manual drawing stays independent by default.

## A short desktop workflow

1. Create/open a project. Switch the workspace selector to **Artwork**.
2. **Import artwork** opens a PNG/JPEG/WebP/BMP image, an ordered set of images, or an 8-bit RGB PSD. Multiple images become one sequence at Source FPS. Import runs as a cancellable job; stale revisions fail atomically.
3. Select a layer. Adjust visibility, looping, Source FPS or Transform. Transform changes key the current frame. Raster PNGs are embedded in `.astral`; external images are unnecessary after import. Original PSDs stay registered as source assets for Save As.
4. Enable **Paint this frame** to paint/erase in original image coordinates. A stroke creates an immutable PNG cel for this frame. The next frame restores its original cel or resumes sequence sampling. Undo restores both pixels and exposure boundaries.
5. Under **Deformation**, create the grid/pins. Corner anchors and a centre pin start at zero offsets. Animate offsets at later frames. This is weighted pin deformation, not an advanced rig/ARAP solver.
6. Use **Extract video frames** for a short sprite sequence. The desktop limits extraction to 120 frames and a 640-pixel longest side. MCP permits bounded explicit dimensions/up to 240 frames within the total pixel budget. Sound is separate.
7. For tracking, choose a distinctive source-image pixel under **Track camera**. Preview a range of at most 120 frames. Review the proposed camera keys, then accept them as one undoable edit. A local template can drift with occlusion, blur or repeated textures; a uniform/lost patch is rejected.

The timeline playhead, playback and save/reopen work across raster, rig, drawing, effects and audio tracks. Painting intentionally displays the unwarped source; switch it off to see the final deformation and composition.

## Recording and mouth cues

In Audio or Artwork, expand **Voice & mouth timing**. **Record microphone** opens the default mono/stereo capture device only after you press it. **Stop & add audio** closes capture and prepares a WAV on a worker. Capture is bounded to 60 seconds; no microphone data is sent to a service. The clip is added at the project/frame where recording began, even if you switch tabs.

Select a character, choose an existing WAV clip and optionally enter a transcript. **Generate mouth cues** creates a separate mouth track. Rest/A/E/O/F/M keys remain editable with the buttons at the current frame, or `PutMouthTrack` through MCP. This uses sound energy and approximate transcript timing, not speech or phoneme recognition. Review it after retiming audio. Existing offline Windows voices remain available in the dialogue tools.

## Motion and text review

**Preview motion polish** proposes continuous motion, conservative planted-foot ranges and bounded upper-body follow-through. Authored/contact anchor frames are preserved; secondary body changes are disabled for declared prop interactions. Preview before accepting. It cannot invent natural acting from arbitrary poses.

**Fit text & suggest placement** uses the same shaping/wrapping as export, expands a bounded box and reduces font size only as needed, then searches for placement. It preserves editable wording. Review results on moving frames. Text overflow warnings appear in previews/review, and deformed raster bounds participate in collision checks. These remain review aids, not a visual-quality certificate.

## CPU and GPU export

Select **Renderer: Auto / CPU / GPU** in Export, use `spec.renderer` through MCP, or `astral export … --renderer gpu`.

- Auto reserves a physical wgpu adapter for at least 48 output frames with substantial raster coverage (over half the source canvas at the first export frame). Short clips, small sprites and vector-only scenes stay on CPU; failed GPU initialization falls back to bounded CPU workers. This conservative heuristic avoids GPU startup overhead for light scenes.
- GPU uses the shared evaluated display list, textured mesh triangles, premultiplied alpha, 4× MSAA and readback. It rejects a software-only adapter. Device/runtime failures are reported and preserve the previous output.
- Completion metrics identify the actual renderer, adapter, CPU worker count, render time, GPU readback time, encoder transfer and total time. GPU submission/readback includes synchronization; timings are not a GPU profiler trace.
- Hardware **encoding** is a separate FFmpeg choice. A GPU renderer does not prove hardware encoding or a universal export speedup. Font-edge coverage can differ slightly between CPU and GPU rasterizers.

FFmpeg is separately installed. Public packages do not redistribute the encoder. PNG previews continue to work without it.

## Agent tools

`artwork_tools` is available in the all/animation MCP profiles. Its tagged `operation` accepts `import`, `paint`, `video`, `lip_sync`, or `track`. Mutations name the project and current `expected_revision`. Track defaults to a non-mutating proposal; inspect its `points` and `plan.commands` before committing. Imports/extraction/tracking return a job ID; poll `job_status` or cancel the job.

```json
{"operation":"import","project":"PROJECT_UUID","expected_revision":12,"paths":["assets/frame-001.png","assets/frame-002.png"],"start":0,"source_fps":24,"looping":true}
```

Use `apply_commands` with `PutRasterLayer`, `PutMouthTrack` and existing commands for transforms, pins, cues and batches. `motion_tools` accepts `edit` with `polish_motion` or `layout_dialogue`; preview first. Image bytes are omitted from ordinary project inspection to keep responses small; metadata and encoded byte counts remain visible.

## Supported limits

Raster imports are bounded to 16 megapixels/image, 128 megapixels/project and 96 MiB of encoded PNGs. Input images/PSD are at most 32 MiB. PSD supports 8-bit RGB layer/composite data; text, masks, adjustments, groups, non-normal blends, high bit depth, PSB and ZIP-compressed sections are not editable equivalents. Unsupported/malformed PSD processing fails without partially publishing a project. Animated GIF import uses its first image and warns; use image sequences for animation.

Legacy supported strokes/circles become authored drawing cels, preserving source pose timing and the original archive. They do not become semantic character rigs, and source tweening, joins, custom shapes and effects need further compatibility work. Full Moho edition parity, physical mobile validation and production Apple/mobile signing remain separate unfinished gates. The [readiness inventory](../platforms/release/readiness.json) records partial features explicitly.
