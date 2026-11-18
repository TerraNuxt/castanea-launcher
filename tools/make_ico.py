#!/usr/bin/env python3
"""Makes the Windows icon from the launcher's PNG: tools/make_ico.py IN.png OUT.ico

Plain Python, no imaging library: the PNG (8 bit, RGB or RGBA, not interlaced) is decoded, scaled down by area
averaging to 16..256 px, and each size is stored as a PNG inside the .ico (Windows Vista and later read that).
The output is deterministic. The result is committed (packaging/windows/castanea-launcher.ico); run this only when
the picture changes.
"""
import struct
import sys
import zlib

SIZES = [16, 24, 32, 48, 64, 128, 256]


def read_png(path):
    data = open(path, "rb").read()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", "not a PNG"
    pos, idat, header = 8, b"", None
    while pos < len(data):
        length, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            header = struct.unpack(">IIBBBBB", body)
        elif kind == b"IDAT":
            idat += body
    width, height, depth, color, _, _, interlace = header
    assert depth == 8 and color in (2, 6) and interlace == 0, "need an 8 bit RGB or RGBA PNG without interlace"
    bpp = 3 if color == 2 else 4
    raw = zlib.decompress(idat)
    stride = width * bpp
    rows, prev = [], bytearray(stride)
    for y in range(height):
        kind = raw[y * (stride + 1)]
        line = bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        for i in range(stride):
            a = line[i - bpp] if i >= bpp else 0
            b = prev[i]
            c = prev[i - bpp] if i >= bpp else 0
            if kind == 1:
                line[i] = (line[i] + a) & 255
            elif kind == 2:
                line[i] = (line[i] + b) & 255
            elif kind == 3:
                line[i] = (line[i] + (a + b) // 2) & 255
            elif kind == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pred = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
                line[i] = (line[i] + pred) & 255
        rows.append(line)
        prev = line
    pixels = []
    for line in rows:
        row = []
        for x in range(width):
            px = line[x * bpp:(x + 1) * bpp]
            row.append((px[0], px[1], px[2], px[3] if bpp == 4 else 255))
        pixels.append(row)
    return width, height, pixels


def scale(width, height, pixels, size):
    out = []
    for y in range(size):
        y0, y1 = y * height / size, (y + 1) * height / size
        row = []
        for x in range(size):
            x0, x1 = x * width / size, (x + 1) * width / size
            acc, weight_sum = [0.0] * 4, 0.0
            for sy in range(int(y0), min(height, int(y1) + 1)):
                wy = min(y1, sy + 1) - max(y0, sy)
                for sx in range(int(x0), min(width, int(x1) + 1)):
                    w = wy * (min(x1, sx + 1) - max(x0, sx))
                    if w <= 0:
                        continue
                    r, g, b, a = pixels[sy][sx]
                    acc[0] += r * a * w
                    acc[1] += g * a * w
                    acc[2] += b * a * w
                    acc[3] += a * w
                    weight_sum += w
            alpha = acc[3] / weight_sum
            if acc[3] == 0:
                row.append((0, 0, 0, 0))
            else:
                row.append((round(acc[0] / acc[3]), round(acc[1] / acc[3]), round(acc[2] / acc[3]), round(alpha)))
        out.append(row)
    return out


def png(size, pixels):
    raw = b"".join(b"\x00" + bytes(v for px in row for v in px) for row in pixels)

    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))

    header = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    width, height, pixels = read_png(sys.argv[1])
    images = [png(s, scale(width, height, pixels, s)) for s in SIZES]
    head = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries, body = b"", b""
    for size, image in zip(SIZES, images):
        entries += struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(image), offset + len(body))
        body += image
    open(sys.argv[2], "wb").write(head + entries + body)


main()
