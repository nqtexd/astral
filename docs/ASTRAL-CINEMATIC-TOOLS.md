# Cinematic 2D production tools

Astral now has native **multiplane camera staging, eased camera segments, alpha-derived shadows, matte rim lighting, linear-light bloom and lift/gamma/gain grading**. These work on authored drawing, raster and vector artwork and can be controlled from the desktop or MCP. They extend the comic effects rather than replacing the drawing/animation workflow.

![Original staging and lighting test](showcase/cinematic-lab/finished.png)

This is the original **Amber passage** technical study. [Open the editable project](showcase/cinematic-lab/amber-passage.astral), [inspect the frame samples](showcase/cinematic-lab/frames.png), or [compare the ungraded image](showcase/cinematic-lab/before.png). The project runs at 24 FPS with 12 FPS traveler artwork; the compact GIF samples 12 FPS. It demonstrates layered depth and finishing, not a film-quality acting benchmark.

## Workflow

1. Import or draw original production artwork on separate foreground, character, midground and background layers. Retain editable source cels, contours and raster meshes.
2. In **Compositing → Cinematic staging**, adjust each artwork layer's **Pan response** and **Zoom response**. A factor of 1 follows the camera normally, 0 stays anchored to the camera's initial position/scale, and values above 1 exaggerate near-plane motion. The first camera key anchors the default depth transform, so enabling parallax does not move the first frame. MCP can set an explicit translation anchor.
3. Author camera keys with the existing camera tools. **Camera segment timing** adds linear, eased or hold/cut timing at each segment start. MCP also supports a validated cubic Bézier timing curve. Camera timing remains at project cadence while artwork can animate on twos.
4. Build a composition from drawing/raster/vector inputs and set its output. **Rim light** adds silhouette-edge lighting; **Soft shadow** adds a tinted, offset, blurred alpha shadow behind its source. For full-scene finishing, blend the complete scene in the graph first.
5. Add **Bloom** for emissive highlights and **Color grade** for exposure stops, contrast, saturation and per-channel lift/gamma/gain. The threshold is in linear light, not encoded sRGB. All four stages have a mix slider, **Key mix at playhead**, and **Clear mix keys**.
6. Review full-resolution frames and moving previews before final export. MCP's sampled image sheets and luminance/change measurements help an external agent inspect a shot; they do not evaluate acting or certify artistic quality.

Depth staging affects drawing, raster and graphic source layers before clipping/rasterization. It also works inside existing composition inputs. This is a 2D multiplane approximation with independent translation and scale responses, not perspective geometry, a 3D camera, automatic depth estimation or depth-of-field simulation. Attached artwork still uses its authored rig pose; its own camera response changes screen placement. Use consistent responses for artwork that must remain attached.

## MCP

Discover `cinematic_tools` in the all/animation profiles. All mutations default to `apply:false`, return validated typed command proposals and before/after PNGs at the requested preview frame, and require `expected_revision`. `apply:true` commits one undoable transaction; stale writes fail. Undo uses the existing `history`/undo tools.

### Stage a camera and layer depth

```json
{
  "operation":"stage_shot",
  "project":"PROJECT_UUID",
  "expected_revision":8,
  "keys":[
    {"frame":0,"center":{"x":960,"y":540},"zoom":1},
    {"frame":71,"center":{"x":1080,"y":510},"zoom":1.08}
  ],
  "easing":"smooth",
  "layers":[
    {"layer":"BACKGROUND_UUID","depth":{"parallax":0.2,"zoom":0.2}},
    {"layer":"CHARACTER_ARTWORK_UUID","depth":{"parallax":1,"zoom":1}},
    {"layer":"FOREGROUND_UUID","depth":{"parallax":1.4,"zoom":1.4}}
  ],
  "preview_frame":36,
  "apply":false
}
```

`stage_shot` **replaces the complete camera track**, preserving other authored animation. Provide the full desired track; 1–64 increasing keys are supported. The same easing applies to each submitted segment. For individual segment timing use `apply_commands` → `set_camera_easing` with `frame` and `easing` (or null to reset). A custom curve uses the existing externally tagged form `{"bezier":{"x1":0.25,"y1":0,"x2":0.75,"y2":1}}`; x and camera y control values must remain within 0–1.

`set_layer_depth` takes `layer` and `depth`, or null to restore ordinary camera behavior. `set_cinematic_camera` accepts the complete `keys` list and an `easing` map keyed by segment-start frame. Retiming remaps camera keys, easing associations and finishing mix keys together. Removing source entities also removes their depth settings.

### Finish a composition

```json
{
  "operation":"finish",
  "project":"PROJECT_UUID",
  "expected_revision":9,
  "composition":"COMPOSITION_UUID",
  "preview_frame":36,
  "apply":false,
  "stages":[
    {"effect":{"effect":"bloom","threshold":0.65,"knee":0.15,"radius":18,"intensity":0.6},"mix":1},
    {"effect":{"effect":"grade","exposure":0,"contrast":1.05,"saturation":0.9,"lift":[0,0,0.01],"gamma":[1,1,1],"gain":[1.03,1,0.98]},"mix":1}
  ]
}
```

`finish` appends 1–4 stages to the enabled composition's output, retaining every other composition and node. Each `CinematicStage` has an `effect`, `mix` (0–1), and optional `keys:[{"frame":0,"strength":0},{"frame":12,"strength":1}]`. Mix keys interpolate linearly and override the static mix. The nested `effect` tag belongs to `CinematicEffect`; the outer `effect` field belongs to its animated stage.

Other effect objects:

```json
{"effect":"drop_shadow","offset":{"x":8,"y":12},"radius":12,"color":[6,9,28,255],"opacity":0.6}
{"effect":"rim_light","offset":{"x":-3,"y":2},"radius":2,"color":[255,185,110,255],"intensity":0.65}
```

For direct graph editing, use `CompositeOp` with `kind:"cinematic"`, `source:NODE_UUID`, and `stage:CinematicStage`. Preserve unrelated graphs when using `set_compositions`.

### Inspect a sequence

```json
{"operation":"review","project":"PROJECT_UUID","frames":[0,24,48,71]}
```

The tool returns an actual rendered sheet with up to six ascending samples, frame/time metadata, mean linear luminance, fractions near black/white, and change from the previous sampled image. Tiles are at most 384 pixels on their longest side; rows follow the returned frame order. Highlight/silhouette warnings are prompts for visual inspection, not automatic failures. Use `render_preview` for delivery-resolution checks, `export_media` for moving previews, and contact/interaction checks for declared actions. The older geometric `review_animation` bounds do not account for layer parallax; inspect rendered frames for final framing.

## Rendering, scope and migration

- Finishing decodes premultiplied sRGB inputs into floating-point linear light, performs its operations, then encodes back to premultiplied sRGB textures. CPU, GPU viewport and export share those textures. The existing general-purpose blend nodes retain their prior sRGB behavior.
- Bloom extracts bright energy with a soft knee, then uses three separable box passes for a bounded soft blur. Shadows blur alpha rather than colored pixels and composite under artwork with source-over. Rim light adds color along a shifted/softened alpha boundary while preserving the artwork's coverage.
- These are **SDR finishing tools** with 8-bit sRGB stage boundaries, not HDR/ACES, normal-map lighting, physical cast shadows, volumetrics or a path tracer. Bloom expands transparent coverage and may crop at canvas edges; provide composition overscan where needed.
- Native 3840×2160 fits the eight-megapixel frame limit. Finishing stages together may cover at most 32 megapixels; the existing 64-megapixel total graph allocation limit also applies. Large graphs can require substantial CPU memory/time. Export scaling does not add source-raster detail.
- New staging/finishing commands upgrade the document to **schema 13**. Older Astral builds reject those projects; retain originals. Ordinary older projects retain their original camera timing and render behavior until these controls are used. No installer or release certification is implied by a source update.
- Rich assets, appealing design, pose/acting choices, cleanup, coherent lighting direction, editorial rhythm and sound still require creative authorship and iteration. The app does not contain a general film-generation model. Pixel thresholds cannot prove film quality.

Reproduce the study with `cargo run -p astral --locked --example cinematic_lab -- docs/showcase/cinematic-lab`.

## Technical sources

The [Khronos sRGB reference](https://registry.khronos.org/OpenGL-Refpages/gl4/html/glEnable.xhtml) grounds linearization before blending. [Jorge Jimenez's production postprocessing notes](https://www.iryoku.com/next-generation-post-processing-in-call-of-duty-advanced-warfare/) discuss bloom filtering and stability; Astral uses an explicitly bounded SDR soft-threshold implementation rather than that HDR pipeline. The [W3C source-over equations](https://www.w3.org/TR/compositing-1/#porterduffcompositingoperators_srcover) define alpha compositing. [Godot's parallax guide](https://docs.godotengine.org/en/stable/tutorials/2d/2d_parallax.html) documents camera scroll factors; Astral's anchored pan/zoom formulas are an implementation adaptation. The [W3C easing specification](https://www.w3.org/TR/css-easing-2/#cubic-bezier-easing-functions) explains cubic Bézier time inversion.
