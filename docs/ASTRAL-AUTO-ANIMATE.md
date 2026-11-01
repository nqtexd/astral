# Manual frames and the offline director — Astral 0.10

## Manual drawing

The drawing panel defaults to **Single-frame drawing**. A pencil stroke, accepted completion, eraser edit, move, paste or clear affects only the playhead frame. The engine writes an explicit blank/restoration key immediately afterward. Multiple strokes on the same frame accumulate normally; the next frame starts blank. Delete clears the frame without resurrecting a previous drawing. Each edit, including its exposure boundary, is one undo step.

Existing saved holds are preserved. To shorten one, select it and use **Frame exposure → Make one frame**. Disable Single-frame drawing to author holds until the next drawing key, or set an explicit exposure count. Onion skin is a preview of other frames; it does not copy their ink. Completion suggestions remain ghosts until accepted.

MCP `draw_frame` takes `project`, `expected_revision`, `layer`, zero-based `frame` and `strokes`. It affects exactly one frame and preserves adjacent authored keys and old holds. The equivalent command is `set_frame_drawing`. The lower-level `set_cel` command deliberately retains its old hold-until-next-key behavior for compatibility.

## Auto Animate

The Motion studio offers **Auto Create**, **Auto Improve** and **Auto Continue**. Each prepares an editable, playable comparison. Accept commits the proposed changes as one undoable transaction. Manual drawing never invokes this controller.

MCP `auto_animate` exposes the same core engine. It defaults to a non-mutating proposal. The `spec` includes `mode`, inclusive `start`/`end` frame numbers, optional `characters`, optional `seed` and `max_repair_attempts` (0–3). Set `apply:true` to commit directly at `expected_revision`; a failed geometric gate prevents direct commit. A stale revision rejects the operation. Responses contain a normal `plan.commands`, proposed scene review, diagnostics before/after, repair-round count, seed and gate status. There is no invented artistic score.

- **create**: a procedural two-character spar for a project without rigs/story shots. It generates four editable storyboard beats, new native rigs, per-frame poses, anticipation/contact/recovery, camera keys, impact effects and markers. Supports 48–960 frames on a canvas at least 480 × 320. Eight move patterns vary by seed; explicit seeds reproduce the choreography. Recent seeds are recorded in project metadata, where available. This is a small offline scene generator, not an embedded language model or unlimited creative storytelling engine.
- **inspect**: read-only anatomy, foot-slide, root-jump and timing diagnostics plus exact contact, spacing and declared prop-interaction checks. The `plan.commands` list is empty.
- **improve**: runs at most three repair rounds for rest-length drift. Ambiguous skids, teleports and timing remain reviewable suggestions. It does not silently rewrite the story, replace the art or lock every foot to the floor.
- **continue**: extrapolates horizontal travel into a decelerating walk/run for selected grounded rigs, at most three seconds. The destination must contain no existing pose keys after the start, action clips or foot locks. Earlier/later content is preserved. It does not infer airborne physics, generate new dialogue or invent plot events. Stationary input can produce a hold.

Create, Continue and review ranges are capped at 1200 frames overall. Long productions should be planned as short shots. Sound uses the existing voice/audio tools; this director does not invent or generate a soundtrack.

## Agent workflow

1. Create a project with `create_project`, or inspect an existing project's revision and rigs.
2. Use the existing `storyboard` tool for a custom story. An external coding agent supplies creative decisions and can author its own beat sequence through `motion_tools`; no additional independent AI models are required.
3. For a starter scene, call:

```json
{
  "project": "PROJECT_UUID",
  "expected_revision": 0,
  "spec": {
    "mode": "create",
    "start": 0,
    "end": 143,
    "seed": 3,
    "max_repair_attempts": 2
  },
  "apply": false
}
```

At 24 fps this proposes six seconds. Inspect `plan.commands`, diagnostics and `review`. Commit those exact commands with `apply_commands` at the returned base revision, or call `auto_animate` with `apply:true` for a new plan.

4. Inspect `render_preview` and a short moving export. Add explicit prop interactions with `scene_interactions`, and align separate sound clips to contact markers.
5. Revise the affected beats, inspect again, then use `save_project` and `export_media`. Export returns a normal progress/cancellation job ID. A coding agent can run this loop autonomously with its own attempt/time budget; the server does not spontaneously start generating scenes.

Geometric gates check declared contacts, torso spacing, prop interactions and unresolved limb-length errors. They cannot judge entertaining choreography, acting or production quality. Intentional squash/stretch should remain a reviewed stylistic choice.

Verification: Rust manual-frame, director, desktop and transaction tests; `node scripts/check-astral-director.mjs` performs real stdio discovery, independent drawing frames, stale-revision rejection, proposal/commit, inspection, one-step undo, save and video export. Artifacts are written to `artifacts/astral-010`.
