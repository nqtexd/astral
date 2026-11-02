# Astral 0.15.0 · Windows candidate

This release adds native editable PSD composition, offline recognized visemes/custom mouths, procedural textured brushes, contour Boolean operations, compositing nodes, built-in AVI/image/audio exports, streamed export mixing, retained save versions and MCP transaction previews.

Read [the workflow](ASTRAL-0.15-WORKFLOW.md) for exact controls, API fields and bounds. Use the Studio and MCP executable from the same package. Existing running processes do not update when a new ZIP is extracted.

## Verified software checks

- Strict workspace Clippy passed with warnings denied.
- Four Python release-gate tests passed, checking binary/report hashes, version binding and refusal to certify short/incomplete stress runs.
- Rust workspace regression suite: 185 passing cases, including recovery, backups, compositing/brushes and worker-panic handling.
- Full JavaScript suite: 3,779 passed, 25 skipped, including hostile PSD rectangle and composite-only cases.
- Real MCP checks passed transaction previews, path confinement, corrupt-image rollback, PSD graph/text import, cycle rollback, save backups, real Rhubarb recognition, custom mouth artwork and independent textured frames.
- The full agent/feedback workflows and all 16 previous export cases passed. A separate native AVI check decoded 24 video frames and synchronized stereo PCM for a one-second range while the application encoder path was deliberately unavailable.
- A 180-second headless stress smoke completed 1,283 edits with no failure; measured private-memory growth was about 1.5 MiB. It does **not** satisfy an eight-hour or desktop/physical-input gate.

Final package/build evidence is added before publication. Local paid voice services were not billed or certified. Headless command timing is not physical drawing latency.

## Production scorecard

The [machine-readable gate inventory](../platforms/release/production-gates.json) fails closed without hash-bound reports. Production certification remains **false**.

| Area | Candidate evidence | Still required |
| --- | --- | --- |
| Stability/data safety | Atomic-save/recovery/backup regressions, failed-job handling, corruption rejection | Full release workflow crash corpus and broader fault injection |
| Performance | Bounded caches/queues, streamed output audio, short headless stress | Physical stroke latency, representative desktop playback and eight-hour engine/desktop runs |
| Artist tools | Pressure-sample brushes, shortcuts, frames, vector/compositing and mouth tools | Physical pen/palm/device tests and complete professional workflow parity |
| Interoperability | Native/export regression cases and original archives | Full PSD/legacy semantics and comprehensive color/long-form equivalence |
| MCP | Revisioned atomic batches, previews, rollback, confinement and cancellation checks | Continuing security/regression review as mutation families grow |
| Release engineering | Locked build, checksum packages, installer/signing scripts and diagnostics | Valid signatures, two-clean-build comparison and clean VM install/update/uninstall |

macOS production signing/notarization and physical Apple/Android validation remain unfinished. No Apple signing account or physical tablet lab is present here. Complete localization/native-language review, independent Bezier handle/path-morph tools, arbitrary image-tip brushes, full source hierarchy/filter/tween fidelity, advanced Photoshop operations and long-form streamed preview/NLE editing remain gaps. Compositing currently works in premultiplied sRGB rather than an ICC/HDR grading pipeline.

The Windows setup is an **unsigned candidate**, not a signed production installer. Portable and setup payloads include third-party notices and AI provenance. GPL-3.0-or-later source remains available with the release.
