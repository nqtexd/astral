# Astral 0.14.0 · Windows early access

## Included in this release

- Optional ElevenLabs and compatible speech endpoints in Settings → Voice, alongside Windows offline speech. Provider profiles, explicit external-request opt-in, Windows Credential Manager keys, cancellable jobs and separate editable audio/dialogue clips.
- Pressure-sample vector strokes, marker/dashed/stipple brushes, closed fills, cubic smoothing and point editing with independent frames and undo.
- Rotation/scale-preserving MLS image deformation, animated cubic curvers and additive bone-driven pin correctives, shared by viewport and export evaluation.
- Native WAV/AIFF/MP3/M4A decoding, full-length audio placement, playback-rate controls and waveform analysis. Object tracking can propose translation, rotation and scale.
- Source-evaluated legacy migration: supported sampled geometry, rig controls, image quads, camera moves and audio become native project data. Immutable original archives remain available; large originals are shared across undo snapshots.
- Eighteen remappable shortcuts and English/Spanish/French core controls. Versioned local MCP bridges prevent new tools from silently contacting an older studio.

See [setup and detailed workflows](ASTRAL-0.14-WORKFLOW.md), [MCP guidance](ASTRAL-AGENTS.md) and the [changelog](ASTRAL-CHANGELOG.md). Project schema 9 requires this release or newer; retain imported originals and previous versions.

## Validation

- Optimized Windows release build passed. Packaged executables passed real MCP artwork/migration and full agent-workflow acceptance; all 16 packaged export cases passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: 174 passed, zero failures.
- `node --test tests/*.test.mjs`: 3,775 passed, zero failures, 25 skipped. Includes two source-evaluation/snapshot regression tests.
- Real stdio MCP checks passed for compressed legacy conversion, native skin pose editing, independent painted frames, stale-revision rollback and save/reopen. Project/animation profile discovery and headless file handoff passed.
- Full headless agent acceptance passed: editable ten-second fight, targeted track-preserving revision, independent cels, example-based completion, training cancellation/stale-revision checks, offline speech, waveform analysis and undo/redo.
- Sixteen real export regression cases passed, covering native images, video, sequences, audio and framing.
- [Windows CI passed for the implementation commit](https://github.com/nqtexd/astral/actions/runs/41403505151) (`b632802d1ae5fd62de190aef32afa80c06051248`), including external MCP, artwork and profile checks.
- Feedback acceptance passed project rename/activity/trash/restore, composition proposals, editable dialogue, pose flow, preview overwrite and trimmed-shot assembly with failure preservation.

The optimized Studio launched and displayed the project manager. Further automated Settings navigation was blocked by the Windows automation helper reporting incorrect 14×14 window bounds; no full live UI sign-off is claimed.

Live paid speech calls and physical input/device certification are not part of these automated checks.

## Remaining limits

This is **not full Moho Debut parity or lossless legacy conversion**. Advanced PSD masks/blends/text/filters, arbitrary vector corrective actions, advanced cages/deformation, complete localization and several other comparison-chart items remain unfinished. Legacy source solvers, joins and nested clip semantics are sampled into native data rather than retained as an equivalent live source hierarchy; unsupported geometry, media and effects receive conversion warnings. Review motion and appearance after import.

Tracking uses bounded local texture matching, not semantic recognition or guaranteed occlusion recovery. Lip sync remains approximate. Pressure samples are supported, but physical tablet certification is unfinished. Windows is the locally tested release target; no new signed Apple/device or certified physical Android release is claimed.

Voice adapter tests use a local server. Real provider credentials, billing, model access and service availability remain user-specific. Compatible speech support describes an HTTP contract, not every vendor API.

The [readiness inventory](../platforms/release/readiness.json) retains the unfinished work. Licensing remains GPL-3.0-or-later; dependency notices and AI provenance ship with the package.
