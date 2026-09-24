#!/usr/bin/env python3
"""Contenido propio de prueba para Revvy (sin datos de Re-Volt).

Genera, relativo a la raíz del repo:

- content/cars/revvy_buggy/: car.toml, body.glb y collision.glb.
- content/levels/revvy_arena/: track.toml, visual.glb y layout.ron.

Uso: python3 tools/test-content/generate.py
"""

import json
import math
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class Glb:
    """Escritor mínimo de glTF 2.0 binario: mallas con color base, nodos y escena."""

    def __init__(self):
        self.bin = bytearray()
        self.views = []
        self.accessors = []
        self.meshes = []
        self.materials = []
        self.material_ids = {}
        self.nodes = []

    def _view(self, data, target):
        while len(self.bin) % 4:
            self.bin.append(0)
        offset = len(self.bin)
        self.bin.extend(data)
        self.views.append({"buffer": 0, "byteOffset": offset, "byteLength": len(data), "target": target})
        return len(self.views) - 1

    def _vec3(self, values, bounds=False):
        data = b"".join(struct.pack("<3f", *v) for v in values)
        accessor = {
            "bufferView": self._view(data, 34962),
            "componentType": 5126,
            "count": len(values),
            "type": "VEC3",
        }
        if bounds:
            accessor["min"] = [min(v[i] for v in values) for i in range(3)]
            accessor["max"] = [max(v[i] for v in values) for i in range(3)]
        self.accessors.append(accessor)
        return len(self.accessors) - 1

    def _indices(self, values):
        data = b"".join(struct.pack("<I", i) for i in values)
        self.accessors.append({
            "bufferView": self._view(data, 34963),
            "componentType": 5125,
            "count": len(values),
            "type": "SCALAR",
        })
        return len(self.accessors) - 1

    def material(self, name, color):
        if name not in self.material_ids:
            self.materials.append({
                "name": name,
                "pbrMetallicRoughness": {"baseColorFactor": list(color) + [1.0], "metallicFactor": 0.0},
            })
            self.material_ids[name] = len(self.materials) - 1
        return self.material_ids[name]

    def mesh(self, parts):
        """`parts`: lista de (Geometry, nombre de material, color)."""
        primitives = []
        for geometry, material, color in parts:
            if not geometry.indices:
                continue
            primitives.append({
                "attributes": {
                    "POSITION": self._vec3(geometry.positions, bounds=True),
                    "NORMAL": self._vec3(geometry.normals),
                },
                "indices": self._indices(geometry.indices),
                "material": self.material(material, color),
            })
        self.meshes.append({"primitives": primitives})
        return len(self.meshes) - 1

    def node(self, name, mesh=None, translation=None, children=None):
        node = {"name": name}
        if mesh is not None:
            node["mesh"] = mesh
        if translation is not None:
            node["translation"] = list(translation)
        if children:
            node["children"] = children
        self.nodes.append(node)
        return len(self.nodes) - 1

    def write(self, path, roots):
        while len(self.bin) % 4:
            self.bin.append(0)
        document = {
            "asset": {"version": "2.0", "generator": "revvy tools/test-content"},
            "scene": 0,
            "scenes": [{"nodes": roots}],
            "nodes": self.nodes,
            "meshes": self.meshes,
            "materials": self.materials,
            "accessors": self.accessors,
            "bufferViews": self.views,
            "buffers": [{"byteLength": len(self.bin)}],
        }
        text = json.dumps(document, separators=(",", ":")).encode()
        while len(text) % 4:
            text += b" "
        total = 12 + 8 + len(text) + 8 + len(self.bin)
        out = bytearray(struct.pack("<4sII", b"glTF", 2, total))
        out += struct.pack("<I4s", len(text), b"JSON") + text
        out += struct.pack("<I4s", len(self.bin), b"BIN\0") + self.bin
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(bytes(out))


class Geometry:
    def __init__(self):
        self.positions = []
        self.normals = []
        self.indices = []

    def quad(self, a, b, c, d):
        """Cara plana a-b-c-d en sentido antihorario visto desde afuera."""
        n = normalize(cross(sub(b, a), sub(d, a)))
        base = len(self.positions)
        self.positions += [a, b, c, d]
        self.normals += [n] * 4
        self.indices += [base, base + 1, base + 2, base, base + 2, base + 3]

    def tri(self, a, b, c):
        n = normalize(cross(sub(b, a), sub(c, a)))
        base = len(self.positions)
        self.positions += [a, b, c]
        self.normals += [n] * 3
        self.indices += [base, base + 1, base + 2]

    def box(self, lo, hi):
        x0, y0, z0 = lo
        x1, y1, z1 = hi
        self.quad((x0, y1, z0), (x0, y1, z1), (x1, y1, z1), (x1, y1, z0))  # arriba
        self.quad((x0, y0, z0), (x1, y0, z0), (x1, y0, z1), (x0, y0, z1))  # abajo
        self.quad((x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1))  # +Z
        self.quad((x1, y0, z0), (x0, y0, z0), (x0, y1, z0), (x1, y1, z0))  # -Z
        self.quad((x1, y0, z1), (x1, y0, z0), (x1, y1, z0), (x1, y1, z1))  # +X
        self.quad((x0, y0, z0), (x0, y0, z1), (x0, y1, z1), (x0, y1, z0))  # -X

    def ramp(self, x0, x1, z0, z1, height):
        """Cuña que sube de z0 (altura 0) a z1 (altura `height`)."""
        self.quad((x0, 0.0, z0), (x0, height, z1), (x1, height, z1), (x1, 0.0, z0))
        self.quad((x0, 0.0, z1), (x1, 0.0, z1), (x1, height, z1), (x0, height, z1))
        self.tri((x1, 0.0, z0), (x1, height, z1), (x1, 0.0, z1))
        self.tri((x0, 0.0, z0), (x0, 0.0, z1), (x0, height, z1))

    def cylinder_x(self, radius, width, segments=16):
        """Rueda: cilindro sobre el eje X, centrado en el origen."""
        h = width / 2.0
        ring = [(math.cos(2 * math.pi * i / segments) * radius, math.sin(2 * math.pi * i / segments) * radius)
                for i in range(segments)]
        for i in range(segments):
            (y0, z0), (y1, z1) = ring[i], ring[(i + 1) % segments]
            self.quad((h, y0, z0), (-h, y0, z0), (-h, y1, z1), (h, y1, z1))
            self.tri((h, 0.0, 0.0), (h, y0, z0), (h, y1, z1))
            self.tri((-h, 0.0, 0.0), (-h, y1, z1), (-h, y0, z0))


def sub(a, b):
    return tuple(a[i] - b[i] for i in range(3))


def cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def normalize(v):
    length = math.sqrt(sum(c * c for c in v)) or 1.0
    return tuple(c / length for c in v)


# --------------------------------------------------------------------- auto propio

WHEELS = {
    "WheelFL": (0.11, 0.0, 0.16),
    "WheelFR": (-0.11, 0.0, 0.16),
    "WheelBL": (0.11, 0.0, -0.15),
    "WheelBR": (-0.11, 0.0, -0.15),
}
WHEEL_RADIUS = 0.05

CAR_TOML = """# Auto propio de prueba de Revvy: parámetros escritos a mano, sin datos de Re-Volt.
# Unidades SI, marco del auto (+X izquierda, +Y arriba, +Z adelante), relativo al centro de masa.
name = "Revvy Buggy"
body = "body.glb"
collision = "collision.glb"

[sound]
engine = "electric"

[vehicle]
mass = 1.6
inertia = [[0.035, 0.0, 0.0], [0.0, 0.042, 0.0], [0.0, 0.0, 0.012]]
gravity = 11.0
hardness = 0.1
resistance = 0.001
angular_resistance = 0.001
angular_resistance_air = 25.0
grip = 2.0
static_friction = 0.8
kinetic_friction = 0.4
body_offset = [0.0, 0.0, 0.0]
steer_rate = 3.0
engine_rate = 5.0
top_speed = 20.0
down_force = 2.0
"""

WHEEL_TOML = """
[[vehicle.wheels]]
present = true
powered = {powered}
steered = {steered}
offset = [{x}, {y}, {z}]
radius = {radius}
mass = 0.12
gravity = 11.0
max_travel = 0.025
skid_width = 0.05
steer_ratio = {steer_ratio}
engine_ratio = {engine_ratio}
axle_friction = 0.00005
grip = {grip}
static_friction = {static_friction}
kinetic_friction = {kinetic_friction}
spring = {{ stiffness = 550.0, damping = 7.0, restitution = -0.7 }}
"""


def buggy():
    folder = ROOT / "content" / "cars" / "revvy_buggy"
    text = CAR_TOML
    for name, (x, y, z) in WHEELS.items():
        front = name.startswith("WheelF")
        text += WHEEL_TOML.format(
            powered="false" if front else "true",
            steered="true" if front else "false",
            x=x, y=y, z=z, radius=WHEEL_RADIUS,
            steer_ratio=-0.35 if front else 0.0,
            engine_ratio=0.0 if front else 0.9,
            grip=3.0 if front else 3.5,
            static_friction=2.2 if front else 2.4,
            kinetic_friction=2.1 if front else 2.3,
        )
    folder.mkdir(parents=True, exist_ok=True)
    (folder / "car.toml").write_text(text)

    glb = Glb()
    chassis = Geometry()
    chassis.box((-0.13, 0.01, -0.23), (0.13, 0.07, 0.24))
    cabin = Geometry()
    cabin.box((-0.08, 0.07, -0.12), (0.08, 0.13, 0.06))
    spoiler = Geometry()
    spoiler.box((-0.12, 0.13, -0.24), (0.12, 0.145, -0.18))
    body = glb.node("Body", glb.mesh([
        (chassis, "Paint", (0.9, 0.35, 0.08)),
        (cabin, "Glass", (0.2, 0.25, 0.3)),
        (spoiler, "Trim", (0.12, 0.12, 0.12)),
    ]))
    wheel_geometry = Geometry()
    wheel_geometry.cylinder_x(WHEEL_RADIUS, 0.045)
    wheel_mesh = glb.mesh([(wheel_geometry, "Tyre", (0.08, 0.08, 0.08))])
    wheels = [glb.node(name, wheel_mesh, translation=pos) for name, pos in WHEELS.items()]
    glb.write(folder / "body.glb", [body] + wheels)

    collision = Glb()
    hull = Geometry()
    hull.box((-0.135, 0.005, -0.24), (0.135, 0.145, 0.245))
    nodes = [collision.node("Hull", collision.mesh([(hull, "Hull", (1, 1, 1))]))]
    for i, (x, y, z, r) in enumerate([
        (0.08, 0.05, 0.17, 0.045), (-0.08, 0.05, 0.17, 0.045),
        (0.08, 0.05, -0.16, 0.045), (-0.08, 0.05, -0.16, 0.045),
        (0.0, 0.08, 0.0, 0.06),
    ]):
        sphere = Geometry()
        sphere.box((x - r, y - r, z - r), (x + r, y + r, z + r))
        nodes.append(collision.node(f"Sphere{i + 1}", collision.mesh([(sphere, "Sphere", (1, 1, 1))])))
    collision.write(folder / "collision.glb", nodes)


# ---------------------------------------------------------------------- pista propia

TILE = 5.0
HALF = 60.0
SURFACE_COLORS = {
    "Road": (0.42, 0.42, 0.44),
    "Ice": (0.72, 0.88, 0.98),
    "Grass": (0.25, 0.55, 0.2),
    "Dirt": (0.5, 0.36, 0.22),
    "Pebbles": (0.6, 0.58, 0.52),
    "Wood": (0.62, 0.45, 0.25),
    "Stone": (0.55, 0.52, 0.5),
}


def tile_surface(ix, iz):
    x = -HALF + (ix + 0.5) * TILE
    z = -HALF + (iz + 0.5) * TILE
    if 15.0 < x < 45.0 and 10.0 < z < 40.0:
        return "Ice"
    if -45.0 < x < -15.0 and 10.0 < z < 40.0:
        return "Grass"
    if -45.0 < x < -15.0 and -35.0 < z < -10.0:
        return "Dirt"
    if 15.0 < x < 45.0 and -35.0 < z < -10.0:
        return "Pebbles"
    return "Road"


def arena():
    folder = ROOT / "content" / "levels" / "revvy_arena"
    tiles = int(2 * HALF / TILE)
    visual_ground = {}
    collision_ground = {}
    for ix in range(tiles):
        for iz in range(tiles):
            surface = tile_surface(ix, iz)
            x0, z0 = -HALF + ix * TILE, -HALF + iz * TILE
            x1, z1 = x0 + TILE, z0 + TILE
            corners = ((x0, 0.0, z0), (x0, 0.0, z1), (x1, 0.0, z1), (x1, 0.0, z0))
            shade = "a" if (ix + iz) % 2 == 0 else "b"
            visual_ground.setdefault((surface, shade), Geometry()).quad(*corners)
            collision_ground.setdefault(surface, Geometry()).quad(*corners)

    ramps = Geometry()
    ramps.ramp(-2.0, 2.0, 20.0, 26.0, 1.2)
    ramps.ramp(-22.0, -18.0, -2.0, 4.0, 0.8)
    ramps.ramp(18.0, 22.0, -2.0, 4.0, 0.8)
    walls = Geometry()
    for lo, hi in [
        ((-HALF - 1, 0, -HALF - 1), (HALF + 1, 1.0, -HALF)),
        ((-HALF - 1, 0, HALF), (HALF + 1, 1.0, HALF + 1)),
        ((-HALF - 1, 0, -HALF), (-HALF, 1.0, HALF)),
        ((HALF, 0, -HALF), (HALF + 1, 1.0, HALF)),
        ((-6.0, 0, 40.0), (6.0, 0.6, 41.0)),
    ]:
        walls.box(lo, hi)

    glb = Glb()
    visual_parts = []
    for (surface, shade), geometry in sorted(visual_ground.items()):
        r, g, b = SURFACE_COLORS[surface]
        k = 1.0 if shade == "a" else 0.88
        visual_parts.append((geometry, f"{surface}Tile{shade}", (r * k, g * k, b * k)))
    visual_parts.append((ramps, "RampPaint", SURFACE_COLORS["Wood"]))
    visual_parts.append((walls, "WallPaint", SURFACE_COLORS["Stone"]))
    visual = glb.node("Visual", children=[glb.node("Ground", glb.mesh(visual_parts))])

    collision_parts = [(geometry, surface, (1, 1, 1)) for surface, geometry in sorted(collision_ground.items())]
    collision_parts.append((ramps, "Wood", (1, 1, 1)))
    collision_parts.append((walls, "Stone", (1, 1, 1)))
    collision = glb.node("Collision", children=[glb.node("CollisionMesh", glb.mesh(collision_parts))])
    glb.write(folder / "visual.glb", [visual, collision])

    (folder / "track.toml").write_text(
        'id = "revvy_arena"\nname = "Revvy Arena"\nauthor = "Revvy"\nlaps = 3\nformat = "revvy-glb-v1"\n'
    )
    slots = "\n".join(
        f"        (pos: (x: {x:.2f}, y: 0.3, z: {z:.2f}), yaw: 0.0),"
        for x, z in [(0.0, -40.0), (-1.3, -40.2), (0.0, -41.5), (-1.3, -41.7)]
    )
    (folder / "layout.ron").write_text(
        "// Grilla de largada de la arena de prueba. El resto del layout llega con el editor.\n"
        "TrackLayout(\n    version: 1,\n    start_grid: [\n" + slots + "\n    ],\n)\n"
    )


if __name__ == "__main__":
    buggy()
    arena()
    print("contenido de prueba generado")
