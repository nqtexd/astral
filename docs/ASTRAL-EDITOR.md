# Projects and frame editing in Astral 0.4

Astral opens at **Projects**. The editor starts with a blank page when you create a project; the Motion lab demo is available through the CLI rather than loaded automatically.

## Project management

- **New project**: choose a name, landscape/portrait/square or custom canvas, fps, duration, background and destination folder. Astral creates a separate folder containing the saved project and a first drawing layer.
- **Open project**: use the native file picker to open an .astral file anywhere on the computer. The chosen document configures its own confined media/export root. Older projects whose media lives in the configured shared workspace can still resolve that workspace.
- **Recent projects**: saved locations, preview thumbnails, search and favorites persist across sessions. Removing an entry from Recent does not delete its file.
- **Save** or **Ctrl+S**: save the current document atomically at its current location.
- **Save as** or **Ctrl+Shift+S**: select a destination file. Registered linked media is copied with its relative paths. Conflicting existing media is reported before project replacement, preserving unrelated files. An .astral file contains the document; keep its adjacent asset folders when moving it manually.
- **Project settings**, in File: rename the project, change its timeline length or background. Shortening is rejected if animation would fall outside the new range.
- **Document tabs**: switch between open projects. Unsaved state is written in plain text. Closing a dirty tab asks to Save, Discard or Cancel; closing the application offers Save All.
- **Recovery**: autosaves are written in each document's recovery folder. Close the open document, then use the project's **More > Recover autosave** action in the library. Recovery opens the autosave as unsaved; Save keeps the recovered changes.

New projects use 24 fps and five seconds by default. Projects created through CLI/MCP retain their requested settings. New/Open shortcuts are Ctrl+N/Ctrl+O; close the current document with Ctrl+W.

## Drawing workflow

The default Draw view has a drawing toolbar, named layers, a large canvas and a thumbnail frame strip.

1. Draw with **Pencil**. A tap makes a dot; a drag records a stroke. Choose a color and brush size in the layer panel.
2. Choose **Blank** to insert an empty drawing immediately after the current frame, or **Duplicate** to start the next frame from the current drawing.
3. Move through the animation using the strip, Previous/Next or the arrow keys. Space plays the project.
4. Use **Copy/Paste** or Ctrl+C/Ctrl+V for the active layer's drawing. Ctrl+D duplicates it into the next frame.
5. Use **Hold** to set the selected drawing key's exposure in frames.

Drawings hold until the next drawing key on the same layer. A small square on a thumbnail identifies a key. Drawing, moving or erasing on a held frame creates an independent key at the playhead; the earlier drawing remains unchanged. Blank and Duplicate insert on the active layer, shifting that layer's later keys when necessary. Other layers, sound and scene tracks retain their timing. The project extends if needed.

**Clear** creates an explicit blank drawing at the current frame. **Delete** removes a drawing key at that exact frame, so the preceding drawing can hold through it. Delete does not shorten the project or remove other tracks. Hold moves subsequent keys on the selected layer; shortening a final hold inserts a blank terminator. All of these operations use the same undo/redo history as other document edits.

**Eraser** removes whole strokes, including strokes crossed during a fast gesture. **Move** translates the active layer's drawing; it copies a held drawing before changing it. Each gesture is one undo step. Pencil/Eraser/Move/Hand shortcuts are B/E/G/H.

Use wheel zoom, middle-button drag or **Hand** to navigate the canvas. **Fit** or Home resets the editor view. View pan/zoom does not modify the animated scene camera.

## Layers and detailed editing

Layers have names, ordering, visibility, locks and opacity. Add, rename, reorder or delete layers from the layer panel; layer deletion is undoable and restores its drawings. Locks prevent drawing, moving, erasing and deletion. The panel scrolls to keep controls accessible at smaller window sizes.

Onion skin uses neighboring drawing keys on the active visible layer. Configure before/after counts and opacity. Muted blue and red ghosts distinguish previous and next drawings without changing the rendered project.

**Detailed timeline** switches from the thumbnail strip to drawing-layer exposures and the existing character, action, graphics, dialogue, VFX and audio tracks. Drag a drawing exposure to move its key in time. Occupied destination keys are rejected rather than overwritten. Use the mode selector to access Animate, Rig, Effects, Audio, Graphics and Dialogue. Existing IK, editable action clips, legacy vector-learning tools, motion graphics and export remain available.

Drawing layers stack with each other in the layer-list order and render in the drawing portion of the existing scene stack, below characters. They do not reorder character, VFX or dialogue tracks. The cel editor uses vector strokes; it is not a raster painting or full 3D workspace.

## Agent interface and project compatibility

The shared command service adds:

- add_layer / set_layer / move_layer
- set_cel / delete_cel / move_cel
- shift_cels / set_exposure

Use these through MCP apply_commands or CLI transaction JSON. set_cel replaces the active frame's complete stroke list and preserves an existing key's identity. To draw on a held frame, inspect its strokes, copy those values and add the new stroke at the target frame. Mutations require the expected document revision. move_cel preserves the key ID and rejects occupied destinations.

Drawing cels require project payload **schema 3**. Astral 0.4 opens schemas 1, 2 and 3; older applications reject schema 3 rather than silently losing cel data. Older drawing, rig, learned-motion and graphics data remains intact. New cel layers and the previous independently timed vector-drawing objects are separate editing models; the existing learned-vector workflow operates on those earlier drawing objects.

Native file selection configures project locations. Agents still receive confined relative-path operations, not arbitrary filesystem access. list_projects includes each open document's root and file. Exports and imported assets are relative to that document's root. For saved files in a managed folder, point the CLI --workspace at that project folder.

Example command body:

~~~json
{
  "project": "PROJECT_UUID",
  "expected_revision": 0,
  "commands": [
    {
      "type": "add_layer",
      "layer": {
        "id": "LAYER_UUID",
        "name": "Ink",
        "visible": true,
        "locked": false,
        "opacity": 1
      }
    },
    {
      "type": "set_cel",
      "layer": "LAYER_UUID",
      "frame": 0,
      "strokes": [{
        "points": [{"x": 100, "y": 200}, {"x": 240, "y": 120}],
        "color": [30, 30, 30, 255],
        "width": 6
      }]
    }
  ]
}
~~~

## Design references

TinyFish research reviewed 48 unique URLs in one focused pass, including 13 fetched official documentation pages. The implementation draws on FlipaClip's simple canvas/mini-strip versus detailed timeline split, and Blender's active-layer auto-keying, drawing exposures, locks and navigation. Astral keeps its own design and branding.

- [FlipaClip's current layouts](https://support.flipaclip.com/article/113-newversion) and [frame/timeline changes](https://support.flipaclip.com/article/114-whatsnew).
- [FlipaClip copy and paste](https://support.flipaclip.com/article/70-copying-and-pasting-frames) and [onion skin](https://support.flipaclip.com/article/17-onion-skinning).
- [Blender Grease Pencil animation](https://docs.blender.org/manual/en/latest/grease_pencil/animation/introduction.html), [Dope Sheet](https://docs.blender.org/manual/en/latest/editors/dope_sheet/modes/grease_pencil.html) and [2D navigation](https://docs.blender.org/manual/en/latest/editors/image/navigating.html).

## Drawing assistance and voice (0.5)

Use **Assist** for example keys, completion proposals and range generation; **Settings** for suggestion preferences, UI scale, new-project defaults, voice parameters and MCP configuration. **Add** exposes stickman image import, voice/dialogue and text. PNG/JPEG drops open a landmark review before rig creation. See [ASTRAL-ASSISTANCE.md](ASTRAL-ASSISTANCE.md) for the workflow and supported inputs.

## Interface refresh (0.6)

Toolbar, frame and layer actions use original vector icons with tooltips and accessible names. Use the folder/layers button to return to projects, the ellipsis for file actions, plus for adding content, sliders for view options, and the linked-node icon for assistance. Settings contains automatic scale fitting. Primary actions stay labelled. See [the design guide](ASTRAL-DESIGN.md).


## Motion studio (0.7)

The linked-node toolbar icon now opens Motion studio. Frame completion remains available from its Pose tab. Beginner controls keep the inspector focused; Pro controls are available in View and Settings. Shift-click thumbnails to select ranges. Audio mode shows individual waveform tracks; effects retain separate rows. See [the motion studio guide](ASTRAL-STUDIO-TOOLS.md) for previews, choreography, libraries and timing.
