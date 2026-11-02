# Astral agent interface

Astral exposes editing primitives, not a built-in language model. An external agent plans shots, applies typed commands, renders previews and iterates. All changes remain editable.

## Plan and review before export

Astral 0.8 adds `storyboard` and `review_animation`. Plan the cast, shots and continuity first; declare contact cues, inspect moving shot previews and review misses/torso overlaps before a long render. Use `set_graphic_bindings` through `apply_commands` for costumes that follow edited rig joints. [Detailed workflow and schemas](ASTRAL-STORYBOARD.md).

## Connection

Use the built `target/release/astral.exe` with these process arguments:

```json
{
  "mcpServers": {
    "astral": {
      "command": "D:/StykAnimate-App-main/target/release/astral.exe",
      "args": ["--workspace", "D:/AstralProjects", "mcp"]
    }
  }
}
```

The default connects to the running studio via a workspace-specific Windows named pipe. `mcp --headless` owns a separate document service. Opening the same workspace does not automatically share headless state with the desktop. The MCP process uses stdout exclusively for protocol traffic.

## Authoring loop

1. Discover tools and call `list_presets` to inspect supported capabilities and limitations.
2. `list_projects` and `inspect_project` find the target project, revision, rig names and current clips. Or call `create_project` with width, height, fps and duration.
3. Send `apply_commands` with `{project, expected_revision, commands}`. It is one atomic undo step. Store returned IDs and revision; inspect again after any stale-revision error.
4. Call `render_preview` with `{project,frame,width,height}`. It returns an actual PNG image, plus metadata. Inspect several event frames before exporting.
5. Refine one action with `replace_action`, retaining its ID. Add separate effect/audio commands at action markers.
6. `save_project` writes `.astral`; `export_media` (or the compatible `export_video`) returns a job ID. Poll `job_status`, or `cancel_job`.

Frames are zero-based. A 10-second project at 30 fps has `duration:300` and frames 0–299. Clip `start` and `end` are inclusive. Action clips on the same character cannot overlap. Translation accumulates and persists after a clip. Named pose commands describe the desired visible pose, even while an action is active.

## Typed command example

Astral 0.2 also exposes **learn_motion**, **learn_drawing**, **add_dialogue** and **add_motion_graphic**. The learning tools need 3–64 corresponding authored examples. Use `bake_motion` or `bake_drawing` through `apply_commands` for editable per-frame output; source changes invalidate the learned model. Dialogue and graphics remain independent editable tracks. See [motion-tool workflows](ASTRAL-MOTION-TOOLS.md) for inputs, limits and project compatibility.

```json
{
  "project": "PROJECT_UUID",
  "expected_revision": 0,
  "commands": [
    {
      "type": "add_character",
      "name": "Nova",
      "position": {"x":650,"y":650},
      "color": [167,139,250,255]
    }
  ]
}
```

Use the returned character UUID in a later action:

```json
{
  "type":"apply_action",
  "clip": {
    "id":"NEW_UUID",
    "character":"CHARACTER_UUID",
    "kind":"run",
    "start":0,"end":89,
    "intensity":1,"distance":210,"direction":1,
    "markers":{"arrival":89}
  }
}
```

Supported commands include custom rigs, visible pose keys and easing, character properties, actions, effects, contacts, camera keys, drawings, assets, audio clips, named poses, selected-joint pose blending/mirroring, proportional retiming, removal, and project properties. Generate authoritative schemas with `astral schema` rather than guessing fields.

Assets use relative workspace paths. Audio mixing supports mono/stereo PCM or float WAV and resamples to stereo 48 kHz. Audio clips have independent trim offsets, duration, gain, fades and looping. Retiming shifts audio cue starts while preserving pitch and source duration. Importing an image currently registers the asset only; sprite rendering is pending.

## CLI and headless workflows

`astral request request.json --live` sends the same typed service request to the desktop. `astral request - --live` reads JSON from stdin. Without `--live`, a request runs in a fresh process; use MCP for multi-request headless sessions.

`astral apply project.astral batch.json` opens the project, applies one validated transaction and atomically saves it. `inspect`, `validate`, `render`, `demo`, and `export` operate on saved files. All project/media paths are relative to `--workspace`; the CLI request/batch input file is a local caller-provided file.

Do not label a legacy archive as a native conversion. `import_legacy` returns explicit warnings; `export_original` through the service/CLI extracts the unchanged archived bytes and does not include Astral edits.

## Social/media export

Astral 0.3 adds `list_export_presets` and `export_media` for 21 discoverable MCP tools. Use preset IDs and explicit fit/fill framing, then poll the returned job. Output extension selects MP4, WebM, GIF, PNG, JPEG, PNG-sequence ZIP or WAV. Resolution, fps, quality and source-frame range are optional overrides. See [export guide and examples](ASTRAL-EXPORTS.md).

## Drawing cels and configured document roots

Astral 0.4 adds add_layer, set_layer, move_layer, set_cel, delete_cel, move_cel, shift_cels and set_exposure to apply_commands. Inspect held drawings before copying their strokes to an independent key at the target frame. Drawing layers/cels use project schema 3. Native project selection configures its asset root; list_projects now reports root and file for each document. Asset, save and export paths are confined relative to that root. See [editor command examples](ASTRAL-EDITOR.md).

## Version 0.5 project and animation profiles

`mcp --profile projects` exposes ten project lifecycle tools. `mcp --profile animation` exposes 35 editing/animation tools. Default `all` exposes 39 tools. Both profiles share the running desktop service unless `--headless` is specified. Use `create_managed_project` to create and immediately save a blank drawing project with its own media root. The full workflow, new typed tools and path semantics are in [ASTRAL-ASSISTANCE.md](ASTRAL-ASSISTANCE.md).

The trained masked-pose model, example-based frame learner, reversible stroke fitting, reviewed stickman mapping and Windows offline `add_voice` tool work locally. Optional external speech providers are configured through Settings → Voice; see [the 0.14 workflow](ASTRAL-0.14-WORKFLOW.md#voice-providers). Inspect schemas and returned method/limitations rather than treating all assistance as neural image generation.


## Motion studio (0.7)

`motion_tools` exposes the same bounded editing proposals as the native editor, including constrained poses, choreography, paths, coaching, batch frames, libraries, contact effects, camera keys and audio edits. Preview results include separate original/proposed images and structured commands. See [the typed workflow and example](ASTRAL-STUDIO-TOOLS.md#mcp-and-cli).
