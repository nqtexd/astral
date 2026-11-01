"""Rasterize Astral's geometric mark for native icons (requires Pillow)."""
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "brand"

def icon(size):
    image = Image.new("RGBA", (size * 4, size * 4))
    draw = ImageDraw.Draw(image)
    scale = size * 4 / 128
    point = lambda p: tuple(round(v * scale) for v in p)
    draw.rounded_rectangle([point((8,8)),point((120,120))],radius=round(24*scale),fill="#20262e")
    draw.line([point((35,94)),point((64,30)),point((93,94))],fill="#edf0f4",width=round(8*scale),joint="curve")
    draw.line([point((46,70)),point((82,70))],fill="#edf0f4",width=round(8*scale))
    return image.resize((size,size), Image.Resampling.LANCZOS)

if __name__ == "__main__":
    OUT.mkdir(parents=True,exist_ok=True)
    icon(512).save(OUT / "astral.png")
    icon(256).save(OUT / "astral.ico",sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)])
    print("Created assets/brand/astral.png and astral.ico")
