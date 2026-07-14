#!/usr/bin/env python3
"""Sharper non-AI procedural planet renderer prototype.

This version builds layered deterministic maps first, then renders them onto
planet and banner targets with lighting, relief, cloud shadow, atmosphere,
specular ocean response, city lights, and rings.
"""

from __future__ import annotations

import json
import math
import struct
import zlib
from array import array
from dataclasses import asdict, dataclass
from pathlib import Path
from random import Random


ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = ROOT / "assets" / "planet-prototype"
SEED = 0x5EED_1208_0001
MAP_W = 1536
MAP_H = 768


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
    render_model: str


@dataclass
class PlanetMaps:
    width: int
    height: int
    color: bytearray
    height_map: array
    water_map: array
    cloud_map: array
    city_map: array


def clamp(value: float, low: float = 0.0, high: float = 1.0) -> float:
    return low if value < low else high if value > high else value


def fade(t: float) -> float:
    return t * t * t * (t * (t * 6.0 - 15.0) + 10.0)


def smoothstep(edge0: float, edge1: float, value: float) -> float:
    if edge0 == edge1:
        return 0.0
    t = clamp((value - edge0) / (edge1 - edge0))
    return t * t * (3.0 - 2.0 * t)


def mix(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def mix_rgb(a: tuple[int, int, int], b: tuple[int, int, int], t: float) -> tuple[int, int, int]:
    t = clamp(t)
    return (
        int(mix(a[0], b[0], t)),
        int(mix(a[1], b[1], t)),
        int(mix(a[2], b[2], t)),
    )


def norm3(x: float, y: float, z: float) -> tuple[float, float, float]:
    length = math.sqrt(x * x + y * y + z * z) or 1.0
    return x / length, y / length, z / length


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

    path.write_bytes(
        b"".join(
            [
                b"\x89PNG\r\n\x1a\n",
                chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)),
                chunk(b"IDAT", zlib.compress(bytes(rows), 7)),
                chunk(b"IEND", b""),
            ]
        )
    )


def sharpen_opaque(buf: bytearray, width: int, height: int, amount: float) -> None:
    """Small unsharp pass over mostly opaque image regions."""
    src = bytes(buf)
    for y in range(1, height - 1):
        for x in range(1, width - 1):
            idx = (y * width + x) * 4
            if src[idx + 3] < 210:
                continue
            for channel in range(3):
                center = src[idx + channel]
                left = src[idx - 4 + channel]
                right = src[idx + 4 + channel]
                up = src[idx - width * 4 + channel]
                down = src[idx + width * 4 + channel]
                blur = (center * 4 + left + right + up + down) / 8.0
                buf[idx + channel] = int(clamp(center + (center - blur) * amount, 0, 255))


def put_pixel(buf: bytearray, width: int, height: int, x: int, y: int, rgba: tuple[int, int, int, int]) -> None:
    if x < 0 or x >= width or y < 0 or y >= height:
        return
    sr, sg, sb, sa = rgba
    if sa <= 0:
        return
    idx = (y * width + x) * 4
    if sa >= 255:
        buf[idx] = sr
        buf[idx + 1] = sg
        buf[idx + 2] = sb
        buf[idx + 3] = 255
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


def make_profile(seed: int) -> PlanetVisualProfile:
    rng = Random(seed)
    return PlanetVisualProfile(
        seed=seed,
        algorithm="procedural-planet-v2",
        planet_class="ringed temperate ocean super-earth",
        radius_km=11100 + rng.randrange(0, 1100),
        temperature_c=16 + rng.randrange(-5, 10),
        ocean_fraction=0.64 + rng.random() * 0.09,
        ice_fraction=0.13 + rng.random() * 0.06,
        cloud_density=0.40 + rng.random() * 0.13,
        atmosphere_density=0.78 + rng.random() * 0.12,
        volcanic_activity=0.08 + rng.random() * 0.07,
        ringed=True,
        palette="deep-ocean-forest-oxide-cloud",
        render_model="layered-fbm-heightmap-relief-atmosphere",
    )


PROFILE = make_profile(SEED)
LIGHT = norm3(-0.62, -0.18, 0.94)
VIEW = (0.0, 0.0, 1.0)


def fbm_map(width: int, height: int, seed: int, base_cell: int, octaves: int, persistence: float) -> array:
    out = array("f", [0.0]) * (width * height)
    amp = 1.0
    total_amp = 0.0

    for octave in range(octaves):
        cell = max(2, int(base_cell / (2**octave)))
        gw = max(2, math.ceil(width / cell))
        gh = max(2, math.ceil(height / cell) + 1)
        rng = Random(seed + octave * 7919)
        grid = [rng.random() for _ in range(gw * gh)]

        x_samples: list[tuple[int, int, float]] = []
        for x in range(width):
            gx = x / cell
            ix = int(gx)
            tx = fade(gx - ix)
            x_samples.append((ix % gw, (ix + 1) % gw, tx))

        for y in range(height):
            gy = y / cell
            iy = min(int(gy), gh - 2)
            ty = fade(gy - iy)
            row0 = iy * gw
            row1 = (iy + 1) * gw
            base = y * width
            for x, (ix0, ix1, tx) in enumerate(x_samples):
                v00 = grid[row0 + ix0]
                v10 = grid[row0 + ix1]
                v01 = grid[row1 + ix0]
                v11 = grid[row1 + ix1]
                a = v00 + (v10 - v00) * tx
                b = v01 + (v11 - v01) * tx
                out[base + x] += (a + (b - a) * ty) * amp

        total_amp += amp
        amp *= persistence

    inv = 1.0 / total_amp
    min_v = 999.0
    max_v = -999.0
    for i, value in enumerate(out):
        v = value * inv
        out[i] = v
        if v < min_v:
            min_v = v
        if v > max_v:
            max_v = v

    span = max(max_v - min_v, 0.0001)
    for i, value in enumerate(out):
        out[i] = (value - min_v) / span
    return out


def ridge(value: float) -> float:
    return 1.0 - abs(value * 2.0 - 1.0)


def build_maps(profile: PlanetVisualProfile, width: int, height: int) -> PlanetMaps:
    continents = fbm_map(width, height, profile.seed + 101, 280, 7, 0.54)
    tectonic = fbm_map(width, height, profile.seed + 211, 96, 6, 0.50)
    moisture = fbm_map(width, height, profile.seed + 307, 210, 5, 0.55)
    cloud_base = fbm_map(width, height, profile.seed + 409, 190, 6, 0.56)
    cloud_detail = fbm_map(width, height, profile.seed + 503, 42, 4, 0.48)
    city_noise = fbm_map(width, height, profile.seed + 601, 34, 4, 0.47)

    color = bytearray(width * height * 4)
    height_map = array("f", [0.0]) * (width * height)
    water_map = array("f", [0.0]) * (width * height)
    cloud_map = array("f", [0.0]) * (width * height)
    city_map = array("f", [0.0]) * (width * height)

    for y in range(height):
        v = y / (height - 1)
        lat_signed = (v - 0.5) * 2.0
        lat = abs(lat_signed)
        polar = smoothstep(0.62, 0.95, lat)
        temp = clamp(1.0 - lat * 1.25)
        wind = math.sin(v * math.pi * 13.0 + math.sin(v * math.pi * 3.0) * 0.8)
        for x in range(width):
            i = y * width + x
            u = x / width
            plate = ridge(tectonic[i])
            mountain = smoothstep(0.58, 0.92, plate) * (0.30 + tectonic[i] * 0.35)
            h = (
                continents[i] * 1.05
                + mountain * 0.22
                - 0.53
                - polar * 0.055
                + math.sin((u * math.pi * 2.0) + continents[i] * 2.5) * 0.012
            )
            ocean_level = 0.035 + (0.68 - profile.ocean_fraction) * 0.18
            water = clamp(smoothstep(ocean_level + 0.012, ocean_level - 0.045, h))
            land = 1.0 - water
            coast = 1.0 - smoothstep(0.002, 0.060, abs(h - ocean_level))
            arid = clamp((1.0 - moisture[i]) * 1.15 + temp * 0.18 - land * 0.06)
            forest = clamp((moisture[i] - 0.28) * 1.35 * temp)
            high = smoothstep(ocean_level + 0.16, ocean_level + 0.42, h)
            ice = clamp(smoothstep(0.68, 0.98, lat + (0.50 - moisture[i]) * 0.16) * profile.ice_fraction * 4.5)

            if water > 0.5:
                depth = clamp((ocean_level - h) * 5.5)
                shallow = 1.0 - depth
                rgb = mix_rgb((5, 17, 44), (16, 91, 122), shallow)
                rgb = mix_rgb(rgb, (36, 139, 151), coast * 0.65)
            else:
                soil = mix_rgb((106, 86, 56), (179, 151, 98), arid)
                veg = mix_rgb((35, 88, 61), (47, 126, 75), forest)
                oxide = mix_rgb((104, 66, 47), (164, 101, 64), arid * 0.55 + high * 0.18)
                rock = mix_rgb((95, 93, 84), (174, 168, 145), high)
                rgb = mix_rgb(soil, veg, forest * 0.78)
                rgb = mix_rgb(rgb, oxide, arid * 0.44)
                rgb = mix_rgb(rgb, rock, high * 0.58)
                rgb = mix_rgb(rgb, (210, 221, 216), ice)

            cloud_cell = cloud_base[i] * 0.74 + cloud_detail[i] * 0.26
            storm_band = smoothstep(0.42, 0.95, wind * 0.5 + 0.5) * 0.13
            cloud = smoothstep(0.58, 0.79, cloud_cell + storm_band - lat * 0.05)
            cloud *= profile.cloud_density * (0.78 + moisture[i] * 0.36)

            settlement = smoothstep(0.78, 0.96, city_noise[i])
            habitable = smoothstep(0.10, 0.34, temp) * smoothstep(0.85, 0.42, lat)
            cities = settlement * land * habitable * smoothstep(0.01, 0.09, coast)

            idx = i * 4
            color[idx] = rgb[0]
            color[idx + 1] = rgb[1]
            color[idx + 2] = rgb[2]
            color[idx + 3] = 255
            height_map[i] = clamp((h + 0.24) / 0.72)
            water_map[i] = water
            cloud_map[i] = cloud
            city_map[i] = cities

    return PlanetMaps(width, height, color, height_map, water_map, cloud_map, city_map)


def rotate_for_map(nx: float, ny: float, nz: float) -> tuple[float, float, float]:
    yaw = 0.92
    pitch = -0.12
    roll = 0.06
    cy, sy = math.cos(yaw), math.sin(yaw)
    cp, sp = math.cos(pitch), math.sin(pitch)
    cr, sr = math.cos(roll), math.sin(roll)

    x1 = nx * cy + nz * sy
    z1 = -nx * sy + nz * cy
    y2 = ny * cp - z1 * sp
    z2 = ny * sp + z1 * cp
    x3 = x1 * cr - y2 * sr
    y3 = x1 * sr + y2 * cr
    return x3, y3, z2


def map_coord(nx: float, ny: float, nz: float, maps: PlanetMaps) -> tuple[float, float]:
    rx, ry, rz = rotate_for_map(nx, ny, nz)
    lon = math.atan2(rz, rx)
    lat = math.asin(clamp(ry, -1.0, 1.0))
    u = ((lon / (math.pi * 2.0)) + 0.5) * maps.width
    v = (0.5 - lat / math.pi) * maps.height
    return u % maps.width, clamp(v, 0.0, maps.height - 1.001)


def sample_scalar(data: array, maps: PlanetMaps, u: float, v: float) -> float:
    x0 = int(u) % maps.width
    y0 = int(v)
    x1 = (x0 + 1) % maps.width
    y1 = min(y0 + 1, maps.height - 1)
    tx = u - int(u)
    ty = v - y0
    i00 = y0 * maps.width + x0
    i10 = y0 * maps.width + x1
    i01 = y1 * maps.width + x0
    i11 = y1 * maps.width + x1
    a = data[i00] + (data[i10] - data[i00]) * tx
    b = data[i01] + (data[i11] - data[i01]) * tx
    return a + (b - a) * ty


def sample_color(maps: PlanetMaps, u: float, v: float) -> tuple[int, int, int]:
    x0 = int(u) % maps.width
    y0 = int(v)
    x1 = (x0 + 1) % maps.width
    y1 = min(y0 + 1, maps.height - 1)
    tx = u - int(u)
    ty = v - y0
    out = []
    for channel in range(3):
        i00 = (y0 * maps.width + x0) * 4 + channel
        i10 = (y0 * maps.width + x1) * 4 + channel
        i01 = (y1 * maps.width + x0) * 4 + channel
        i11 = (y1 * maps.width + x1) * 4 + channel
        a = maps.color[i00] + (maps.color[i10] - maps.color[i00]) * tx
        b = maps.color[i01] + (maps.color[i11] - maps.color[i01]) * tx
        out.append(int(a + (b - a) * ty))
    return out[0], out[1], out[2]


def surface_rgba(nx: float, ny: float, nz: float, maps: PlanetMaps, edge_alpha: float) -> tuple[int, int, int, int]:
    u, v = map_coord(nx, ny, nz, maps)
    base = sample_color(maps, u, v)
    water = sample_scalar(maps.water_map, maps, u, v)
    cloud = sample_scalar(maps.cloud_map, maps, u, v)
    city = sample_scalar(maps.city_map, maps, u, v)

    h_l = sample_scalar(maps.height_map, maps, u - 1.2, v)
    h_r = sample_scalar(maps.height_map, maps, u + 1.2, v)
    h_u = sample_scalar(maps.height_map, maps, u, v - 1.2)
    h_d = sample_scalar(maps.height_map, maps, u, v + 1.2)
    relief = clamp(1.0 + (h_l - h_r) * 2.4 + (h_d - h_u) * 1.8, 0.58, 1.42)

    light = nx * LIGHT[0] + ny * LIGHT[1] + nz * LIGHT[2]
    lit = smoothstep(-0.20, 0.10, light)
    diffuse = (0.14 + max(light, 0.0) * 1.03) * relief
    limb = clamp((1.0 - nz) ** 1.85)

    shadow_u = u - LIGHT[0] * 16.0
    shadow_v = v + LIGHT[1] * 12.0
    cloud_shadow = sample_scalar(maps.cloud_map, maps, shadow_u, shadow_v) * water * lit * 0.23

    r, g, b = base
    r = int(r * diffuse * (1.0 - cloud_shadow))
    g = int(g * diffuse * (1.0 - cloud_shadow))
    b = int(b * diffuse * (1.0 - cloud_shadow))

    if water > 0.46 and light > -0.05:
        hx, hy, hz = norm3(LIGHT[0] + VIEW[0], LIGHT[1] + VIEW[1], LIGHT[2] + VIEW[2])
        spec = max(nx * hx + ny * hy + nz * hz, 0.0) ** 96
        spec *= smoothstep(0.35, 0.86, water)
        r = int(clamp(r + spec * 120, 0, 255))
        g = int(clamp(g + spec * 158, 0, 255))
        b = int(clamp(b + spec * 198, 0, 255))

    if city > 0.05:
        night = 1.0 - smoothstep(-0.13, 0.08, light)
        glow = city * night
        r = int(clamp(r + glow * 255, 0, 255))
        g = int(clamp(g + glow * 168, 0, 255))
        b = int(clamp(b + glow * 74, 0, 255))

    if cloud > 0.02:
        cloud_light = smoothstep(-0.12, 0.22, light)
        alpha = cloud * cloud_light * 0.88
        r = int(mix(r, 240, alpha))
        g = int(mix(g, 246, alpha))
        b = int(mix(b, 248, alpha))

    atmosphere = limb * PROFILE.atmosphere_density * smoothstep(-0.34, 0.22, light)
    r = int(clamp(r + atmosphere * 38, 0, 255))
    g = int(clamp(g + atmosphere * 74, 0, 255))
    b = int(clamp(b + atmosphere * 114, 0, 255))

    darkness = smoothstep(1.02, 0.62, nx * nx + ny * ny)
    r = int(r * (0.84 + darkness * 0.16))
    g = int(g * (0.84 + darkness * 0.16))
    b = int(b * (0.84 + darkness * 0.16))

    return r, g, b, int(255 * edge_alpha)


def background_pixel(x: int, y: int, width: int, height: int, seed: int) -> tuple[int, int, int, int]:
    u = x / max(width - 1, 1)
    v = y / max(height - 1, 1)
    core = math.exp(-((u - 0.13) ** 2 / 0.020 + (v - 0.18) ** 2 / 0.050))
    band = math.exp(-((u - 0.53) ** 2 / 0.22 + (v - 1.18) ** 2 / 0.18))
    r = int(4 + core * 22 + band * 20)
    g = int(7 + core * 25 + band * 15)
    b = int(18 + core * 58 + band * 28)
    h = ((x * 374761393 + y * 668265263 + seed * 31) & 0xFFFFFFFF)
    h ^= h >> 13
    h = (h * 1274126177) & 0xFFFFFFFF
    star = h / 0xFFFFFFFF
    if star > 0.9972:
        s = int((star - 0.9972) / 0.0028 * 220) + 40
        r = int(clamp(r + s, 0, 255))
        g = int(clamp(g + s, 0, 255))
        b = int(clamp(b + s, 0, 255))
    return r, g, b, 255


def render_background(buf: bytearray, width: int, height: int, seed: int) -> None:
    for y in range(height):
        for x in range(width):
            put_pixel(buf, width, height, x, y, background_pixel(x, y, width, height, seed))


def render_planet(buf: bytearray, width: int, height: int, maps: PlanetMaps, cx: float, cy: float, radius: float) -> None:
    pad = int(radius * 0.18)
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
                z = math.sqrt(1.0 - d2)
                edge = 1.0 - smoothstep(0.992, 1.0, math.sqrt(d2))
                put_pixel(buf, width, height, x, y, surface_rgba(dx, dy, z, maps, edge))
            elif d2 <= 1.055:
                d = math.sqrt(d2)
                glow = smoothstep(1.036, 0.999, d) * PROFILE.atmosphere_density
                if glow > 0.01:
                    put_pixel(buf, width, height, x, y, (65, 144, 218, int(30 * glow)))


def ring_rgba(x: int, y: int, cx: float, cy: float, radius: float, front: bool) -> tuple[int, int, int, int] | None:
    dx = x + 0.5 - cx
    dy = y + 0.5 - cy
    angle = -0.155
    ca, sa = math.cos(angle), math.sin(angle)
    xr = dx * ca - dy * sa
    yr = dx * sa + dy * ca
    a = radius * 1.86
    b = radius * 0.34
    radial = math.sqrt((xr / a) ** 2 + (yr / b) ** 2)
    if radial < 0.73 or radial > 1.31:
        return None

    disk = math.sqrt(dx * dx + dy * dy) / radius
    if front:
        if not (disk < 1.04 and yr > 0.0):
            return None
    elif disk < 1.035:
        return None

    gap_a = 1.0 - smoothstep(0.012, 0.0, abs(radial - 0.91))
    gap_b = 1.0 - smoothstep(0.014, 0.0, abs(radial - 1.13))
    bands = 0.62 + 0.24 * math.sin(radial * 116.0) + 0.10 * math.sin(radial * 271.0 + 0.7)
    edge = smoothstep(0.73, 0.79, radial) * (1.0 - smoothstep(1.24, 1.31, radial))
    alpha = clamp(bands) * gap_a * gap_b * edge
    if alpha <= 0.018:
        return None
    if front:
        alpha *= 0.58
    rgb = mix_rgb((132, 125, 108), (232, 220, 188), bands)
    return rgb[0], rgb[1], rgb[2], int(126 * alpha)


def draw_rings(buf: bytearray, width: int, height: int, cx: float, cy: float, radius: float, front: bool) -> None:
    x0 = max(0, int(cx - radius * 2.55))
    x1 = min(width, int(cx + radius * 2.55) + 1)
    y0 = max(0, int(cy - radius * 0.86))
    y1 = min(height, int(cy + radius * 0.86) + 1)
    for y in range(y0, y1):
        for x in range(x0, x1):
            rgba = ring_rgba(x, y, cx, cy, radius, front)
            if rgba is not None:
                put_pixel(buf, width, height, x, y, rgba)


def render_moon(buf: bytearray, width: int, height: int, cx: int, cy: int, radius: int, seed: int) -> None:
    for y in range(cy - radius - 2, cy + radius + 3):
        dy = (y + 0.5 - cy) / radius
        for x in range(cx - radius - 2, cx + radius + 3):
            dx = (x + 0.5 - cx) / radius
            d2 = dx * dx + dy * dy
            if d2 <= 1.0:
                z = math.sqrt(1.0 - d2)
                light = clamp(dx * LIGHT[0] + dy * LIGHT[1] + z * LIGHT[2])
                rough = math.sin((dx * 13.7 + dy * 7.9 + seed * 0.001) * 3.1) * 0.5 + 0.5
                crater = smoothstep(0.78, 0.96, rough) * 26
                base = int(68 + light * 118 + crater)
                edge = 1.0 - smoothstep(0.965, 1.0, math.sqrt(d2))
                put_pixel(buf, width, height, x, y, (base, base + 2, base + 7, int(255 * edge)))


def render_icon(path: Path, maps: PlanetMaps, size: int = 768) -> None:
    buf = bytearray(size * size * 4)
    render_planet(buf, size, size, maps, size / 2, size / 2, size * 0.405)
    sharpen_opaque(buf, size, size, 0.72)
    write_png(path, size, size, buf)


def render_banner(path: Path, maps: PlanetMaps, width: int = 1800, height: int = 620) -> None:
    buf = bytearray(width * height * 4)
    render_background(buf, width, height, PROFILE.seed & 0xFFFF)
    cx = width * 0.755
    cy = height * 0.61
    radius = height * 0.67
    draw_rings(buf, width, height, cx, cy, radius, front=False)
    render_planet(buf, width, height, maps, cx, cy, radius)
    draw_rings(buf, width, height, cx, cy, radius, front=True)
    render_moon(buf, width, height, int(width * 0.225), int(height * 0.25), 20, PROFILE.seed)
    sharpen_opaque(buf, width, height, 0.46)
    write_png(path, width, height, buf)


def render_surface_map(path: Path, maps: PlanetMaps, width: int = 768, height: int = 384) -> None:
    buf = bytearray(width * height * 4)
    for y in range(height):
        src_y = int(y / height * maps.height)
        for x in range(width):
            src_x = int(x / width * maps.width)
            src = (src_y * maps.width + src_x) * 4
            dst = (y * width + x) * 4
            buf[dst : dst + 4] = maps.color[src : src + 4]
    write_png(path, width, height, buf)


def write_preview(path: Path, icon_name: str, banner_name: str, map_name: str) -> None:
    html = f"""<!doctype html>
<meta charset="utf-8">
<title>Universus Planet Visual Prototype V2</title>
<style>
body {{
  margin: 0;
  min-height: 100vh;
  background: #050711;
  color: #dbeaf1;
  font-family: Arial, sans-serif;
  display: grid;
  place-items: center;
}}
.wrap {{
  width: min(1180px, calc(100vw - 32px));
  padding: 28px 0;
}}
.banner {{
  position: relative;
  overflow: hidden;
  border: 1px solid rgba(159, 205, 232, .20);
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
  max-width: 560px;
  text-shadow: 0 2px 18px rgba(0,0,0,.90);
}}
h1 {{
  margin: 0 0 8px;
  font-size: 35px;
  letter-spacing: 0;
}}
p {{
  margin: 0;
  color: #bfd0d8;
}}
.row {{
  display: grid;
  grid-template-columns: 174px 1fr;
  align-items: center;
  gap: 20px;
  margin-top: 18px;
}}
.icon {{
  width: 174px;
  height: 174px;
  background: radial-gradient(circle, rgba(74,145,205,.14), transparent 65%);
}}
.map {{
  width: 100%;
  margin-top: 14px;
  border: 1px solid rgba(159, 205, 232, .16);
}}
code {{
  color: #9fd4ff;
}}
</style>
<main class="wrap">
  <section class="banner" aria-label="Generated planet overview banner">
    <img src="{banner_name}" alt="Procedurally generated overview banner for New Terra">
    <div class="caption">
      <h1>New Terra</h1>
      <p>{PROFILE.planet_class} - G1:S120:P8 - seed <code>{PROFILE.seed}</code></p>
    </div>
  </section>
  <div class="row">
    <img class="icon" src="{icon_name}" alt="Procedurally generated planet icon">
    <p>V2 uses layered surface maps, relief lighting, cloud shadows, ocean specular, atmosphere glow, city lights, and sharper ring bands from the same deterministic seed.</p>
  </div>
  <img class="map" src="{map_name}" alt="Generated equirectangular surface texture map">
</main>
"""
    path.write_text(html, encoding="utf-8")


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    maps = build_maps(PROFILE, MAP_W, MAP_H)

    icon = OUT_DIR / "new-terra-v2-icon.png"
    banner = OUT_DIR / "new-terra-v2-overview-banner.png"
    surface = OUT_DIR / "new-terra-v2-surface-map.png"
    preview = OUT_DIR / "preview-v2.html"
    profile = OUT_DIR / "new-terra-v2-profile.json"

    render_icon(icon, maps)
    render_banner(banner, maps)
    render_surface_map(surface, maps)
    write_preview(preview, icon.name, banner.name, surface.name)
    profile.write_text(json.dumps(asdict(PROFILE), indent=2), encoding="utf-8")

    print(f"wrote {icon}")
    print(f"wrote {banner}")
    print(f"wrote {surface}")
    print(f"wrote {preview}")
    print(f"wrote {profile}")


if __name__ == "__main__":
    main()
