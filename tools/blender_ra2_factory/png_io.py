"""
Dependency-free lossless PNG encoder/decoder & UV-island dilation utilities.

Works identically inside Blender 5.3's bundled Python (`numpy` + stdlib `zlib`/`struct`,
no `Pillow` dependency) and in standard host Python. Crucially preserves un-premultiplied
straight RGBA bytes so `Alpha = 0` team-mask pixels keep exact `(g, g, g, 0)` grayscale!
"""

from __future__ import annotations

import struct
import zlib
from typing import Tuple

import numpy as np

PNG_SIG = b"\x89PNG\r\n\x1a\n"


def _png_chunk(chunk_type: bytes, data: bytes) -> bytes:
    crc = zlib.crc32(chunk_type)
    crc = zlib.crc32(data, crc) & 0xFFFFFFFF
    return struct.pack(">I", len(data)) + chunk_type + data + struct.pack(">I", crc)


def encode_png_bytes(img: np.ndarray, compress_level: int = 6) -> bytes:
    """
    Encodes `(H, W, 3)` uint8 RGB or `(H, W, 4)` uint8 straight-RGBA array into lossless PNG bytes.
    Never premultiplies or alters RGB where `Alpha == 0`.
    """
    arr = np.ascontiguousarray(img, dtype=np.uint8)
    if arr.ndim != 3 or arr.shape[2] not in (3, 4):
        raise ValueError(f"Expected (H, W, 3|4) uint8 array, got {arr.shape}")
    h, w, ch = arr.shape
    color_type = 6 if ch == 4 else 2

    # Prepend filter byte 0 (Sub/None; Sub=1 compresses gradients/maps much smaller & faster!)
    # Using filter 1 (Sub) per scanline for compact `.glb` size while 100% lossless:
    row_bytes = w * ch
    flat = arr.reshape(h, row_bytes)
    filtered = np.empty((h, 1 + row_bytes), dtype=np.uint8)
    filtered[:, 0] = 1  # PNG Sub filter
    filtered[:, 1 : 1 + ch] = flat[:, :ch]
    if row_bytes > ch:
        filtered[:, 1 + ch :] = (flat[:, ch:].astype(np.int16) - flat[:, :-ch].astype(np.int16)) & 0xFF

    ihdr = struct.pack(">IIBBBBB", w, h, 8, color_type, 0, 0, 0)
    idat = zlib.compress(filtered.tobytes(), level=compress_level)
    return PNG_SIG + _png_chunk(b"IHDR", ihdr) + _png_chunk(b"IDAT", idat) + _png_chunk(b"IEND", b"")


def _paeth_predictor(a: np.ndarray, b: np.ndarray, c: np.ndarray) -> np.ndarray:
    p = a + b - c
    pa = np.abs(p - a)
    pb = np.abs(p - b)
    pc = np.abs(p - c)
    cond_a = (pa <= pb) & (pa <= pc)
    cond_b = (~cond_a) & (pb <= pc)
    return np.where(cond_a, a, np.where(cond_b, b, c))


def decode_png_rgba(png_bytes: bytes) -> np.ndarray:
    """
    Decodes an 8-bit RGB, RGBA, Grayscale, Grayscale+Alpha, or Palette PNG into `(H, W, 4)` uint8 RGBA.
    Supports all 5 standard PNG scanline filters (0..4).
    """
    if len(png_bytes) < 24 or png_bytes[:8] != PNG_SIG:
        raise ValueError("Invalid PNG signature")

    pos = 8
    w = h = bit_depth = color_type = 0
    plte = None
    trns = None
    idat_parts = []

    while pos + 8 <= len(png_bytes):
        length = struct.unpack_from(">I", png_bytes, pos)[0]
        ctype = png_bytes[pos + 4 : pos + 8]
        cdata = png_bytes[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if ctype == b"IHDR":
            w, h, bit_depth, color_type, comp, filt, interlace = struct.unpack(">IIBBBBB", cdata[:13])
            if bit_depth != 8 or interlace != 0:
                raise ValueError(f"Unsupported PNG bit_depth={bit_depth} interlace={interlace}")
        elif ctype == b"PLTE":
            plte = np.frombuffer(cdata, dtype=np.uint8).reshape(-1, 3)
        elif ctype == b"tRNS":
            trns = np.frombuffer(cdata, dtype=np.uint8)
        elif ctype == b"IDAT":
            idat_parts.append(cdata)
        elif ctype == b"IEND":
            break

    ch_map = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}
    bpp = ch_map[color_type]
    stride = w * bpp
    raw = zlib.decompress(b"".join(idat_parts))
    rows = np.frombuffer(raw, dtype=np.uint8).reshape(h, 1 + stride)

    recon = np.zeros((h, stride), dtype=np.uint8)
    if np.all(rows[:, 0] == 1):
        # Fast vectorized path for PNG Sub filter (filter 1), used by `encode_png_bytes`:
        sub_pix = rows[:, 1:].reshape(h, w, bpp).astype(np.int32)
        recon = (np.cumsum(sub_pix, axis=1) & 0xFF).astype(np.uint8).reshape(h, stride)
    elif np.all(rows[:, 0] == 0):
        recon[:] = rows[:, 1:]
    else:
        for y in range(h):
            ftype = int(rows[y, 0])
            line = rows[y, 1:].astype(np.int32)
            up = recon[y - 1].astype(np.int32) if y > 0 else np.zeros(stride, dtype=np.int32)
            if ftype == 0:
                recon[y] = line.astype(np.uint8)
            elif ftype == 1:
                recon[y] = (np.cumsum(line.reshape(w, bpp), axis=0) & 0xFF).astype(np.uint8).reshape(stride)
            elif ftype == 2:
                recon[y] = ((line + up) & 0xFF).astype(np.uint8)
            elif ftype == 3:
                out = line.copy()
                for x in range(stride):
                    left = out[x - bpp] if x >= bpp else 0
                    out[x] = (out[x] + ((left + up[x]) >> 1)) & 0xFF
                recon[y] = out.astype(np.uint8)
            elif ftype == 4:
                out = line.copy()
                for x in range(stride):
                    a = out[x - bpp] if x >= bpp else 0
                    b = up[x]
                    c = up[x - bpp] if x >= bpp else 0
                    p = a + b - c
                    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                    pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                    out[x] = (out[x] + pr) & 0xFF
                recon[y] = out.astype(np.uint8)
            else:
                raise ValueError(f"Unsupported PNG filter {ftype}")

    rgba = np.empty((h, w, 4), dtype=np.uint8)
    if color_type == 6:
        rgba[:] = recon.reshape(h, w, 4)
    elif color_type == 2:
        rgba[:, :, :3] = recon.reshape(h, w, 3)
        rgba[:, :, 3] = 255
    elif color_type == 0:
        g = recon.reshape(h, w)
        rgba[:, :, 0] = g
        rgba[:, :, 1] = g
        rgba[:, :, 2] = g
        rgba[:, :, 3] = 255
    elif color_type == 4:
        ga = recon.reshape(h, w, 2)
        rgba[:, :, 0] = ga[:, :, 0]
        rgba[:, :, 1] = ga[:, :, 0]
        rgba[:, :, 2] = ga[:, :, 0]
        rgba[:, :, 3] = ga[:, :, 1]
    elif color_type == 3:
        idx = recon.reshape(h, w)
        assert plte is not None
        pal_rgba = np.zeros((256, 4), dtype=np.uint8)
        pal_rgba[:, 3] = 255
        pal_rgba[: len(plte), :3] = plte
        if trns is not None:
            pal_rgba[: len(trns), 3] = trns
        rgba[:] = pal_rgba[idx]
    return rgba


def linear_to_srgb(lin: np.ndarray) -> np.ndarray:
    """Standard IEC 61966-2-1 linear [0..1] -> sRGB [0..1] transfer."""
    x = np.clip(lin, 0.0, 1.0)
    return np.where(x <= 0.0031308, x * 12.92, 1.055 * np.power(np.maximum(x, 1e-8), 1.0 / 2.4) - 0.055)


def dilate_uv_islands(
    data: np.ndarray,
    valid_mask: np.ndarray,
    iterations: int = 12,
    region_mask: np.ndarray | None = None,
) -> Tuple[np.ndarray, np.ndarray]:
    """
    Morphologically dilates `data` `(H, W, C)` outward from `valid_mask` `(H, W)` into empty
    UV margin pixels (restricted to `region_mask` if given) so bilinear & mipmap sampling
    across UV island borders never samples unbaked background color.
    """
    out = data.copy()
    filled = valid_mask.copy()
    if region_mask is not None:
        filled &= region_mask

    offsets = ((-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (-1, 1), (1, -1), (1, 1))
    h, w = filled.shape
    for _ in range(iterations):
        unfilled = (~filled) & (region_mask if region_mask is not None else True)
        if not np.any(unfilled):
            break
        acc = np.zeros_like(out, dtype=np.float32)
        cnt = np.zeros((h, w), dtype=np.float32)
        for dy, dx in offsets:
            y0_src = max(0, -dy)
            y1_src = min(h, h - dy)
            x0_src = max(0, -dx)
            x1_src = min(w, w - dx)
            y0_dst = max(0, dy)
            y1_dst = min(h, h + dy)
            x0_dst = max(0, dx)
            x1_dst = min(w, w + dx)
            src_m = filled[y0_src:y1_src, x0_src:x1_src]
            dst_need = unfilled[y0_dst:y1_dst, x0_dst:x1_dst] & src_m
            if np.any(dst_need):
                acc[y0_dst:y1_dst, x0_dst:x1_dst][dst_need] += out[y0_src:y1_src, x0_src:x1_src][dst_need]
                cnt[y0_dst:y1_dst, x0_dst:x1_dst][dst_need] += 1.0
        newly = cnt > 0.0
        if not np.any(newly):
            break
        out[newly] = (acc[newly] / cnt[newly, None]).astype(out.dtype)
        filled |= newly
    return out, filled
