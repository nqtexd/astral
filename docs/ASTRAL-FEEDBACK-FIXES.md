# Motion, composition and finishing updates

This Windows development build addresses representative community feedback. No failing user project was supplied, so benchmarks and checks below describe local fixtures rather than a reproduction of a user's reel.

## Flow through authored poses

Motion studio → Pose → Preview flowing motion prepares a reversible proposal for the selected character and frame range. It changes key interpolation, retains authored key poses, and uses shortest joint-angle arcs with time-aware, shape-preserving cubic tangents. It avoids bone shortening and the repeated stop at every Smooth key. Existing contacts still solve after interpolation. Original projects retain their existing interpolation until you accept a proposal.

MCP `motion_tools` input:

```json
{"operation":"edit","project":"PROJECT_UUID","expected_revision":4,"tool":{"task":"flow_motion","character":"CHARACTER_UUID","start":0,"end":71},"apply":false,"preview_frame":18}
```

Inspect the original/proposed images and moving preview, then commit `plan.commands` through `apply_commands` at the same revision. Flow does not invent acting, weight shifts, anticipation or follow-through. Dense baked keys can still need manual refinement; intentional holds remain available.

## Composition review and placement

`review_animation` now includes `layout`: dialogue/graphic-text bounds against heads, bodies, feet, props, other dialogue boxes and camera edges. Each issue identifies its subject, obstacle, frame and bounds. Attached graphics and character-relative dialogue are evaluated in world space. Large background plates are excluded. These are conservative bounds checks; diagonal limbs, transparent contours and intentional overlaps can generate false positives. Sampling can miss a collision between samples; use `step:1` for a final short-shot review.

`render_preview` returns single-frame `layout_review`; CLI `validate` separates structural validity from visual review and never certifies artistic quality. Exports report sampled composition warnings. Storyboard → Review shot includes a layout-warning count. Select a native dialogue in the Dialogue workspace to see current-frame warnings and choose Suggest clear placement.

For a revision-checked placement proposal:

```json
{"operation":"edit","project":"PROJECT_UUID","expected_revision":4,"tool":{"task":"layout_dialogue","dialogue":"DIALOGUE_UUID"},"apply":false,"preview_frame":18}
```

Placement searches stable positions over every frame of the dialogue (maximum 1,201 frames and a bounded scene budget), retains editable words, and does not commit silently. If no clear placement exists, it returns a warning; resize, shorten or reframe the cue rather than trusting a nonzero-score suggestion.

## Editable dialogue presentation

Dialogue now stores `appearance`: `border_color`, `border_width` (0 hides it), `corner_radius` and `speaker_color`. Fill and text colors remain editable. The Dialogue inspector exposes these controls and a Dark caption preset. Legacy files retain their original appearance. MCP `add_dialogue` accepts appearance, fill/text colors and size; `replace_dialogue` updates them without converting words to outlines.

## Locked previews

Unlocked PNG previews are atomically replaced as before. If another Windows application locks a preview or marks it read-only, Astral preserves it and automatically writes a uniquely versioned PNG. `render_preview` returns the actual `path`, `requested_path` and a warning. Always display the returned path. Other errors (bad paths, disk failures, permissions on the directory) are still reported; no destructive unlink is used.

## Render pipeline

Frame rasterization runs on up to four workers, constrained by output size and a 192 MiB frame-buffer budget per export. Bounded queues retain exact frame order and stop on cancellation or encoder failure. Encoding still uses the existing encoder choices. Full-canvas and fill exports avoid a redundant full-resolution clip mask; letterboxed exports retain clipping.

Metrics distinguish aggregate worker render time from encoder transfer time and total wall time; `render_seconds` is summed CPU-worker time and can exceed wall time; in GPU mode it is serial render/readback elapsed time. `render_workers` reports the worker count. Select the separate GPU renderer for offscreen wgpu rasterization; hardware encoding remains independent. No universal performance or artistic-quality claim is made.

## Assemble rendered shots

Export → Assemble rendered shots joins existing local videos, including a corrected ending, using hard cuts and independent in/out trims in seconds. Set Source audio off for a video without an audio stream. Paths are relative to the active project's workspace. Set the output path, even dimensions and FPS, then Assemble MP4. The Render queue reports progress and cancellation. The source project and source clips are retained; a failed job preserves the previous completed destination.

MCP example:

```json
{"project":"PROJECT_UUID","spec":{"path":"renders/finished.mp4","width":1920,"height":1080,"fps":30,"clips":[{"path":"renders/main.mp4","start_seconds":0,"end_seconds":42,"audio":true},{"path":"renders/corrected-ending.mp4","start_seconds":0,"end_seconds":6,"audio":true}]}}
```

Call `assemble_video`, then poll `job_status`. Two to 32 local MP4/MOV/WebM/MKV shots are normalized and re-encoded, including resampled stereo sound; silent inputs receive silence. The maximum is ten minutes per clip and one hour total. This is a finishing operation, not an editable multitrack video editor, crossfade tool or lossless splice.

## Verification

Core regression tests cover continuous velocity, joint lengths, immutable placement proposals, undo, camera edges and collisions. Windows tests hold a preview file with delete sharing disabled and verify automatic versioning and preservation. Renderer tests cover custom/no-border text and pixel equality of the full-canvas clipping optimization. `scripts/check-astral-feedback.mjs` exercises the real MCP author/review/propose/commit/render/assemble loop and failed-output preservation. `scripts/benchmark-astral-feedback.mjs` records a repeatable two-second 1080p stickman/text fixture; its original 0.10 baseline took about 2.25 seconds on this machine, so it did not reproduce minutes-long simple exports.
