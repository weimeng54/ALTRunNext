# Copyright (c) 2026 GPT-6.1 Sol
# Licensed under the MIT License. See LICENSE file in the project root for full license information.

"""Render app.svg into a multi-size Windows icon."""
import argparse
from pathlib import Path
from struct import pack

from resvg_py import svg_to_bytes

SIZES = (16, 18, 20, 24, 32, 40, 48, 64, 96, 128, 256)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--png", type=Path, help="also export the 256×256 PNG to this path")
    args = parser.parse_args()
    assets = Path(__file__).resolve().parents[1] / "assets"
    svg = (assets / "app.svg").read_text(encoding="utf-8")
    frames = [
        svg_to_bytes(svg_string=svg, width=size, height=size, skip_system_fonts=True)
        for size in SIZES
    ]
    directory = bytearray(pack("<HHH", 0, 1, len(SIZES)))
    images = bytearray()
    offset = 6 + 16 * len(SIZES)
    for size, data in zip(SIZES, frames, strict=True):
        dim = size % 256
        directory.extend(pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(data), offset))
        images.extend(data)
        offset += len(data)
    (assets / "app.ico").write_bytes(directory + images)
    if args.png is not None:
        args.png.write_bytes(frames[-1])


if __name__ == "__main__":
    main()
