"""Build the Windows icon from the approved blue terminal mark."""
from io import BytesIO
from pathlib import Path
from struct import pack

from PIL import Image, ImageDraw

SIZES = (16, 20, 24, 32, 40, 48, 64, 96, 128, 256)


def icon(size):
    canvas = size * 8
    image = Image.new("RGBA", (canvas, canvas))
    draw = ImageDraw.Draw(image)

    def coord(value):
        return round(value * canvas / 24)

    draw.rounded_rectangle(
        (0, 0, canvas - 1, canvas - 1), radius=coord(6), fill="#245C99"
    )
    for points in (
        [(6.5, 7.5), (11.1, 12), (6.5, 16.5)],
        [(13.3, 16.5), (18, 16.5)],
    ):
        draw.line(
            [(coord(x), coord(y)) for x, y in points],
            fill="#FFFFFF",
            width=coord(1.7),
            joint="curve",
        )
    result = image.resize((size, size), Image.Resampling.LANCZOS)
    output = BytesIO()
    result.save(output, format="PNG")
    return output.getvalue()


def main():
    directory = bytearray(pack("<HHH", 0, 1, len(SIZES)))
    images = bytearray()
    offset = 6 + 16 * len(SIZES)
    for size in SIZES:
        data = icon(size)
        dim = size % 256
        directory.extend(pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(data), offset))
        images.extend(data)
        offset += len(data)
    (Path(__file__).resolve().parents[1] / "assets" / "app.ico").write_bytes(directory + images)


if __name__ == "__main__":
    main()
