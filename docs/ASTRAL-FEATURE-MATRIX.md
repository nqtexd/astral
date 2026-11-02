# Requested artist workflows · Astral 0.15

This matrix describes implemented controls and tested software behavior. It does not certify production reliability, animation quality or device performance.

| Workflow | Available now | Location and remaining limits |
| --- | --- | --- |
| Autosave and recovery | Background snapshots for open documents, five previous saves, newer-autosave recovery, atomic writes | Settings sets recovery interval. Recovered work stays unsaved; originals are retained. Broader crash/fault testing remains open. |
| Timeline graph editor | Independent root/joint X/Y channels, draggable keys/tangents, numeric slopes, cubic/linear/hold, automatic tangents, transactional undo | Motion studio → Curves. Independent channels must be retimed explicitly; raster/effect scalar channels are not part of this editor. |
| Automatic phoneme lip-sync | Bundled offline English and phonetic recognition, editable visemes and custom mouth artwork | Voice/mouth tools. Recognition is a starting point; review language, consonants, acting and timing. No paid service is required. |
| Brush engine | Pressure samples, procedural textured dabs, spacing/scatter/flow/hardness/grain, custom presets and optional stabilization | Draw → Brush studio. Physical pen pressure/palm testing, image-tip and ABR compatibility remain open. |
| Masks and clipping | Alpha/luminance/inverted masks; clipping preserves base alpha, including soft edges | Artwork → Compositing. Native graph operations are supported; full PSD clipping-chain equivalence is not claimed. |
| Color management | Supported embedded RGB ICC → sRGB conversion, untransformed mask coverage, sRGB-tagged PNG/JPEG/PNG sequences | SDR sRGB pipeline. Monitor-profile discovery, calibration, CMYK/gray ICC and HDR grading are unfinished. |
| Audio waveform editing | Waveform seek, trim/slip drafts, gain/fades/speed/loop, undoable split with continuous source offset | Motion studio → Finish. Cuts inside fades are refused. Stop/restart playback after relinking; a complete multitrack NLE remains unfinished. |
| X-sheet | Virtualized drawing-key/hold/blank rows, range exposure and blanking, dialogue/audio/marker starts | Project menu → Exposure sheet. Other animation entities are notes rather than full editable sheet columns. |
| Node compositing | Validated editable DAGs for scene sources, blends, masks/clips, gradients, blur and color | Artwork → Compositing. Uses node inspectors/connection selectors rather than a freeform node canvas. Bounded CPU compositing is shared by viewport/export. |
| Symbols/nesting | Live shared-library sources, independent keyed transforms, looping/speed/offset, eight nested levels | Motion studio → Library. Shared source replacement uses `set_library`; nested sources include figures/drawing cels/effects, excluding source audio and raster-layer tracks. |
| Relink/packages | Missing-file status, stable-ID relinking, cancellable `.astralpack` snapshots, manifest hashes and safe extraction to new folders | Project menu → Media and packages; MCP `asset_tools` in full/animation profiles. Embedded image pixels need re-import to change their content. |
| Signed installer/updater | Installer/signing pipeline and a hash/Authenticode/publisher-pinned update verification script | **Not signed here:** no owned certificate is available. Current installer remains an unsigned candidate. Updater is explicit PowerShell, refuses unsigned downloads, and is not an automatic GUI updater. VM upgrade/uninstall tests remain open. |

Project schema 11 adds curves, live instances and clipping. Preserve originals when moving between versions. See [exact workflow and bounds](ASTRAL-0.15-WORKFLOW.md), [candidate verification](ASTRAL-0.15-RELEASE.md) and [production gates](../platforms/release/production-gates.json).
