# Renderer measurements — 1 November 2026

Three runs per renderer, alternating order, using the four included original projects. Output: 1920 × 1080, 30 FPS, matching Standard/libx264 settings, no hardware encoder. Ryzen 7 7735HS (8C/16T), 16 GB RAM, RTX 4050 laptop/Vulkan; Windows 11. No concurrent Cargo builds ran during this measurement. CPU uses up to four workers; GPU includes initialization and readback.

These are median export-job wall times, not a universal speedup claim. CPU/GPU edge coverage can differ; this is matching delivery/encoder settings, not a pixel-perfect quality certification.

| Project | CPU | GPU | CPU ÷ GPU | Reused output frames: CPU / GPU |
| --- | --- | --- | --- | --- |
| contact | 2.05 s | 3.17 s | 0.65× | 50 / 53 |
| soft | 1.97 s | 2.82 s | 0.70× | 0 / 0 |
| voice | 2.40 s | 3.16 s | 0.76× | 109 / 176 |
| raster-stress | 10.55 s | 5.84 s | 1.81× | 0 / 0 |

The GPU helped the continuous, full-frame raster stress scene. CPU was faster for these smaller studies, which supports keeping light scenes on CPU by default. Auto uses a conservative coverage/duration heuristic, not a learned hardware-performance model. The planned broad 2× median speedup has **not been established**, and no matched legacy performance baseline is claimed.

Exact frame reuse retains every encoded frame and audio cue. CPU workers have independent strided caches, so their hit count can differ from the serial GPU cache. Full-scene movement invalidates reuse; the raster stress corpus moves throughout all ten seconds. Reuse lowers raster work but cannot eliminate encoder cost.

`render_seconds` sums CPU workers and can exceed elapsed time; GPU render time is serial elapsed. `gpu_readback_seconds` includes waiting/synchronization and copying, not an isolated GPU-profiler counter. `encoder_transfer_seconds` includes pipe backpressure. Compare `total_seconds` for complete job timing. CLI startup is separately recorded as `wall_seconds` in the [raw results](ASTRAL-RENDER-BENCHMARK.json).

Reproduce with a release build, separately installed FFmpeg and the checked-in projects:

```powershell
cargo build -p astral --release --locked
$env:ASTRAL_FFMPEG = 'C:\Tools\ffmpeg\bin\ffmpeg.exe'
python scripts/benchmark-astral-renderers.py --repeats 3
```

Project SHA256 values, binary hash, adapter, individual timings and source commit are included in the raw report. Integrated AMD, Mac and mobile rendering performance were not measured. Different resolutions, assets, effects, encoder settings, drivers and laptop power/thermal conditions can change the outcome.
