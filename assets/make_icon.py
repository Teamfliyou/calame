"""Génère assets/icon.png : un calame (roseau taillé) blanc sur fond bleu, avec un trait d'encre.

Aucune dépendance : le PNG est écrit à la main. Usage : python assets/make_icon.py
"""

import math
import struct
import zlib
from pathlib import Path

SIZE = 256
SUPERSAMPLE = 3


def in_polygon(x, y, poly):
    inside = False
    j = len(poly) - 1
    for i in range(len(poly)):
        xi, yi = poly[i]
        xj, yj = poly[j]
        if (yi > y) != (yj > y) and x < (xj - xi) * (y - yi) / (yj - yi) + xi:
            inside = not inside
        j = i
    return inside


def in_rounded_square(x, y, margin=8, radius=52):
    lo, hi = margin, SIZE - margin
    if not (lo <= x <= hi and lo <= y <= hi):
        return False
    cx = min(max(x, lo + radius), hi - radius)
    cy = min(max(y, lo + radius), hi - radius)
    return (x - cx) ** 2 + (y - cy) ** 2 <= radius**2


def pen_polygon():
    tip, end = (66.0, 196.0), (198.0, 64.0)
    length = math.dist(tip, end)
    dx, dy = (end[0] - tip[0]) / length, (end[1] - tip[1]) / length
    nx, ny = -dy, dx
    half, taper = 19.0, 70.0

    def at(along, across):
        return (tip[0] + dx * along + nx * across, tip[1] + dy * along + ny * across)

    return [at(0, 0), at(taper, half), at(length, half), at(length, -half), at(taper, -half)]


def ink_wave():
    """Trait d'encre ondulé sous la plume, épais de 9 pixels."""
    top, bottom = [], []
    for i in range(41):
        x = 36 + i * 3.4
        y = 220 + 7 * math.sin(i * 0.5)
        top.append((x, y - 4.5))
        bottom.append((x, y + 4.5))
    return top + bottom[::-1]


PEN = pen_polygon()
INK = ink_wave()


def sample(x, y):
    """Couleur RGBA d'un point (x, y) de l'image, sans lissage."""
    if not in_rounded_square(x, y):
        return (0, 0, 0, 0)
    t = (y - 8) / (SIZE - 16)
    background = (
        round(36 + (18 - 36) * t),
        round(78 + (150 - 78) * t),
        round(166 + (168 - 166) * t),
        255,
    )
    if in_polygon(x, y, PEN):
        return (255, 255, 255, 255)
    if in_polygon(x, y, INK):
        return (255, 196, 64, 255)
    return background


def main():
    rows = []
    n = SUPERSAMPLE
    for py in range(SIZE):
        row = bytearray([0])  # filtre PNG : aucun
        for px in range(SIZE):
            acc = [0.0, 0.0, 0.0, 0.0]
            for sy in range(n):
                for sx in range(n):
                    r, g, b, a = sample(px + (sx + 0.5) / n, py + (sy + 0.5) / n)
                    # Moyenne en alpha prémultiplié pour de beaux bords.
                    acc[0] += r * a
                    acc[1] += g * a
                    acc[2] += b * a
                    acc[3] += a
            total = acc[3]
            if total == 0:
                row += bytes((0, 0, 0, 0))
            else:
                row += bytes(
                    (
                        round(acc[0] / total),
                        round(acc[1] / total),
                        round(acc[2] / total),
                        round(total / (n * n)),
                    )
                )
        rows.append(bytes(row))

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(b"".join(rows), 9))
        + chunk(b"IEND", b"")
    )
    out = Path(__file__).with_name("icon.png")
    out.write_bytes(png)
    print(f"{out} : {len(png)} octets")


if __name__ == "__main__":
    main()
