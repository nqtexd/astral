# Astral 0.15.0 · Windows candidate

This release adds native editable PSD composition, offline recognized visemes/custom mouths, procedural textured brushes, contour Boolean operations, compositing nodes, built-in AVI/image/audio exports, streamed export mixing, retained save versions and MCP transaction previews.

Read [the workflow](ASTRAL-0.15-WORKFLOW.md) for exact controls, API fields and bounds. Use the Studio and MCP executable from the same package. Existing running processes do not update when a new ZIP is extracted.

## Verified software checks

- Strict workspace Clippy passed with warnings denied.
- Four Python release-gate tests passed, checking binary/report hashes, version binding and refusal to certify short/incomplete stress runs.
- Rust workspace regression suite: 199 passing cases, including recovery, backups, compositing/brushes and worker-panic handling.
- Latest full JavaScript run: 3,777 passed, 25 skipped, two timing-sensitive failures under concurrent compilation. Both affected files passed all 13 tests on isolated rerun. The preceding baseline passed 3,779 tests; the latest full run is not represented as wholly green. PSD rectangle/composite-only regressions passed.
- Real MCP checks passed transaction previews, path confinement, corrupt-image rollback, PSD graph/text import, cycle rollback, save backups, real Rhubarb recognition, custom mouth artwork and independent textured frames.
- The full agent/feedback workflows and all 16 previous export cases passed. A separate native AVI check decoded 24 video frames and synchronized stereo PCM for a one-second range while the application encoder path was deliberately unavailable.
- A 180-second headless stress smoke completed 1,283 edits with no failure; measured private-memory growth was about 1.5 MiB. It does **not** satisfy an eight-hour or desktop/physical-input gate.

The editor/media MCP acceptance passed six additional checks: Hermite channel evaluation, duplicate-channel rollback, stable-ID relinking, confinement, packaged channel reopen and corrupt-package refusal. Final optimized package evidence is recorded before publication. The application control/UI/schema crate uses optimization level 2; animation, rendering and audio dependencies retain level 3 to avoid spending excessive build time optimizing control code. Local paid voice services were not billed or certified. Headless command timing is not physical drawing latency.

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

macOS production signing/notarization and physical Apple/Android validation remain unfinished. No Apple signing account or physical tablet lab is present here. Complete localization/native-language review, independent vector Bezier/path-morph tools, arbitrary image-tip brushes, full source hierarchy/filter/tween fidelity, advanced Photoshop operations and long-form streamed preview/NLE editing remain gaps. RGB image/PSD pixels now normalize supported embedded ICC profiles into sRGB, and PNG/JPEG outputs are tagged. Compositing remains premultiplied sRGB; monitor-profile discovery and HDR grading are unfinished.

The Windows setup is an **unsigned candidate**, not a signed production installer. Portable and setup payloads include third-party notices and AI provenance. GPL-3.0-or-later source remains available with the release.
