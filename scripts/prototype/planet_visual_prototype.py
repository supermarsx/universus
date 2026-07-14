#!/usr/bin/env python3
"""Non-AI procedural planet visual prototype.

Generates one deterministic planet icon and one overview banner using only
math, seeded randomness, and a tiny built-in PNG writer.
"""

from __future__ import annotations

import json
import math
import struct
import zlib
from dataclasses import asdict, dataclass
from pathlib import Path
from random import Random


ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = ROOT / "assets" / "planet-prototype"
SEED = 0x5EED_1208_0001


@dataclass(frozen=True)
class PlanetVisualProfile:
    seed: int
    algorithm: str
    planet_class: str
    radius_km: int
    temperature_c: int
    ocean_fraction: float
    ice_fraction: float
    cloud_density: float
    atmosphere_density: float
    volcanic_activity: float
    ringed: bool
    palette: str


def clamp(value: float, low: float = 0.0, high: float = 1.0) -> float:
    return low if value < low else high if value > high else value


def smoothstep(edge0: float, edge1: float, value: float) -> float:
    if edge0 == edge1:
        return 0.0
    t = clamp((value - edge0) / (edge1 - edge0))
    return t * t * (3.0 - 2.0 * t)


def mix(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def mix_rgb(a: tuple[int, int, int], b: tuple[int, int, int], t: float) -> tuple[int, int, int]:
    return (
        int(mix(a[0], b[0], t)),
        int(mix(a[1], b[1], t)),
        int(mix(a[2], b[2], t)),
    )


def norm3(x: float, y: float, z: float) -> tuple[float, float, float]:
    length = math.sqrt(x * x + y * y + z * z) or 1.0
    return x / length, y / length, z / length


def hash_u32(a: int, b: int, c: int = 0) -> int:
    n = (a * 374761393 + b * 668265263 + c * 2246822519 + 0x9E3779B9) & 0xFFFFFFFF
    n ^= n >> 13
    n = (n * 1274126177) & 0xFFFFFFFF
    n ^= n >> 16
    return n & 0xFFFFFFFF


def hash_float(a: int, b: int, c: int = 0) -> float:
    return hash_u32(a, b, c) / 0xFFFFFFFF


def fbm3(x: float, y: float, z: float, seed: int, octaves: int = 5) -> float:
    total = 0.0
    amp = 0.56
    freq = 1.0
    weight = 0.0
    for i in range(octaves):
        s = seed + i * 1013
        p1 = (hash_float(s, 11) * 6.2831853)
        p2 = (hash_float(s, 29) * 6.2831853)
        a = 2.7 + hash_float(s, 41) * 3.8
        b = 2.1 + hash_float(s, 53) * 4.2
        c = 2.4 + hash_float(s, 67) * 3.6
        wave = math.sin((x * a + y * b + z * c) * freq + p1)
        fold = math.sin(((x + y * 0.57) * c - z * a) * freq * 0.73 + p2)
        total += (wave * 0.68 + fold * 0.32) * amp
        weight += amp
        amp *= 0.52
        freq *= 2.03
    return total / weight


def make_profile(seed: int) -> PlanetVisualProfile:
    rng = Random(seed)
    return PlanetVisualProfile(
        seed=seed,
        algorithm="procedural-planet-v0",
        planet_class="ringed temperate ocean world",
        radius_km=10400 + rng.randrange(0, 1800),
        temperature_c=12 + rng.randrange(-8, 16),
        ocean_fraction=0.62 + rng.random() * 0.12,
        ice_fraction=0.10 + rng.random() * 0.08,
        cloud_density=0.46 + rng.random() * 0.16,
        atmosphere_density=0.82 + rng.random() * 0.12,
        volcanic_activity=0.08 + rng.random() * 0.08,
        ringed=True,
        palette="deep-ocean-jade-cloud-gold",
    )


PROFILE = make_profile(SEED)
LIGHT = norm3(-0.72, -0.34, 0.86)
VIEW = (0.0, 0.0, 1.0)


def rotate_normal(nx: float, ny: float, nz: float) -> tuple[float, float, float]:
    yaw = 0.72
    pitch = -0.18
    cy, sy = math.cos(yaw), math.sin(yaw)
    cp, sp = math.cos(pitch), math.sin(pitch)
    x1 = nx * cy + nz * sy
    z1 = -nx * sy + nz * cy
    y2 = ny * cp - z1 * sp
    z2 = ny * sp + z1 * cp
    return x1, y2, z2


def planet_surface_rgba(nx: float, ny: float, nz: float, edge_alpha: float) -> tuple[int, int, int, int]:
    sx, sy, sz = rotate_normal(nx, ny, nz)
    latitude = math.asin(clamp(sy, -1.0, 1.0))
    abs_lat = abs(latitude) / (math.pi / 2.0)

    warp = fbm3(sx * 1.3 + 4.0, sy * 1.3, sz * 1.3, PROFILE.seed + 17, 3) * 0.22
    terrain = fbm3((sx + warp) * 2.15, (sy - warp * 0.4) * 2.15, (sz + warp) * 2.15, PROFILE.seed + 71, 5)
    detail = fbm3(sx * 8.5, sy * 8.5, sz * 8.5, PROFILE.seed + 173, 4)
    moisture = fbm3(sx * 3.8 - 1.0, sy * 3.8 + 1.7, sz * 3.8, PROFILE.seed + 257, 4)

    ocean_level = mix(0.17, 0.03, PROFILE.ocean_fraction - 0.62)
    is_ocean = terrain < ocean_level
    snow = smoothstep(0.68, 0.91, abs_lat + detail * 0.12)

    if is_ocean:
        depth = clamp((ocean_level - terrain) * 2.2)
        shallow = clamp((terrain - ocean_level + 0.14) * 7.5)
        color = mix_rgb((8, 31, 62), (23, 111, 130), shallow)
        color = mix_rgb(color, (3, 14, 42), depth * 0.55)
    else:
        green = smoothstep(-0.12, 0.45, moisture - abs_lat * 0.18)
        dry = smoothstep(0.18, 0.72, terrain + abs_lat * 0.08)
        rock = mix_rgb((91, 82, 61), (148, 132, 92), dry)
        forest = mix_rgb((31, 78, 59), (45, 112, 77), clamp(green))
        highland = mix_rgb((88, 76, 64), (179, 160, 128), smoothstep(0.38, 0.88, terrain))
        color = mix_rgb(rock, forest, clamp(green * 0.78))
        color = mix_rgb(color, highland, smoothstep(0.46, 0.86, terrain))

    color = mix_rgb(color, (218, 231, 229), snow * PROFILE.ice_fraction * 5.5)

    light = clamp(nx * LIGHT[0] + ny * LIGHT[1] + nz * LIGHT[2], -1.0, 1.0)
    diffuse = 0.18 + max(light, 0.0) * 0.92
    dusk = smoothstep(-0.18, 0.16, light)
    terminator = 1.0 - dusk

    r, g, b = color
    r = int(r * diffuse)
    g = int(g * diffuse)
    b = int(b * diffuse)

    if not is_ocean and terminator > 0.35:
        city = fbm3(sx * 36.0, sy * 36.0, sz * 36.0, PROFILE.seed + 991, 3)
        if city > 0.58 and abs_lat < 0.58:
            glow = clamp((city - 0.58) * 5.0) * terminator
            r = int(mix(r, 255, glow * 0.72))
            g = int(mix(g, 177, glow * 0.52))
            b = int(mix(b, 77, glow * 0.32))

    if is_ocean:
        hx, hy, hz = norm3(LIGHT[0] + VIEW[0], LIGHT[1] + VIEW[1], LIGHT[2] + VIEW[2])
        spec = max(nx * hx + ny * hy + nz * hz, 0.0) ** 82
        r = int(clamp(r + spec * 120, 0, 255))
        g = int(clamp(g + spec * 155, 0, 255))
        b = int(clamp(b + spec * 185, 0, 255))

    cloud_base = fbm3(sx * 4.2 + 8.0, sy * 4.2 - 3.0, sz * 4.2, PROFILE.seed + 557, 5)
    cloud_fine = fbm3(sx * 14.0, sy * 14.0, sz * 14.0, PROFILE.seed + 653, 3)
    cloud = smoothstep(0.06, 0.54, cloud_base + cloud_fine * 0.28)
    cloud *= PROFILE.cloud_density
    cloud *= smoothstep(-0.28, 0.18, light)
    if cloud > 0.02:
        cr = int(mix(r, 238, cloud * 0.82))
        cg = int(mix(g, 245, cloud * 0.82))
        cb = int(mix(b, 248, cloud * 0.82))
        r, g, b = cr, cg, cb

    rim = (1.0 - nz) ** 2.2 * PROFILE.atmosphere_density
    rim *= smoothstep(-0.24, 0.28, light)
    r = int(clamp(r + rim * 52, 0, 255))
    g = int(clamp(g + rim * 88, 0, 255))
    b = int(clamp(b + rim * 124, 0, 255))

    return r, g, b, int(255 * edge_alpha)


def put_pixel(buf: bytearray, width: int, height: int, x: int, y: int, rgba: tuple[int, int, int, int]) -> None:
    if x < 0 or x >= width or y < 0 or y >= height:
        return
    idx = (y * width + x) * 4
    sr, sg, sb, sa = rgba
    if sa >= 255:
        buf[idx] = sr
        buf[idx + 1] = sg
        buf[idx + 2] = sb
        buf[idx + 3] = 255
        return
    if sa <= 0:
        return
    da = buf[idx + 3]
    inv = 255 - sa
    out_a = sa + (da * inv + 127) // 255
    if out_a <= 0:
        return
    buf[idx] = (sr * sa + buf[idx] * da * inv // 255) // out_a
    buf[idx + 1] = (sg * sa + buf[idx + 1] * da * inv // 255) // out_a
    buf[idx + 2] = (sb * sa + buf[idx + 2] * da * inv // 255) // out_a
    buf[idx + 3] = out_a


def write_png(path: Path, width: int, height: int, rgba: bytearray) -> None:
    def chunk(kind: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + kind
            + data
            + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
        )

    rows = bytearray()
    stride = width * 4
    for y in range(height):
        rows.append(0)
        rows.extend(rgba[y * stride : (y + 1) * stride])

    data = b"".join(
        [
            b"\x89PNG\r\n\x1a\n",
            chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)),
            chunk(b"IDAT", zlib.compress(bytes(rows), 7)),
            chunk(b"IEND", b""),
        ]
    )
    path.write_bytes(data)


def render_planet_layer(buf: bytearray, width: int, height: int, cx: float, cy: float, radius: float) -> None:
    pad = int(radius * 0.16)
    x0 = max(0, int(cx - radius - pad))
    x1 = min(width, int(cx + radius + pad) + 1)
    y0 = max(0, int(cy - radius - pad))
    y1 = min(height, int(cy + radius + pad) + 1)
    for y in range(y0, y1):
        dy = (y + 0.5 - cy) / radius
        for x in range(x0, x1):
            dx = (x + 0.5 - cx) / radius
            d2 = dx * dx + dy * dy
            if d2 <= 1.0:
                z = math.sqrt(max(0.0, 1.0 - d2))
                edge = 1.0 - smoothstep(0.985, 1.0, math.sqrt(d2))
                put_pixel(buf, width, height, x, y, planet_surface_rgba(dx, dy, z, edge))
            elif d2 < 1.22:
                d = math.sqrt(d2)
                glow = smoothstep(1.13, 0.995, d) * PROFILE.atmosphere_density
                put_pixel(buf, width, height, x, y, (74, 148, 210, int(72 * glow)))


def ring_rgba(x: int, y: int, cx: float, cy: float, radius: float, front: bool) -> tuple[int, int, int, int] | None:
    dx = x + 0.5 - cx
    dy = y + 0.5 - cy
    angle = -0.175
    ca, sa = math.cos(angle), math.sin(angle)
    xr = dx * ca - dy * sa
    yr = dx * sa + dy * ca
    a = radius * 1.86
    b = radius * 0.38
    radial = math.sqrt((xr / a) ** 2 + (yr / b) ** 2)
    if radial < 0.74 or radial > 1.33:
        return None
    disk = math.sqrt(dx * dx + dy * dy) / radius
    if front:
        if not (disk < 1.03 and yr > 0):
            return None
    elif disk < 1.04:
        return None
    bands = 0.58 + 0.26 * math.sin(radial * 97.0 + 0.4) + 0.12 * math.sin(radial * 211.0)
    gaps = 1.0 - smoothstep(0.017, 0.0, abs(radial - 0.93))
    gaps *= 1.0 - smoothstep(0.018, 0.0, abs(radial - 1.145))
    alpha = smoothstep(0.74, 0.82, radial) * (1.0 - smoothstep(1.25, 1.33, radial))
    alpha *= clamp(bands) * gaps
    if alpha <= 0.02:
        return None
    if front:
        alpha *= 0.56
    color = mix_rgb((120, 114, 103), (225, 214, 186), clamp(bands))
    return color[0], color[1], color[2], int(112 * alpha)


def draw_rings(buf: bytearray, width: int, height: int, cx: float, cy: float, radius: float, front: bool) -> None:
    x0 = max(0, int(cx - radius * 2.55))
    x1 = min(width, int(cx + radius * 2.55) + 1)
    y0 = max(0, int(cy - radius * 0.92))
    y1 = min(height, int(cy + radius * 0.92) + 1)
    for y in range(y0, y1):
        for x in range(x0, x1):
            rgba = ring_rgba(x, y, cx, cy, radius, front)
            if rgba is not None:
                put_pixel(buf, width, height, x, y, rgba)


def background_pixel(x: int, y: int, width: int, height: int) -> tuple[int, int, int, int]:
    u = x / max(width - 1, 1)
    v = y / max(height - 1, 1)
    nebula = fbm3(u * 2.4, v * 2.4, 0.35, PROFILE.seed + 2001, 4)
    glow_left = math.exp(-((u - 0.11) ** 2 / 0.028 + (v - 0.23) ** 2 / 0.055))
    glow_low = math.exp(-((u - 0.52) ** 2 / 0.12 + (v - 1.10) ** 2 / 0.20))
    r = 4 + int(20 * glow_left + 18 * glow_low + 8 * clamp(nebula))
    g = 7 + int(26 * glow_left + 13 * glow_low + 5 * clamp(nebula))
    b = 17 + int(58 * glow_left + 25 * glow_low + 16 * clamp(nebula))
    star = hash_float(x, y, PROFILE.seed & 0xFFFF)
    if star > 0.9966:
        s = int((star - 0.9966) / 0.0034 * 210) + 40
        r = clamp(r + s, 0, 255)
        g = clamp(g + s, 0, 255)
        b = clamp(b + s, 0, 255)
    return int(r), int(g), int(b), 255


def render_icon(path: Path, size: int = 640) -> None:
    buf = bytearray(size * size * 4)
    render_planet_layer(buf, size, size, size / 2, size / 2, size * 0.405)
    write_png(path, size, size, buf)


def render_banner(path: Path, width: int = 1600, height: int = 520) -> None:
    buf = bytearray(width * height * 4)
    for y in range(height):
        for x in range(width):
            put_pixel(buf, width, height, x, y, background_pixel(x, y, width, height))

    cx = width * 0.76
    cy = height * 0.62
    radius = height * 0.69
    draw_rings(buf, width, height, cx, cy, radius, front=False)
    render_planet_layer(buf, width, height, cx, cy, radius)
    draw_rings(buf, width, height, cx, cy, radius, front=True)

    moon_x = int(width * 0.245)
    moon_y = int(height * 0.28)
    moon_r = 18
    for y in range(moon_y - moon_r - 2, moon_y + moon_r + 3):
        for x in range(moon_x - moon_r - 2, moon_x + moon_r + 3):
            dx = (x + 0.5 - moon_x) / moon_r
            dy = (y + 0.5 - moon_y) / moon_r
            d2 = dx * dx + dy * dy
            if d2 <= 1.0:
                z = math.sqrt(1.0 - d2)
                light = clamp(dx * LIGHT[0] + dy * LIGHT[1] + z * LIGHT[2])
                rough = fbm3(dx * 6.0, dy * 6.0, z * 6.0, PROFILE.seed + 3033, 3)
                base = int(86 + rough * 22 + light * 82)
                edge = 1.0 - smoothstep(0.93, 1.0, math.sqrt(d2))
                put_pixel(buf, width, height, x, y, (base, base + 3, base + 7, int(255 * edge)))

    write_png(path, width, height, buf)


def write_preview(path: Path, icon_name: str, banner_name: str) -> None:
    html = f"""<!doctype html>
<meta charset="utf-8">
<title>Universus Planet Visual Prototype</title>
<style>
body {{
  margin: 0;
  min-height: 100vh;
  background: #050711;
  color: #d8e7ef;
  font-family: Arial, sans-serif;
  display: grid;
  place-items: center;
}}
.wrap {{
  width: min(1100px, calc(100vw - 32px));
}}
.banner {{
  position: relative;
  overflow: hidden;
  border: 1px solid rgba(160, 206, 232, .18);
  background: #080b16;
}}
.banner img {{
  display: block;
  width: 100%;
  height: auto;
}}
.caption {{
  position: absolute;
  left: 32px;
  bottom: 28px;
  max-width: 520px;
  text-shadow: 0 2px 16px rgba(0,0,0,.85);
}}
h1 {{
  margin: 0 0 8px;
  font-size: 34px;
  letter-spacing: 0;
}}
p {{
  margin: 0;
  color: #b9c9d3;
}}
.row {{
  display: flex;
  align-items: center;
  gap: 18px;
  margin-top: 18px;
}}
.icon {{
  width: 156px;
  height: 156px;
  background: radial-gradient(circle, rgba(77,145,195,.16), transparent 65%);
}}
code {{
  color: #9ed1ff;
}}
</style>
<main class="wrap">
  <section class="banner" aria-label="Generated planet overview banner">
    <img src="{banner_name}" alt="Procedurally generated overview banner for New Terra">
    <div class="caption">
      <h1>New Terra</h1>
      <p>Ringed temperate ocean world · G1:S120:P8 · seed <code>{PROFILE.seed}</code></p>
    </div>
  </section>
  <div class="row">
    <img class="icon" src="{icon_name}" alt="Procedurally generated planet icon">
    <p>Same seed drives the square planet icon and the overview banner.</p>
  </div>
</main>
"""
    path.write_text(html, encoding="utf-8")


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    icon = OUT_DIR / "new-terra-icon.png"
    banner = OUT_DIR / "new-terra-overview-banner.png"
    preview = OUT_DIR / "preview.html"
    profile = OUT_DIR / "new-terra-profile.json"

    render_icon(icon)
    render_banner(banner)
    write_preview(preview, icon.name, banner.name)
    profile.write_text(json.dumps(asdict(PROFILE), indent=2), encoding="utf-8")

    print(f"wrote {icon}")
    print(f"wrote {banner}")
    print(f"wrote {preview}")
    print(f"wrote {profile}")


if __name__ == "__main__":
    main()
