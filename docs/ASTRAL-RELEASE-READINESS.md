# Release scope and platform evidence

Astral 0.13.0 is a Windows early-access release. It is **not feature-equivalent to Moho Debut or approved for app-store publication**. Its project-management, customization, composition, preview and finishing changes have local test evidence. The 0.13 Android ARM64/x86-64 APK compiles and has a development test signature. The emulator interaction checks below remain historical evidence; no physical Android device was connected for this revision. The macOS ARM development build and real headless MCP acceptance passed on a GitHub Mac runner at `fe7efde` ([run](https://github.com/nqtexd/astral/actions/runs/41322916660)); that is compile/headless evidence, not physical-device or production-signing evidence.

## Reference scope

The initial requirements come from the official [Debut feature page](https://moho.lostmarble.com/products/moho-debut-14) and the [14.5 announcement dated October 12, 2026](https://moho.lostmarble.com/blogs/news/moho-14-5-showtime-design-rig-and-animate-characters-live-all-the-power-of-moho-now-live). Debut lists limited rig correctives, PSD layers, mesh/curve deformation, audio/lip sync, path following, tracking, vector tools and content. The 14.5 note additionally distinguishes Debut's ability to open/stream live models from Pro's model-authoring capability. Pro-only marketing must not be mistaken for Debut requirements.

The machine-readable inventory is [readiness.json](../platforms/release/readiness.json). It is a **minimum confirmed inventory**, not a claim that the entire comparison chart has been audited. Basic bitmap/8-bit RGB PSD, mesh pins, desktop recording, approximate mouth cues and local template tracking are implemented with limits. Version 0.14 adds pressure-sample brushes, MLS deformation, curvers, additive bone-driven pin correctives, object tracking, remappable shortcuts and English/Spanish/French core controls. These implementations have bounded scope: advanced deformation/vector correctives, phoneme-level lip sync, occlusion-aware tracking, live models and complete localization still need work. Physical stylus validation is unfinished. See the [0.14 workflow and migration limits](ASTRAL-0.14-WORKFLOW.md). [Exact 0.13 workflows and limits](ASTRAL-ARTWORK.md). Existing Astral tools—MCP, example-based motion learning, storyboard, contact/prop review, native cels and the seeded director—are retained.

These are feature-equivalence goals, not permission to copy Moho code, branding, proprietary assets or its content library. Astral needs its own implementations and licensed content.

## Android and touch preview

`astral-mobile` is a native Rust/egui/wgpu touch client using the same document engine, revisioned commands, renderer and `.astral` files. It has single-frame drawing, erase, rig posing, timeline/playback, onion previews, project save/open, recovery and PNG output. Two-finger gestures pan/zoom the view. The Android host adds the system document picker and image share sheet. It does not request broad storage, microphone, camera or network permissions.

This is a **preview subset**. Mobile audio/video export, desktop editing parity, mature stylus handling, accessibility, device performance and interruption tests are still release work. Compiling an ARM library or running an emulator does not prove physical phone/tablet readiness.

Build locally with Android SDK API 36, build-tools 36.0.0, NDK 28.2.13676358, a JDK and the Android Rust targets:

```powershell
rustup target add aarch64-linux-android x86_64-linux-android
python scripts/build-astral-android.py
```

The APK has a local test signature and a separate preview application identifier. Do not publish it as a production release or reuse its test key for production.

The API 36 x86_64 emulator passed live drawing, independent adjacent frames, explicit blanks, saving, restart recovery, adding a rig and PNG export. Responsive layout tests cover 320/390-point phones, landscape and a 1024-point tablet; icon bounds are checked against their clipping rectangles. No physical device has been tested. The preview uses GLES through wgpu after the emulator's Vulkan surface stalled. This is compatibility evidence, not a performance benchmark.

Touch imports have a 32 MiB transfer limit and a 64 MiB decoded project limit, with budgets of 32 rigs, 2,048 graphics and 12,000 pose keys. Image/video assets are rejected rather than silently omitted. Desktop remains necessary for larger scenes and unsupported media.

`python scripts/package-astral-source.py` packages the native sources and license with a file checksum inventory, excluding build caches, signing keys and user productions. Dependency notices are included in the APK. The broader publication rights/source audit remains incomplete.

## macOS and Apple mobile

Desktop MCP now has a Unix-domain transport in addition to Windows named pipes. It uses a private per-user socket directory, verifies peer credentials and bounds messages. The desktop and touch UI remain native Rust.

`.github/workflows/astral-platforms.yml` is a manually started development workflow for native Intel/Apple-silicon Mac checks and unsigned iOS simulator builds. It has not been executed here. Current runner labels were checked against [GitHub's runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).

On a Mac with Xcode and XcodeGen:

```sh
python scripts/build-astral-apple.py
```

This builds a Rust XCFramework and an unsigned universal-family iPhone/iPad simulator application. It does not produce a signed device IPA. iOS file workflows, required-reason privacy API declarations, rendering behavior, touch/keyboard/pencil input, lifecycle and device tests require real Apple validation. The privacy file is an initial development inventory and must not be used as a completed App Store privacy audit.

`package-astral-macos.py` produces an ad-hoc-signed development app; notarization and Developer ID signing are separate uncompleted gates. [Apple's membership documentation](https://developer.apple.com/programs/whats-included/) describes Xcode, distribution certificates and store membership.

## Licensing and publication

The repository retains GPL-3.0-or-later. Copyright provenance, bundled dependency/encoder source obligations and the intended Apple distribution terms must be reviewed before publication. The FSF's [historical App Store enforcement explanation](https://www.fsf.org/blogs/licensing/more-about-the-app-store-gpl-enforcement) documents conflicts with additional restrictions; it is not a substitute for checking current terms and all contributors' permissions. Do not silently relicense this derivative project or promise exclusive proprietary rights.

## Final release gate

```sh
python scripts/check-astral-release.py
python scripts/check-astral-release.py --final
python scripts/astral-release-gate.test.py
```

The final check deliberately fails today. It requires a completed feature inventory, implementation evidence for every requirement, version-matched physical-device test reports and artifact hashes, production mobile signing, and publication/license/privacy gates. A package version number or successful compile cannot bypass it. Production Apple accounts/signing and physical hardware evidence are unavailable. Remote Mac development builds do not satisfy those gates.
