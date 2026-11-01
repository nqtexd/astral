# Motion studio (0.7)

Open **Motion studio** with the linked-node toolbar icon. The editor keeps its graphite surfaces, blue accents and compact vector controls. Beginner controls are the default for posing, animation, audio and effects; enable **Pro controls** in the inspector, View menu or Settings for the detailed inspector. Drawing retains the frame-strip workflow.

## Pose and movement

- **Pose:** Smart posing guides a hand or foot with two-bone IK and preserves rest lengths. Natural, Exaggerated and Anime Fight change the amount of body adjustment. Hold **Alt** while dragging to position a joint manually. Constraints are stored per character; enabling them also affects its interpolated poses.
- Save named poses, apply them to compatible rigs, or mirror a selected range. Foot locking can detect ground-like stance ranges or use a manually selected range. Review the suggestion before accepting it.
- **Motion:** select a character and named control, then draw a path on the canvas. Choose the frame range, number of editable keys, movement style and easing. Root paths move the whole character. The curve preview supports even, eased, snappy, hold and editable Bézier settings.
- Enable changed-joint onion skins or a ghost trajectory in the inspector. Ghost paths use warmer segments for faster movement; they do not change the animation.

## Choreography and coaching

**Fight** takes two characters and an ordered list of punch, dodge, counter, kick and knockback beats. Each beat has an attacker, frame count and strength. **Starter combo** supplies an editable example. The builder produces ordinary anticipation, contact, follow-through and recovery pose keys, contact markers and optional effects. Replacement of existing animation requires the explicit replacement option. Existing foot contacts must be adjusted before generating overlapping choreography.

**Coach** reviews a range for possible limb-length drift, foot sliding, position jumps and uneven key spacing. Each flag explains what was observed and can offer a correction. Flags are heuristic guidance: deliberate stretch, skids, teleports and impact timing can be retained. Nothing is silently corrected. Locked characters can be inspected without being modified.

These tools use deterministic animation templates, geometry and heuristics. They are distinct from the separately bundled small learned pose model and authored-example inbetween learning. They do not provide a physics simulator or guarantee finished choreography.

## Frame ranges and timing

Shift-click frame thumbnails to select a range. **Frames** can copy, move, stretch, translate or hold drawings on the active layer, or edit authored pose keys. Copy/paste captures a snapshot of the source range. A later edit to the source does not change the clipboard. Destination conflicts are reported; enable replacement deliberately. Drawing moves retain the moved cel identities, and unrelated layers retain their timing.

The same panel offers ripple timing in seconds, FPS conversion that preserves approximate duration, and named timeline markers. Retiming that merges authored keys is rejected. Ripple timing moves audio cue starts while retaining audio source duration and pitch. Pose-only operations leave additive action clips in place and can change neighboring interpolation; review their warnings.

## Reusable library

**Library** includes editable idle, walk, run, jump, turn, strike, recoil and fall action presets. It also creates human, quadruped and winged rig templates. Presets and templates are starting points for manual editing.

Save a character, motion range, drawing symbol or two-character combo. Insert with the plus button or drag an item to the canvas, then review its placement. Motions can be retargeted onto a rig with matching control names and parent relationships; limb lengths adapt to that target. Symbols remain drawings on editable layers. Combos retain editable poses, attached effects and contact markers. Audio clips stay separate in the project.

Library entries persist in the project. **Export library** and **Import library** use `.astral-library.json` files inside the active project workspace. Imports receive fresh library item IDs. Files are validated and limited to 4 MiB and 64 items; one operation covers at most 1,201 frames. Oversized or invalid edits are rejected atomically.

## Effects, sound and camera

**Finish** offers Light, Heavy and Energy impact packs attached to a named character control. Optional imported WAV cues align with the contact frame or a selected action's contact/accent marker. The new flash, speed-line, dust and shockwave effects join the existing effects; every effect remains independently editable.

The detailed timeline shows separate effect and audio rows. Audio mode opens this timeline automatically. WAV waveforms reflect trimming, gain, fades and looping. Click a clip or effect to edit it in Finish. Offline Windows text-to-speech and dialogue remain accessible through **Voice and dialogue**. Playback and exports use the shared audio mixer; the before/after motion comparison is a visual preview.

**Camera** generates editable pan/zoom keys, optionally following a character with adjustable start/end offsets. Camera shake is available through impact packs and existing effects.

## Review and undo

Generated edits open a comparison of the original and suggested scene. Play or scrub the visual preview, inspect warnings, then **Accept** or **Cancel**. Accept commits one revision-checked undo transaction. If the document changes during preparation or review, preview it again. Invalid plans and cancelled reviews do not mutate the document.

Projects with new constraints, impact effects or timeline markers use schema 5. Older schemas remain readable. Older Astral binaries cannot open schema 5 projects; retain a separate original when moving between releases.

## MCP and CLI

`motion_tools` is available in the `animation` and `all` MCP profiles. Its typed request schema groups the same studio operations under `operation: "edit"`. Editing defaults to a non-mutating proposal. The result contains `plan.commands`, warnings, diagnostics, markers and the current revision. When a preview frame is requested, MCP returns separate Original and Proposed PNG image blocks rather than base64 strings in the structured result.

Example tool arguments after creating two characters:

```json
{
  "operation": "edit",
  "project": "PROJECT_UUID",
  "expected_revision": 2,
  "apply": false,
  "preview_frame": 12,
  "tool": {
    "task": "fight",
    "first": "FIRST_CHARACTER_UUID",
    "second": "SECOND_CHARACTER_UUID",
    "start": 0,
    "beats": [
      {"attacker": "FIRST_CHARACTER_UUID", "action": "punch", "frames": 24, "strength": 1},
      {"attacker": "SECOND_CHARACTER_UUID", "action": "counter", "frames": 24, "strength": 1}
    ],
    "style": "anime_fight",
    "impacts": true,
    "overwrite": false
  }
}
```

Review the result, then send its exact `plan.commands` to `apply_commands` at the returned revision. Alternatively set `apply: true` for an explicitly intended direct transaction. Preview validation precedes any commit.

Other tasks include `smart_pose`, `motion_brush`, `analyze`, `trajectory`, `tween`, `foot_lock`, `mirror`, `frame_batch`, `pose_batch`, `timing`, `frame_rate`, `marker`, `preset`, `rig`, `save_library`, `insert_library`, `remove_library`, `paste`, `camera`, `impact`, `edit_audio`, `edit_effect` and `library`. Separate operations provide `waveform`, `export_library` and revision-checked `import_library`. Inspect the generated tool schema for required fields and enum values; easing is `"linear"`, `"smooth"`, `"hold"` or a Bézier object.

The CLI accepts the same request in a JSON file through `astral request file.json`; wrap this payload as `{"operation":"production","request":{...}}`. Use `--live` to send it to the desktop document service. All file paths remain confined to the configured project workspace.

`scripts/smoke-astral-motion.mjs`, invoked by the complete MCP acceptance script, exercises a five-beat fight, camera keys, an impact sound, waveforms, library file roundtrip, a targeted motion-path revision, coaching, trajectories and undo/redo.
