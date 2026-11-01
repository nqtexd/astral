# Astral 0.5: drawing assistance, image mapping and offline voice

## Drawing and frame workflow

Draw normally in the Draw workspace. A blue ghost suggests a straight continuation or a fitted circular closure when the stroke fits well enough. **Tab / Accept** commits it as one undoable edit; **Esc / Dismiss** discards it. Drawing again discards the old suggestion. This fitting is geometric, with no neural inference or network request.

Open **Assist**, select a drawing layer, and use **Use current key as example** on at least three authored frames. Examples need corresponding strokes in the same order, with matching colors and widths. Select the range and generate inbetweens, or ask for a completion at the playhead. The local RBF model learns the vector trajectories. Source keys remain intact; occupied keys are skipped unless overwrite is selected. Generation commits one undoable transaction. Partial-frame completion preserves existing strokes and proposes missing strokes. It does not reconstruct arbitrary raster drawings or infer changing stroke topology.

The rig completion tool uses the bundled, trained 3,606-parameter masked pose network. Select the joints you have already posed; those joints stay pinned while the model proposes the others. Accept or dismiss the preview. The model supports Astral's canonical eleven-joint stick rig. It predicts joint positions, not raster images. Use action presets or learned motion for temporal animation.

**Settings → Drawing assistance** controls suggestions, fit threshold, continuation length, automatic example-based frame suggestions and a project-relative custom pose-model path. **Train a project pose model** trains another synthetic pose prior offline, stores it as a project asset and reports evaluation. It does not automatically train on imported artwork. Use **Use this pose model** after training to select it. See the model card distributed under `models/`.

## Import a stickman start frame

Use **Add → Stickman image**, **Import stickman** on Home, or drop a PNG/JPEG into Astral. Home asks for a project first. The import worker preserves the source image in project assets, segments the foreground, thins its skeleton and estimates eleven named landmarks. Drag the landmarks or edit their coordinates in **Review stickman joints**, then import the editable rig. The optional pose prior fills estimated landmarks. Animate the resulting rig with poses, actions or agent tools.

Use one clear, separated stickman on a simple or transparent background. Crossed limbs, clutter, disconnected limbs, perspective, multiple figures and textured generated art can produce incorrect or rejected mappings. The mapping score is a heuristic, not a calibrated probability. Review is required. Image-side left/right names refer to the picture. Mapping does not preserve raster texture as an animated sprite.

## Text and voice box

Dialogue supports regular, bold, italic and bold italic text, left/center/right alignment, line spacing, padding, font size, speech/thought bubbles, captions and typewriter reveal. These remain editable vector text. Rich formatting promotes the project to schema 4; previous project schemas still open. Text coverage follows the bundled Noto Sans fonts.

Open **Add → Voice and dialogue**. Choose an installed Windows voice, rate, pitch and volume; enter text and generate at the playhead. Astral uses Windows SpeechSynthesizer locally and writes a WAV asset. No API key or paid voice service is used. Windows must have the desired voice installed. The separate audio clip and optional matching dialogue are editable; the timeline extends to fit speech. Generation runs in a cancellable worker and commits audio, text and duration together as one undoable edit. Playback starts audio at the current playhead.

## MCP server profiles

**Settings → Agents → Copy MCP configuration** produces configuration for the actual executable and workspace. The portable package also supplies `mcp-servers.json`. If you move the extracted app, copy fresh configuration from Settings or update its executable paths. Both `projects` and `animation` profiles connect to the running native studio through its local bridge, so agents and the person share revisions and undo history. Start the studio first.

```powershell
astral.exe --workspace D:/AstralProjects mcp --profile projects
astral.exe --workspace D:/AstralProjects mcp --profile animation
```

For a standalone agent without the desktop, use one server:

```powershell
astral.exe --workspace D:/AstralProjects mcp --headless --profile all
```

Separate headless processes have independent in-memory documents. To hand off between them, save and reopen the project; they do not share unsaved sessions.

Project tools include `create_managed_project`, `list_saved_projects`, `open_project`, `inspect_project`, `save_project`, `close_project`, recovery and history. Managed creation saves a blank drawing project immediately and establishes its asset root. Dirty close requires explicit `discard:true` or saving first. Paths are relative to the configured workspace; after managed creation, media/model/save paths are relative to that project's folder.

Animation tools include `create_character`, `animate_character`, `add_drawing_layer`, `set_drawing_frame`, `suggest_drawing`, `complete_frame`, `generate_inbetweens`, `complete_pose`, `recognize_stickman`, `import_stickman_rig`, `train_pose_model`, `model_info`, `list_voices` and `add_voice`, in addition to existing command, drawing, effects, text, preview and export tools. Discover their generated schemas with `tools/list`. Frames are zero-based. Inspect before mutation and supply `expected_revision`.

Suggested sequence: create a managed project; create or map a character; apply actions; author corresponding cels; generate inbetweens; list voices and add speech; poll `job_status`; render and inspect a preview; save. For targeted changes, reuse entity IDs and issue only the replacement command. Completion defaults to a non-mutating proposal. Commit its commands with the returned revision. A stale revision rejects the entire transaction. Long operations return cancellable job IDs; do not assume completion until `job_status` reports it. Failed stale asset jobs may leave an unreferenced generated file; they do not partially edit the document.

## Research basis and limitations

The compact masked network is an engineering implementation for this rig. [Masked motion modeling research](https://arxiv.org/html/2308.07301v2) motivates visible/missing joint conditioning; Astral does not reproduce that paper's transformer. [Missing-marker reconstruction research](https://arxiv.org/html/1803.02665v4) also shows why neural performance should be compared against simple interpolation, especially for very short gaps. The shipped evaluation compares against a fixed rest-pose baseline on synthetic poses; it does not establish superiority to interpolation or professional animation tools.

The topology mapper follows the family of iterative skeleton thinning methods described in [scikit-image's skeletonization documentation](https://scikit-image.org/docs/0.25.x/auto_examples/edges/plot_skeleton.html). Native speech uses Microsoft's [SpeechSynthesizer](https://learn.microsoft.com/en-us/uwp/api/windows.media.speechsynthesis.speechsynthesizer), [installed voice list](https://learn.microsoft.com/en-us/uwp/api/windows.media.speechsynthesis.speechsynthesizer.allvoices) and [voice options](https://learn.microsoft.com/en-us/uwp/api/windows.media.speechsynthesis.speechsynthesizeroptions). No third-party model weights or external training dataset are included.
