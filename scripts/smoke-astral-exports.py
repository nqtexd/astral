"""Exercise real encoders, framing, sequences and audio through the public CLI.

Requires Pillow for decoded-image assertions. ASTRAL_BINARY can select a package.
"""
import io
import json
import os
from pathlib import Path
import subprocess
import wave
import zipfile

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT / "artifacts" / "astral-exports"
EXE = Path(os.environ.get("ASTRAL_BINARY", ROOT / "target/debug/astral.exe"))
FFMPEG = Path(os.environ.get("ASTRAL_FFMPEG", ROOT / "build/video-encoder/ffmpeg.exe"))
WORK.mkdir(parents=True, exist_ok=True)


def cli(*args):
    result = subprocess.run([str(EXE), "--workspace", str(WORK), *args], capture_output=True, text=True, encoding="utf-8", timeout=180)
    assert result.returncode == 0, result.stderr
    return json.loads(result.stdout)


def decode(path):
    result = subprocess.run([str(FFMPEG), "-v", "error", "-i", str(path), "-frames:v", "1", "-f", "image2pipe", "-vcodec", "png", "pipe:1"], capture_output=True, timeout=60)
    assert result.returncode == 0, result.stderr.decode()
    return Image.open(io.BytesIO(result.stdout)).convert("RGB")


cli("demo", "motion.astral")
catalog = cli("export-presets")
assert len(catalog["presets"]) == 9 and len(catalog["formats"]) == 8
reports = {}


def export(name, *flags):
    report = cli("export", "motion.astral", name, "--start", "104", "--end", "107", *flags)
    assert report["state"] == "completed", report
    assert (WORK / name).stat().st_size > 0
    reports[name] = report
    return report


for preset, dimensions in [("instagram-reels", (1080, 1920)), ("instagram-stories", (1080, 1920)), ("instagram-portrait", (1080, 1350)), ("instagram-square", (1080, 1080)), ("youtube-1080p", (1920, 1080)), ("youtube-4k", (3840, 2160)), ("youtube-shorts", (1080, 1920)), ("tiktok", (1080, 1920))]:
    name = preset + ".mp4"
    report = export(name, "--preset", preset)
    image = decode(WORK / name)
    assert image.size == dimensions, (name, image.size)
    assert report["plan"]["frames"] == 4

fit = decode(WORK / "instagram-reels.mp4")
assert max(fit.getpixel((540, 10))) <= 4, "Fit must letterbox a wide source"
export("fill.mp4", "--preset", "instagram-reels", "--framing", "fill")
fill = decode(WORK / "fill.mp4")
assert max(fill.getpixel((540, 10))) > 8, "Fill must crop rather than letterbox"

for extension in ["webm", "gif", "jpg", "png", "zip", "wav"]:
    export("sample." + extension, "--width", "320", "--height", "568")
for extension in ["webm", "gif", "jpg", "png"]:
    assert decode(WORK / ("sample." + extension)).size == (320, 568)
for name in ["instagram-reels.mp4", "sample.webm"]:
    audio = subprocess.run([str(FFMPEG), "-v", "error", "-i", str(WORK / name), "-map", "0:a:0", "-f", "s16le", "pipe:1"], capture_output=True, timeout=60)
    assert audio.returncode == 0 and any(audio.stdout), "Video must retain the audible cue: " + name
with Image.open(WORK / "sample.gif") as gif:
    assert gif.n_frames == 2
with wave.open(str(WORK / "sample.wav"), "rb") as wav:
    assert wav.getframerate() == 48000 and wav.getnchannels() == 2
    assert wav.getnframes() == 6400
    assert any(wav.readframes(wav.getnframes())), "Sound cue should survive range trimming"
with zipfile.ZipFile(WORK / "sample.zip") as archive:
    assert archive.testzip() is None
    frames = [name for name in archive.namelist() if name.endswith(".png")]
    assert len(frames) == 4
    manifest = json.loads(archive.read("manifest.json"))
    assert manifest["plan"]["fps"] == 30 and manifest["plan"]["first"] == 104
    assert Image.open(io.BytesIO(archive.read(frames[0]))).size == (320, 568)
    assert "audio.wav" in archive.namelist()

report = export("retimed.mp4", "--width", "320", "--height", "180", "--fps", "60")
assert report["plan"]["frames"] == 8
result = subprocess.run([str(FFMPEG), "-v", "error", "-i", str(WORK / "retimed.mp4"), "-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1"], capture_output=True, timeout=60)
assert result.returncode == 0 and len(result.stdout) == 320 * 180 * 4 * 8
(WORK / "report.json").write_text(json.dumps(reports, indent=2), encoding="utf-8")
print(json.dumps({"passed": True, "exports": len(reports), "workspace": str(WORK)}))
