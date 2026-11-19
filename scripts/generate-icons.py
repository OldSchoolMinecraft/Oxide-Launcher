#!/usr/bin/env python3
"""Regenerate platform icons from the original launcher favicon; requires Pillow."""
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
ICONS = ROOT / "assets/icons"


def main():
    source = Image.open(ICONS / "minecraft-source.png").convert("RGBA")
    if source.size != (32, 32):
        raise ValueError("Expected the original 32x32 Minecraft launcher favicon")
    (ICONS / "minecraft.rgba").write_bytes(source.tobytes())
    def scaled(size):
        return source.resize((size, size), Image.Resampling.NEAREST)
    scaled(256).save(ICONS / "minecraft.png")
    sizes = [16, 24, 32, 48, 64, 128, 256]
    scaled(256).save(ICONS / "minecraft.ico", sizes=[(s, s) for s in sizes],
                     append_images=[scaled(s) for s in sizes])
    scaled(1024).save(ICONS / "minecraft.icns")


if __name__ == "__main__":
    main()
