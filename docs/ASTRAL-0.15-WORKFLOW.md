# Astral 0.15 · editable finishing and release gates

This Windows candidate adds working tools behind several previously partial rows. It does **not** certify the complete comparison chart or the production scorecard. The [feature inventory](../platforms/release/readiness.json) and [production gates](../platforms/release/production-gates.json) retain unfinished work.

## PSD layers, masks and text

In **Artwork → Import artwork**, choose an 8-bit RGB PSD. The Windows package includes a pinned parser runtime; a separate Node installation is unnecessary. Import runs outside the UI thread and publishes one revision only after document/image validation.

Supported layer pixels, nested groups, bitmap masks and normal/multiply/screen/overlay/darken/lighten/difference/add blends become an editable native composition. Basic text becomes an editable caption using Astral fonts, including small unboxed labels. Layer names and original PSD assets remain available. ZIP-compressed layer data is decoded by the parser.

Open **Artwork → Compositing** to inspect the graph, choose its output, change blend/opacity/mask settings or add blur, color adjustment and gradient-fill nodes. Disabling/removing a composition restores its input layers to ordinary rendering. Keep the graph enabled when reviewing imported group opacity/masks. A graph can reference raster layers, drawing layers, graphics and dialogue; the initial GUI builder stacks artwork layers, while typed `SetCompositions` commands can choose all input types.

The node editor is an inspector, not a free-form node canvas. Graphs are bounded to 64 nodes each, eight graphs, and 64 megapixels across their document-sized intermediate surfaces. Complex graphs use one CPU export worker to bound memory. Static evaluated inputs reuse cached outputs. CPU node evaluation produces a normal premultiplied texture shared by CPU/GPU delivery and the desktop; this is not a GPU effects engine.

**Conversion limits:** PSB, CMYK and high-bit-depth documents remain unsupported. Photoshop text runs/font metrics/affine transforms, adjustment layers, clipping chains, layer styles, smart objects and vector masks are not equivalent native operations. Pass-through groups become isolated groups. Mask feather/density can differ. Inspect returned warnings and compare the moving appearance; originals are retained. PSD decoding stays isolated in a monitored child process with pre-decompression rectangle/channel budgets, output bounds, cancellation and a deadline. Failed decoding does not fall back to an in-process parser or partially commit the project. Composite-only PSD files remain supported through the same isolated decoder.

## Offline viseme recognition

Select the character in Animate/Rig, then open **Artwork/Audio → Voice & mouth timing**. Choose an audio clip and a timing method:

| Method | Intended use |
| --- | --- |
| English | Rhubarb/PocketSphinx recognition for English recordings; an optional transcript can improve uncommon words. |
| Phonetic | Language-independent sound recognition; review accuracy for your language, microphone and performance. |
| Envelope | Earlier approximate sound-energy/text timing; retained for compatibility, explicitly not speech recognition. |

Press **Generate mouth cues** and wait for the job to complete. Recognized source A/B/C/D/E/F/G/H/X codes map to distinct native mouth states. Keys stay editable. Expand **Mouth artwork** to assign imported bitmaps to mouth states; hide an imported reference layer if you only want its bitmap used as a mouth. Unassigned states use vector placeholders.

The complete Rhubarb runtime/models/notices ship in the Windows package. Speech analysis is offline, cancellable, limited to two minutes per clip and a 180-second worker deadline. Trim, speed, gain, looping and original fade timing are applied before recognition. Review noisy speech, closures and consonants; this is automatic viseme timing, not a guarantee of professional facial acting or phoneme transcription.

MCP example after inspecting current IDs/revision:

```json
{"operation":"lip_sync","project":"PROJECT_UUID","expected_revision":12,"clip":"AUDIO_UUID","character":"CHARACTER_UUID","engine":"phonetic","text":"","mapping":{"H":"l","G":"f"}}
```

Use `artwork_tools`; poll `job_status`. `mapping` overrides source viseme-to-native-state assignments. Use `PutMouthTrack.sprites` for bitmap assignments. Omitting `engine` retains the synchronous envelope compatibility method.

## Brush studio and vector paths

In **Draw → Brush studio**, choose Pencil, Ink, Airbrush or Chalk. Adjust spacing, flow, hardness, grain and scatter, then name/save a custom local preset. Pressure changes stamp width when the input device supplies force. Brush parameters and deterministic grain seeds are stored in editable strokes; output is reproducible. **Use simple brush** returns to the previous solid/marker/dashed/stipple engine.

These are procedural texture brushes, not an arbitrary image-tip/ABR engine or a full commercial brush library. Each stroke has a 50,000-dab budget. Closed fill and bitmap strokes cannot use the stamp engine. Example-based completion retains compatible stored stroke settings; it is not a newly trained brush model. Physical stylus/driver/palm-rejection certification remains open.

In **Motion graphics → Vector paths**, choose two shapes and Union/Subtract/Intersect/XOR. **Create combined shape** adds an editable result and retains both originals for comparison/undo. Rectangle, ellipse, star, ring and explicit contours are supported; rig-bound shapes require unbinding first. Holes preserve opposite winding. Input paths are bounded to 10,000 points.

**Make path editable** converts a supported primitive to contours. Select a contour/point for precise X/Y edits. These edits change the shared shape across its animation, while drawing cels retain independent frame editing. Independent Bezier tangent handles/keyed path morphing remain unfinished. Linear/radial gradients are graph nodes, commonly masked by a source's alpha.

## Built-in exports and long-form audio

| Output | External encoder? | Limits |
| --- | --- | --- |
| AVI | No | Native Motion JPEG and synchronized stereo 48 kHz PCM. AVI 1.0 safety limit 2 GB; black matte for transparency. Large compared with H.264. |
| PNG/JPEG | No | Delivery-size native image rendering; PNG preserves alpha, JPEG uses black matte. |
| PNG-sequence ZIP | No | Native PNG frames, manifest and optional separate `audio.wav`; preserves alpha. |
| WAV | No | Native mixed PCM. |
| MP4/WebM/GIF | FFmpeg | Existing framing/quality/hardware-encoding paths; install/configure FFmpeg separately. |

Export uses a temporary destination and publishes only after success, preserving previous files on failure/cancellation. Output frame count is bounded to one million. Social framing presets also work with native formats, but MP4 remains the intended social upload format.

Audio export now mixes in two-second blocks, preserving absolute source time, speed, looping and original fade phase for selected ranges. Output audio ranges can be up to six hours; simultaneously active decoded sources are bounded to 256 MiB. Individual encoded imports/decoding retain their existing ten-minute limits. In-memory desktop preview still has its earlier ten-minute mix limit; full streamed long-form preview/NLE editing and color-management equivalence are unfinished.

## Data safety, automation and diagnostics

Distinct saves retain five prior files under `.astral-backups/<path-hash>/`. Recovery saves remain a separate rolling copy. Unchanged saves do not create duplicate backups. Recent-project discovery excludes backups/trash. To inspect an older version, copy it to a separate project filename before opening it, keeping associated project-relative assets with it. This is not a complete asset-versioning system.

`preview_commands` validates a complete batch without changing the project or undo history. It returns proposed counts; generated preview IDs are not commit IDs. Apply the reviewed commands with the current `expected_revision`. New bitmap payloads are decoded before committing. Cycles, missing references, stale revisions, invalid images and out-of-workspace paths are rejected.

Background-task panics become finished failed jobs rather than staying queued forever. Local panic reports retain at most ten files of 32 KiB under the local application-data `Astral/diagnostics` directory. Panic payloads are omitted; stack traces may contain machine paths. Nothing is uploaded automatically. Inspect the current revision after a failed job rather than assuming it made no changes.

## Production validation

Run the real workflow checks against the package:

```powershell
$env:ASTRAL_BINARY = (Resolve-Path release/Astral-0.15.0-windows-x64/astral.exe).Path
node scripts/check-astral-production.mjs
python scripts/stress-astral.py --binary $env:ASTRAL_BINARY --hours 8
python scripts/check-astral-readiness.py --binary $env:ASTRAL_BINARY --evidence build/production-evidence.json
```

The stress runner records bounded private-memory samples, mutations, undo/redo, saves, rendering, failures and command latency. A short smoke does not satisfy eight hours. Headless command latency is **not physical pen-to-pixel latency** or a desktop playback measurement. After the packaged workflow/export suites pass, `collect-astral-evidence.py` binds their results and the Rust integrity log to the exact binary hash. The readiness checker fails closed without hash-bound evidence for every gate; `--allow-candidate` prints pending work without declaring production readiness.

`stage-astral-installer.py` pins the official compiler in private portable mode. `platforms/windows/Astral.iss` and `scripts/build-astral-installer.ps1` define user-local install/update/uninstall and optional code signing. Production mode requires an owned certificate. No certificate, clean installer VM, physical tablet lab or Apple signing account is present here; scripts alone do not satisfy those gates. Apple/macOS notarization and device releases remain unfinished. Core English/Spanish/French labels are expanded, but full translation/layout/native-language review remains partial.

## Implementation references

The PSD adapter uses [ag-psd](https://github.com/Agamnentzar/ag-psd). Offline mouth timing follows [Rhubarb’s recognizers and JSON interface](https://github.com/DanielSWolf/rhubarb-lip-sync). Boolean operations use [geo’s documented non-zero fill operations](https://docs.rs/geo/latest/geo/algorithm/bool_ops/trait.BooleanOps.html). Original third-party notices remain in the source/package; these integrations do not imply code or feature equivalence with proprietary editors.


## Curves, exposures and live animation

Open **Motion studio → Curves**. Select a character, create a channel, choose root/joint and X/Y, then add keys at the playhead. Drag keys or incoming/outgoing tangent handles. Numeric controls and hold/linear/cubic interpolation remain available. **Auto tangents** estimates a local slope. **Apply curve** commits the draft as one undoable revision. Only the authored range overrides ordinary poses/actions; bone constraints and contacts still run afterward. Curve values are absolute channel coordinates, not offsets. Retiming pose keys does not automatically retime these independent channels; edit their frames explicitly through `set_curve_tracks`.

**Project menu → Exposure sheet** shows drawing keys, holds/blanks, dialogue starts, sound starts and markers. Click a cell; Shift-click extends the range. Expose the current drawing through a range or blank selected frames. Existing timeline/motion tools provide copying and broader retiming. The sheet virtualizes rows for long projects; it is not a full compositing-layer dope sheet.

**Motion studio → Library → Live symbols and nested animation** places a shared library source rather than copying its contents. Each instance has local offset, speed, loop/hold, transform keys and an optional parent instance. Child timing is measured in the parent's local frames; nesting is limited to eight levels and cycles are rejected. `set_library` replaces a shared definition and updates its instances; `set_symbol_instances` edits instance structure transactionally. Source figures, drawing cels and effects are supported; source audio, raster-layer tracks, independent curve tracks and arbitrary movie files are not currently instanced. A library source in use cannot be removed without changing its instances in the same batch.

## Waveform and color controls

In **Motion studio → Finish**, select an audio clip. Click its waveform to seek. Drag the left edge to trim its head; drag elsewhere to trim its tail; Shift-drag slips source audio. These are draft edits until **Apply audio edit** or the reviewed preview is committed. **Split at playhead** preserves source time, speed, looping, outer fades and gain as one undoable batch. A cut inside a fade is refused to avoid changing the envelope. Waveform caches refresh when the linked source path changes. Stop and restart playback after relinking audio.

RGB artwork with a supported embedded ICC profile is converted to the internal **sRGB SDR** working space. Untagged artwork is assumed sRGB. PSD pixel layers use the document RGB profile; mask coverage stays untransformed. Editable PSD text colors/font appearance still require review. PNG and JPEG exports carry an sRGB ICC profile, including PNG sequences. Compositing is premultiplied sRGB, not linear-light/HDR grading. OS monitor-profile discovery, calibrated display validation, CMYK/gray ICC workflows and full video color-profile equivalence remain unfinished.

## Moving projects and recovering work

**Project menu → Media and packages** shows linked media availability. **Relink** copies a selected replacement into the project asset folder and preserves the asset ID and clip timing. It does not replace already embedded bitmap pixels: re-import changed artwork. Waveforms refresh when the source path changes; stop and restart audio playback after relinking.

**Create verified package** writes a new `.astralpack` snapshot containing the project, linked media and a SHA-256 manifest. Paths are relative to the project folder. **Verify and unpack** uses paths in the configured automation workspace and a new destination folder. No existing folder is overwritten; traversal, links, oversized files and checksum mismatches are rejected. Embedded artwork is already in the project. Limits: 4,096 linked paths, 256 MiB per file and 2 GiB total. Packages do not include credentials, preferences or diagnostic logs.

The MCP `asset_tools` tool exposes `inspect`, `relink`, `package`, and `unpack`; package/unpack return cancellable job IDs. Relink/package require the expected revision. GUI and MCP opens prefer a newer autosave for ordinary project files, retain the original file and mark recovered work unsaved. Explicitly opened backup/recovery files are not automatically replaced. An older autosave request cannot overwrite a higher-revision recovery snapshot.

## Installer updates

`build-astral-installer.ps1 -Production -CertificateThumbprint …` requires an owned valid code-signing certificate. No certificate is present in this environment, so public setup builds remain **unsigned candidates**.

The packaged `scripts/Update-Astral.ps1 -Tag vX.Y.Z -PublisherThumbprint <trusted certificate SHA-1 thumbprint>` downloads and verifies a specific official GitHub installer. It checks the SHA-256 release digest, size, Authenticode validity and the operator-pinned signer. It refuses unsigned releases. Add `-Install` to request installation, or `-WhatIf` to inspect that step; prereleases additionally require `-AllowPrerelease`. Publisher trust must come from a separately verified source, not the downloaded release. This is an explicit PowerShell update workflow, not a background GUI updater. It cannot update the current unsigned candidate through the signed path. Clean-VM installation/upgrade/uninstall and rollback testing remain release gates.
