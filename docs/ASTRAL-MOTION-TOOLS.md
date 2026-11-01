# Astral 0.2 — learned frames, dialogue and motion graphics

## Learn the frames between your drawings

The offline learner fits a multi-output RBF kernel-ridge model to your authored examples. It learns the trajectory of one selected sequence, using three to sixty-four distinct example frames. It runs in Rust without a downloaded model, Python, an API key or a cloud account. [Algorithm reference](https://scikit-learn.org/stable/modules/generated/sklearn.kernel_ridge.KernelRidge.html).

For a character:

1. Select a character and open **Rig**.
2. Pose it at frames such as 0, 15 and 30. Dragging a joint creates a pose key; **Key current pose** also captures the visible pose.
3. Click **Learn pose inbetweens** in the inspector. Play or scrub through the interval: Astral evaluates the learned motion for intermediate frames.
4. Adjust **Smoothing** and learn again to refit. Editing a pose deactivates the previous model so the application cannot use stale training data.
5. **Bake to editable pose keys** converts the learned motion to ordinary per-frame keys. Undo restores the original samples and model.

Rig learning uses unwrapped joint angles and lengths, rather than blending limb endpoints directly. Authored key poses and the two interval endpoints remain exact. Existing IK contacts and action clips still apply after the learned base pose.

For a drawing:

1. Open **Draw** and draw the same outline at three or more different frames. Use the same brush width and color.
2. Select those corresponding strokes in the inspector and choose **Learn stroke inbetweens**.
3. Astral aligns and resamples their paths, learns their point trajectories, and draws the intervening frames. Original authored outlines remain exact at their source frames.
4. Choose **Bake … into frames** for individual editable drawings. Enable **Edit stroke points**, choose a drawing, and drag its highlighted control points on the stage.

Source strokes must describe the same outline: open strokes match open strokes, and closed contours match closed contours. Resampling and alignment are preprocessing; the trajectory is learned by supervised regression. The model is specific to these vector samples and does not generate unseen artwork or interpolate arbitrary raster pictures. Two example frames are insufficient for learned mode. Baking is limited to 2,001 frames per operation; unbaked playback does not need materialized drawings at every frame.

## Dialogue

Open **Dialogue**, enter a speaker and text, then choose **Speech**, **Thought** or **Caption**. Set duration and typewriter speed; zero shows all the text immediately. **Add at playhead** creates an independent editable dialogue clip. Selecting a character attaches speech/thought dialogue to its root; captions use world coordinates by default.

Select an existing dialogue in the inspector to update its text/style, position, dimensions, timing, font size, fill and text color. Text wraps inside the box; overflow ends with an ellipsis. The full text remains in the project. Typewriter reveal uses grapheme boundaries and stable layout.

Text is shaped with HarfRust and rendered from embedded Noto Sans vector outlines, including in PNG and video exports. It does not depend on fonts installed on the exporting PC. The font has finite script/emoji coverage; additional fallback fonts remain future work. [HarfRust](https://github.com/harfbuzz/harfrust).

## Motion graphics and clouds

Open **Graphics** to add clouds, rectangles, ellipses, stars, arrows, rings, bursts and lower thirds. Presets provide hold, cloud drift, slide/fade, pop/overshoot, full rotation and draw-on animation.

Select a graphic, move the playhead into its clip, and adjust position, scale, rotation or opacity to add a transform key. **Key this transform** explicitly records the current values. Easing belongs to the outgoing key: choose linear, smooth, hold or a custom cubic Bezier with editable controls. Rotation is shown in degrees in the UI and stored in radians in the API.

Width, height, color, foreground/background placement and draw-on remain editable. Lower thirds contain editable text. Clouds are vector artwork rendered behind the characters by default. All these layers use the shared scene display list for the desktop, previews and exports.

## Agent access and project compatibility

MCP adds `learn_motion`, `learn_drawing`, `add_dialogue` and `add_motion_graphic`. Advanced edits use the corresponding typed commands in `apply_commands`: `train_motion`, `train_drawing`, `bake_motion`, `bake_drawing`, `replace_drawing`, `add_dialogue`, `replace_dialogue`, `add_graphic`, `replace_graphic` and `set_graphic_key`.

All edits obey existing project/revision checks, atomic validation and undo/redo. Models, sources, coefficients, dialogue and graphics survive save/reopen. Source edits invalidate affected models; retiming refits them against the new authored timestamps. Exports can render learned motion directly without baking.

These additions use project schema 2. Astral 0.2 reads schema-1 projects; Astral 0.1 refuses schema-2 projects instead of silently dropping the new features. Upgrade backups of open documents are saved under `upgrade-backups/0.2.0/` in the configured workspace.

The default **Motion lab** project contains the actual three pose examples and three ribbon drawings used for its learned animation, along with dialogue, clouds and keyed graphics. Inspect and change them to explore the workflow.
