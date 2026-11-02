# Comic 2D animation

Astral's native drawing, custom vector contours, raster sequences and deformation layers can now use an editable comic finish. This is an animation/compositing workflow for authored artwork; the built-in stickman director remains specialized. External agents can create and animate arbitrary artwork through the existing drawing, graphics and raster tools.

## Desktop

Open **Compositing**, create an empty composition, add a drawing/raster/graphic scene input, and set that input as the output. Add **Halftone**, **RGB split** and/or **Glitch**. Each new effect consumes the current output. Controls expose dot spacing/angle/ink, channel offsets, band height/displacement/update rate/seed, and strength. **Key strength at playhead** inserts or replaces a linear strength key. Clear keys to return to constant strength.

**Layer frame rates** selects independent cadence for drawing layers, raster layers, vector graphics and characters. Select **Project** to restore continuous project sampling. A 12 FPS layer in a 24 FPS project samples frames 0, 0, 2, 2, 4, 4. Camera, audio and other layers keep their own clock. Cadence uses the absolute project clock; source visibility ranges retain their authored boundaries. Attached artwork follows its rig's cadence; choose matching rig/artwork rates when needed. Rates above a subsequently lowered project FPS are capped at evaluation time.

All changes are transactional and undoable. These features upgrade projects to **schema 12**. Older builds cannot open schema 12 projects; retain originals.

## MCP

Discover `comic_tools` in the all/animation profiles. An example request after obtaining the current project revision and a source layer ID:

```json
{
  "operation": "create_look",
  "project": "PROJECT_UUID",
  "expected_revision": 4,
  "source": "DRAWING_RASTER_OR_GRAPHIC_UUID",
  "layer_fps": 12,
  "apply": false,
  "styles": [
    {"effect":"halftone","spacing":8,"angle":0.7853982,"ink":[15,12,40,255],"strength":0.6},
    {"effect":"rgb_split","offset":{"x":4,"y":0},"strength":0.7},
    {"effect":"glitch","displacement":24,"band_height":12,"seed":42,"rate":12,"strength":0,
     "keys":[{"frame":0,"strength":0},{"frame":12,"strength":1},{"frame":18,"strength":0}]}
  ]
}
```

Frame keys must fit the project duration. `create_look` returns a validated command proposal and composition ID; `apply:true` commits it in one revision. Preview IDs become real only when that returned proposal is committed. Sources already in a composition are rejected to preserve their graph; edit the existing graph using `apply_commands` → `set_compositions`. `CompositeOp` uses `kind:"comic"`, `source:NODE_UUID`, and the same nested `style` object above. Chain nodes or blend independently styled inputs for multilayer scenes. Preserve unrelated compositions when replacing the composition list.

`apply_commands` also accepts `{"type":"set_layer_frame_rate","layer":"LAYER_UUID","fps":12}`. Use `fps:null` to reset. All writes require the current revision.

Inspect actual pixels with `comic_tools` operation `inspect_frame`, `project`, and `frame`. It returns an inline PNG (maximum 512 pixels on its longest side), source revision and coverage information. Use existing `render_preview` for larger images and `export_media` for moving previews and final output. Effects are baked into the shared display list, so CPU/GPU preview and export receive the same composited textures.

For automatic refinement, use operation `refine` with `project`, `expected_revision`, `composition`, `frames:[0,12,18]`, `max_difference:0.08`, `rounds:3`, and `apply:false` (default). It renders the selected frames against the same graph with its comic strengths set to zero, then halves strength and strength keys until the worst sampled mean RGB difference over black meets the budget or the round limit. Results include before/after images, measurements, budget status and an undoable proposal. `apply:true` commits the final proposal, including when the reported budget is still unmet. This reduces measured distortion; composition, acting and artistic appeal require visual review. Unsampled frames are not certified.

## Implementation and limits

- RGB split shifts red and blue in opposite directions in source-pixel units. Alpha expands to cover shifted channels, preserving valid premultiplied pixels on transparent artwork. Content shifted beyond canvas edges is cropped.
- Glitch uses deterministic seeded horizontal bands and a separately stepped update rate. Strength is keyframeable; the same project/frame/seed produces the same pixels. This is a 2D approximation of dimensional breakup.
- Halftone is a rotated, antialiased luminance dot screen with adjustable colored ink. It darkens source art while retaining its alpha. It is a CPU image shader, shared as a texture across rendering backends; it is not a native GPU fragment shader or a four-plate CMYK print simulation.
- Effects stay editable in compositing DAGs and obey existing graph allocation/cycle/reference limits. Animated graph cache keys include frame time so held source art cannot freeze animated effects. Native canvas-size processing can be expensive on large graphs.
- An effect preset does not provide film-level character design, performance, lighting, hatching or multi-view rendering. The application does not contain a general text-to-film model.

## Research

The official [Three.js RGBShiftShader](https://github.com/mrdoob/three.js/blob/master/examples/jsm/shaders/RGBShiftShader.js) demonstrates opposite channel offsets. The alpha expansion here is specific to transparent 2D layers. [Maxime Heckel's halftone implementation](https://blog.maximeheckel.com/posts/shades-of-halftone/) and [Alexander Ameye's halftone shader notes](https://ameye.dev/notes/halftone-shader/) describe rotated dot screens and coverage. No external shader code is bundled.
