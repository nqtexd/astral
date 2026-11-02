# Astral 0.14 workflow

This Windows early-access update expands the native editing and migration tools. It is **not a declaration of full Moho Debut parity or lossless legacy semantics**. The [readiness inventory](../platforms/release/readiness.json) remains explicit about remaining work.

## Voice providers

Open **Settings → Voice**. Choose a provider, set its model/voice IDs, and save the provider settings before generating speech. Changing the form does not silently change a queued speech job. Each job captures the configured endpoint/model/voice settings when it is submitted.

| Provider | Setup | Behavior |
| --- | --- | --- |
| Windows offline | Choose an installed voice. | Uses Windows SpeechSynthesizer locally. Rate, pitch and volume work without an API key. |
| ElevenLabs | Enter an API key and voice ID. The initial model ID is `eleven_multilingual_v2`; it remains editable. Enable external requests and choose **Save provider**. | Calls ElevenLabs’ text-to-speech endpoint, requests 24 kHz PCM, and writes an editable WAV clip. Speed range is 0.7–1.2. |
| Compatible speech API | Enter a full speech endpoint, model and voice ID. Add a key if the server requires one. | Sends `input`, `model`, `voice`, `response_format: "wav"` and `speed` to an OpenAI-compatible speech endpoint. A local example is `http://127.0.0.1:8880/v1/audio/speech`. This is an adapter contract, not support for every vendor’s different API. |

Compatible speech requests are limited to 4,096 characters; other speech requests retain Astral’s 5,000-character bound.

External speech is optional and disabled initially. **Generate voice at playhead** sends the dialogue to the selected provider; hosted services may charge credits. A local endpoint receives the dialogue locally. Astral does not automatically generate speech when a project opens. External pitch must remain 1.0; use the resulting audio clip’s pitch/speed control afterward if needed.

On Windows, keys are stored in **Windows Credential Manager**, outside projects and repository files. ElevenLabs has its own credential; compatible-provider credentials are scoped to the configured URL origin so changing hosts does not forward a stored key to the new host. Compatible endpoints require HTTPS, with HTTP allowed for loopback servers. Redirects are disabled. Provider error bodies are not echoed into project/job reports.

**Remove stored API key** removes the selected stored credential. Environment overrides remain under the user’s control: `ELEVENLABS_API_KEY` and `ASTRAL_VOICE_API_KEY`. On other desktop builds, session keys and these environment variables are supported; Windows Credential Manager persistence is Windows-specific. Credentials are never included in `.astral` projects, MCP parameters or preview resources.

The non-secret provider configuration is `Astral/voice.json` beside the encoder settings under the local application-data directory (home-directory fallback on other systems). Provider model/voice/endpoint profiles are remembered independently. Do not put API keys in this JSON file or in endpoint query parameters.

Generated speech stays a separate audio clip, with optional matching editable dialogue. Long work returns a cancellable job. Cancelling after a provider has received a request cannot undo any provider charge. A stale project revision prevents a partial document commit.

The live ElevenLabs service requires the user’s credentials and was not billed or exercised during the local test run. Adapter tests use a local HTTP server. The request contract follows [ElevenLabs’ official create-speech documentation](https://elevenlabs.io/docs/api-reference/text-to-speech/convert). The compatible JSON fields follow the [official Speech API reference](https://developers.openai.com/api/reference/resources/audio/subresources/speech/methods/create/); provider/model availability can change independently of Astral.

### Through MCP

Use the GUI and MCP/CLI executable from the same Astral version. Local bridge channels are versioned so new requests cannot silently reach an older running studio.

After configuring the provider locally, an agent calls `add_voice`. Discover the schema with `tools/list`; never supply keys in the conversation or tool arguments.

```json
{
  "name": "add_voice",
  "arguments": {
    "project": "PROJECT_UUID",
    "expected_revision": 12,
    "text": "The door is open. Keep moving.",
    "start": 48,
    "options": {
      "provider": "elevenlabs",
      "rate": 1.0,
      "pitch": 1.0,
      "volume": 0.9
    },
    "dialogue": true
  }
}
```

Use `provider: "offline"` to explicitly select installed Windows speech, or `"compatible"` for the configured compatible profile. Omitting the provider uses the saved active provider. `list_voices` lists installed Windows voices; external voice IDs come from their provider. Poll `job_status`, inspect the resulting clip/dialogue and save the project.

## Drawing and point editing

In **Draw**, the Brush section offers solid, marker, dashed and stipple styles. Closed fill uses the stroke outline as a filled shape. Cubic smoothing evaluates a Catmull–Rom cubic curve through the stored points. It is not a full independent Bezier tangent-handle editor.

**Pressure controls width** records normalized pressure reported by the input device. A mouse uses full width. Pressure samples remain in the editable vector stroke; the viewport and export render the same samples. Device/driver delivery varies, and physical tablet certification remains unfinished. Pattern marks and cubic samples have bounded budgets to reject pathological projects.

Choose **Edit points**, then drag a point on the drawing. The change is one undoable edit, isolated to the current frame by default. Editing a point on an imported skinned stroke detaches that stroke on that cel while preserving its visible position. Other cels and rig controls remain available.

Stroke completion remains an explicit ghost proposal. Uniform-pressure strokes can be completed without losing their width. Nonuniform pressure/rig-bound/image strokes are not silently replaced by geometric completion. Example-based frame learning retains consistent brush/fill/curve styles; pressure profiles are resampled from the first example rather than learned as a separate expressive performance.

## Deformation and correctives

Import an image in **Artwork**, select its layer and add a deformation mesh.

- **Soft pins** retain the earlier inverse-distance translation behavior.
- **Preserve rotation & scale** uses moving least-squares similarity deformation. It can reproduce rigid rotations/scaling from the pin constraints, unlike translation blending alone.
- **Curver** adds an animated cubic centerline. Its four controls bend the image strip; offsets across the strip follow the curve normal. A straight horizontal centerline is the identity deformation.
- **Bone corrective** adds an offset to a pin based on a chosen bone’s local angle relative to its parent. The rest angle, signed range and corrective X/Y offsets remain editable. Keyed pin motion and the corrective add together.

To add a corrective through the current UI, select a character in Animate/Rig first, switch to Artwork, then open a mesh pin’s **Bone corrective** control. Advanced callers can set `MeshPin.driver` directly with `PutRasterLayer`. A driver contains the character ID, joint index, rest angle, signed angular range and normalized target offset.

These tools share evaluation with CPU/GPU exports and layout review. They are **not ARAP, an advanced curve cage, arbitrary vector corrective actions or certified Moho Smart Bone equivalence**. Very strong pin offsets or self-intersecting curves can fold a mesh; inspect the moving result.

## Audio and tracking

**Import audio** accepts bounded WAV, AIFF, MP3 and M4A files. Native decoding normalizes imported clips to editable PCM WAV outside the UI thread. The complete clip length is placed on the timeline, which extends when necessary. Source files remain separate. Native decoding is bounded to mono/stereo, ten minutes and a decoded-memory budget; unsupported codec/container variants report errors.

Pitch/speed is a playback-rate change from 0.25× to 4×, so it changes pitch and the amount of source consumed. Timeline length remains a separate editable value. Waveforms and generated mouth-cue energy windows follow this rate. Mouth cues remain approximate envelope/transcript timing, not a phoneme recognizer; regenerate them after changing audio timing.

In Artwork’s **Motion tracking** section, choose Camera or a graphic/image target. Select a distinctive source-pixel point and preview the range. For an object target, optionally enable a second point to propose rotation and scale. The proposal preserves adjacent frames and existing object animation outside the range. Bone-bound graphics must be detached before world-space tracking.

MCP `artwork_tools` supports:

```json
{
  "operation": "track",
  "project": "PROJECT_UUID",
  "expected_revision": 12,
  "layer": "SOURCE_IMAGE_LAYER_UUID",
  "target": "TARGET_GRAPHIC_OR_IMAGE_UUID",
  "start": 0,
  "end": 59,
  "point": {"x": 120, "y": 90},
  "second_point": {"x": 170, "y": 90},
  "apply": false
}
```

Omit `target` and `second_point` for camera tracking. The local template tracker is limited to 120 frames and can fail under occlusion, blur, rotation or repetitive texture. It does not infer semantic objects. Review before acceptance.

For audio import through MCP, use `artwork_tools` with `operation: "audio"`, project/revision, a workspace-relative `path`, and a start frame. This creates a full-length normalized clip. Asset registration alone does not place media on stage.

## Legacy migration

The read-only compatibility helper first decodes the original document, then uses the established source evaluator at the project’s FPS. It evaluates supported source tweens, IK/joined geometry, custom Stick Nodes shape expansions and movieclip-expanded geometry before creating native data.

The native result includes sampled vector cels, pose controls for stable top-level rigs, skin bindings that follow later pose edits, supported embedded image quads, camera translation/zoom keys and embedded audio tracks. Playback after migration does not need the helper. Node is the transitional import dependency for modern sectioned containers (including zstd) and Stick Nodes; older Pivot readers still use Python. Playback after import uses the native Rust engine. Modern containers do not require a separate Python zstandard installation.

Complete original bytes and canonical decoded metadata live in an immutable archive shared across undo revisions. `inspect_project` omits archive/PNG payloads; its conversion summary includes coverage warnings. Original-file export reads the archived original. It does not pretend new Astral edits can round-trip into every source format.

Important limits remain:

- Sampling preserves evaluated integer-frame poses, not identical editable source tween solvers, subframe curves or live join/movieclip hierarchies.
- Camera rotation is baked into sampled points; translation/zoom remain camera keys.
- Source text outlines, gradients/pattern/filter stacks, unsupported media and version-2 tiled drawings are not lossless native equivalents. External assets are not downloaded implicitly.
- Native conversion is bounded to 12,000 frames/two million vector points and native bitmap budgets. Unavailable or unrepresentable evaluated conversion falls back to the authored-pose converter with explicit warnings; it is not reported as complete migration.
- Original files are never implicitly overwritten. Inspect the converted motion before editing further.

## Workspace customization

**Settings → Appearance → Keyboard shortcuts** edits 18 actions. Choose Change, then press a shortcut; Escape cancels. Conflicting bindings are rejected, and Reset restores defaults. Header/tool hints use the actual bindings.

English, Spanish and French core-control catalogs are available under Appearance. Untranslated advanced controls, dynamic diagnostics and other languages remain unfinished; project dialogue/names are never translated. RTL/localization equivalence is not claimed.

Advanced features use **project schema 9**. Older versions reject these files rather than silently dropping fields. Existing projects remain readable; legacy inline source archives are upgraded in memory when opened in the command service. Saving is explicit.
