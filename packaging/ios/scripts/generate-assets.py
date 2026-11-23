#!/usr/bin/env python3
"""Generate iOS app icon and launch mark from the symbolic.software palette."""

from __future__ import annotations

import math
import struct
import zlib
from pathlib import Path

LIGHT_BG = (0xF4, 0xEE, 0xE2, 255)
INK = (0x1A, 0x1A, 0x1A, 255)
PURPLE = (0x6E, 0x58, 0x83, 255)
GREEN = (0x3D, 0x66, 0x52, 255)
ORANGE = (0xA3, 0x5C, 0x3A, 255)
YELLOW = (0xEC, 0xC3, 0x68, 255)

ROOT = Path(__file__).resolve().parents[1]


def write_png(path: Path, width: int, pixels: list[tuple[int, int, int, int]]) -> None:
	raw = bytearray()
	for y in range(width):
		raw.append(0)
		row = pixels[y * width : (y + 1) * width]
		for r, g, b, a in row:
			raw.extend((r, g, b, a))
	compressed = zlib.compress(bytes(raw), 9)

	def chunk(tag: bytes, data: bytes) -> bytes:
		crc = zlib.crc32(tag + data) & 0xFFFFFFFF
		return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", crc)

	ihdr = struct.pack(">IIBBBBB", width, width, 8, 6, 0, 0, 0)
	png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", compressed) + chunk(b"IEND", b"")
	path.parent.mkdir(parents=True, exist_ok=True)
	path.write_bytes(png)


def lerp(a: tuple[int, int, int, int], b: tuple[int, int, int, int], t: float) -> tuple[int, int, int, int]:
	t = max(0.0, min(1.0, t))
	return tuple(int(round(a[i] + (b[i] - a[i]) * t)) for i in range(4))  # type: ignore[return-value]


def paint_icon(size: int) -> list[tuple[int, int, int, int]]:
	cx = cy = (size - 1) / 2.0
	pixels: list[tuple[int, int, int, int]] = []
	outer = size * 0.36
	inner = size * 0.22
	dot_r = size * 0.055
	dot_orbit = size * 0.30
	colors = (PURPLE, GREEN, ORANGE, YELLOW, PURPLE, GREEN, ORANGE, YELLOW)
	for y in range(size):
		for x in range(size):
			dx = x - cx
			dy = y - cy
			dist = math.hypot(dx, dy)
			px = LIGHT_BG
			# eight petal-ish dots around a purple ring
			for i, color in enumerate(colors):
				ang = i * (math.pi / 4.0) - math.pi / 2.0
				px_d = cx + math.cos(ang) * dot_orbit
				py_d = cy + math.sin(ang) * dot_orbit
				d = math.hypot(x - px_d, y - py_d)
				if d <= dot_r:
					edge = (dot_r - d) / max(1.0, dot_r * 0.25)
					px = lerp(LIGHT_BG, color, edge)
			if inner <= dist <= outer:
				edge = min(dist - inner, outer - dist) / max(1.0, (outer - inner) * 0.18)
				px = lerp(px, PURPLE, edge)
			if dist < inner * 0.35:
				px = lerp(px, INK, 0.9)
			pixels.append(px)
	return pixels


def main() -> None:
	icon_dir = ROOT / "Assets.xcassets" / "AppIcon.appiconset"
	mark_dir = ROOT / "Assets.xcassets" / "LaunchMark.imageset"
	icon_dir.mkdir(parents=True, exist_ok=True)
	mark_dir.mkdir(parents=True, exist_ok=True)

	write_png(icon_dir / "AppIcon.png", 1024, paint_icon(1024))
	write_png(mark_dir / "LaunchMark.png", 256, paint_icon(256))

	(icon_dir / "Contents.json").write_text(
		"""{
  "images" : [
    {
      "filename" : "AppIcon.png",
      "idiom" : "universal",
      "platform" : "ios",
      "size" : "1024x1024"
    }
  ],
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}
""",
		encoding="utf-8",
	)
	(mark_dir / "Contents.json").write_text(
		"""{
  "images" : [
    {
      "filename" : "LaunchMark.png",
      "idiom" : "universal",
      "scale" : "1x"
    },
    {
      "idiom" : "universal",
      "scale" : "2x"
    },
    {
      "idiom" : "universal",
      "scale" : "3x"
    }
  ],
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}
""",
		encoding="utf-8",
	)
	(ROOT / "Assets.xcassets" / "Contents.json").write_text(
		"""{
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}
""",
		encoding="utf-8",
	)
	print(f"wrote icons under {ROOT / 'Assets.xcassets'}")


if __name__ == "__main__":
	main()
