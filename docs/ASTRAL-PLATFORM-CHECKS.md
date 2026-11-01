# Platform development evidence

These are development builds, not store-ready releases or full desktop feature parity on phones/tablets. Apple artifacts are available from the linked GitHub Actions runs while retained. They have their build source commit; they are not represented as binaries built from a later release tag.

| Target | Evidence | Limits |
| --- | --- | --- |
| Windows 11 x64 | Local 0.13 release build, 159 workspace tests, real stdio MCP acceptance, live desktop UI inspection and physical RTX 4050 rendering. | Unsigned portable release; see the renderer benchmark and artwork limits. |
| macOS ARM64 | [Mac runner job](https://github.com/nqtexd/astral/actions/runs/41322916660/job/124896750675), source `fe7efde`: shared/touch tests, desktop IPC tests, optimized desktop build, real headless MCP acceptance and ad-hoc application packaging passed. | No physical Mac interaction, Developer ID or notarization validation. |
| macOS Intel x86-64 | [Intel Mac runner](https://github.com/nqtexd/astral/actions/runs/41324947412), source `7a8ca01`: shared/touch tests, IPC tests, optimized desktop build, real headless MCP acceptance and development packaging passed. | Ad-hoc development package; no physical interaction, Developer ID or notarization validation. |
| iPhone/iPad simulator | [Unsigned simulator build](https://github.com/nqtexd/astral/actions/runs/41324951325), source `7a8ca01`: device/simulator Rust libraries, XCFramework and native simulator app compiled successfully. | Compilation only; no physical iPhone/iPad checks, production signing, store account or full desktop editing parity. |
| Android ARM64/x86-64 | Local SDK/NDK preview build; see the release's Android build report when present. Earlier emulator drawing/save/recovery checks remain historical evidence. | No physical Android device was connected for this revision. APK uses a development test signature. |

The first Intel Mac check stopped during encoder staging because that wheel's FFmpeg build was non-redistributable. The retry uses a separately installed Homebrew encoder for testing; the packaged app excludes FFmpeg. The first simulator link attempted an x86-64 architecture against an ARM64 simulator library; the build now explicitly selects the available simulator architecture. These failures were build-workflow issues and are retained in the Actions history.

The [release inventory](../platforms/release/readiness.json) intentionally remains incomplete. Development artifacts do not satisfy physical-device, signing, provenance or full feature-equivalence gates.
