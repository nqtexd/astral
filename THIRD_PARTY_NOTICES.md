# License and component notices

## Astral and upstream code

GPL-3.0-or-later; the complete license is in [COPYING](COPYING). Original StykAnimate attribution is retained. [Credits](CREDITS.md) identify the upstream repository. Source for this Astral release is the v0.13.0 tag and its attached source archive.

## Rust dependencies

Cargo.lock records exact versions. The Windows ZIP includes THIRD_PARTY/dependencies.json and license files collected from dependency packages. egui/eframe and wgpu are dual MIT/Apache-2.0; other packages retain the declarations and notices shipped with them. These notices do not relicense those components.

The raster importer uses image (MIT/Apache-2.0) and psd (MIT/Apache-2.0); microphone capture uses cpal (Apache-2.0), and WAV writing uses hound (Apache-2.0). Exact license declarations and texts are collected with the release dependencies. The PNG artwork studies and synthetic impact samples in `docs/showcase` are original project examples under the repository license. The voice study uses an installed Windows system voice; no voice engine or voice dataset is redistributed.

## Fonts and retained web components

- Noto Sans: vendor/text/OFL.txt.
- Inter: assets/brand/Inter-LICENSE.txt and font notices where supplied.
- OpenType helper: vendor/text/opentype.LICENSE.
- resvg: vendor/resvg/LICENSE and vendor/resvg/NOTICE.md.
- zstd helper: vendor/zstd/LICENSE.
- Earcut: vendor/earcut.LICENSE.

Retained web/Electron sources and vendored helpers are compatibility code. Preserve their notices when redistributing them.

## External FFmpeg

The public v0.13.0 Windows package does not include FFmpeg. Obtain it separately through [the official download page](https://ffmpeg.org/download.html) and retain that distribution's licenses. Its license depends on build configuration/components. Developer staging scripts record provenance for local tests; that is not a completed corresponding-source audit for redistributing a third-party binary.

## Scope

No ownership of user-imported media or generated productions is asserted. Users must have permission for media they distribute. Apple store terms, signing, privacy declarations and the full cross-platform publication audit remain outside this Windows early-access release.


### Native voice and audio dependencies (0.14)

The Rust build now includes reqwest (MIT OR Apache-2.0), rustls and related TLS dependencies, plus Symphonia (MPL-2.0) and its enabled native audio decoders. Exact versions, SPDX declarations, source links and available license files are collected from Cargo metadata in each Windows package. See Cargo.lock and the packaged dependency notices. ElevenLabs is an optional user-configured service; no ElevenLabs model weights, voices or API credentials are bundled.
