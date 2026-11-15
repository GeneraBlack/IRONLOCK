"""
Headless Blender 5.3 CLI Runner, Manifest Verifier & 3-Sheet Renderer (`w2_veh_ground/cli.py`).

Usage:
    blender.exe --background --factory-startup --python .../w2_veh_ground/cli.py -- [--only a,b] [--sheets-only] [--no-sheets]
"""

from __future__ import annotations

import argparse
import os
import sys
import time
from typing import Dict, List, Tuple

THIS_DIR = os.path.dirname(os.path.abspath(__file__))
TOOLS_DIR = os.path.abspath(os.path.join(THIS_DIR, "..", ".."))
REPO_ROOT = os.path.abspath(os.path.join(TOOLS_DIR, ".."))
if TOOLS_DIR not in sys.path:
    sys.path.insert(0, TOOLS_DIR)

from blender_ra2_factory import load_wave2_manifest
from blender_ra2_factory.glb_postprocess import parse_glb, verify_baked_glb
from blender_ra2_factory.verify_sheet import SheetAssetSpec, render_glb_comparison_sheet
from blender_ra2_factory.w2_veh_ground import VEH_GROUND_BUILDERS


SHEET_GROUPS: List[Tuple[str, str, List[Tuple[str, str, str, str]]]] = [
    (
        "veh_ground_1.png",
        "RUST-RA2 WAVE-2 GROUND VEHICLES SHEET 1/3 - ALLIED ARMOR & SUPPORT",
        [
            ("prism_tank", "ALLIED PRISM TANK", "allied", "2.02x3.15m | Tracked Chassis, Reflector Dish & Glowing Prism Crystal Yoke"),
            ("mirage_tank", "ALLIED MIRAGE TANK", "allied", "2.00x3.05m | Stealth-Faceted Hull, Crescent Holo-Shrouds & Thermal Cannon"),
            ("ifv", "ALLIED MULTIPURPOSE IFV", "allied", "1.84x2.90m | Fast 6x6 Wheeled V-Hull & Dual 6-Cell Hover-SAM Turret"),
            ("robot_tank", "ALLIED ROBOT TANK", "allied", "1.76x2.58m | Tri-Pod Ducted Hover Drone, Cyan Optic Visor & Twin Plasma Guns"),
            ("battle_fortress", "ALLIED BATTLE FORTRESS", "allied", "2.40x3.72m | Heavy Super-APC, Crusher Prow, Firing Ports & MG Cupola"),
            ("tank_destroyer", "GERMAN TANK DESTROYER", "allied", "2.12x3.22m | Low-Profile Casemate Jagdpanzer, Side Skirts & 120mm AT Gun"),
            ("chrono_miner", "ALLIED CHRONO MINER", "allied", "2.14x3.36m | Front Ore Scoop Cage, Glowing Chrono-Sphere Core & Dump Hopper"),
            ("allied_mcv", "ALLIED MCV", "allied", "2.34x3.65m | Heavy 6x6 Tractor Cab, Modular Hangar Pod & Yellow Gantry Crane"),
            ("allied_lcv", "ALLIED HOVER LCV", "allied", "2.18x3.55m | Inflatable Rubber Hover Skirt, Vehicle Deck & Twin Ducted Fans"),
        ],
    ),
    (
        "veh_ground_2.png",
        "RUST-RA2 WAVE-2 GROUND VEHICLES SHEET 2/3 - SOVIET HEAVY ARMOR & DRONES",
        [
            ("rhino_tank", "SOVIET RHINO HEAVY TANK", "soviet", "2.16x3.34m | 6-Wheel Tracks, Glacis Nose Horn, Cast Turret, Fuel Drums & 120mm"),
            ("tesla_tank", "SOVIET TESLA TANK", "soviet", "2.04x3.10m | Tracked Generator Hull & Twin Chrome/Copper Tesla Arc Coils"),
            ("flak_track", "SOVIET FLAK TRACK", "soviet", "1.86x2.92m | Steered Front Tires + Rear Half-Tracks & Twin 23mm AA Flak Mount"),
            ("v3_rocket_launcher", "SOVIET V3 ROCKET LAUNCHER", "soviet", "2.06x3.64m | 8x8 MAZ-543 Split-Cab TEL & Finned Crimson/White V3 Missile"),
            ("demolition_truck", "LIBYAN DEMOLITION TRUCK", "soviet", "1.86x3.05m | 6x6 Cab-Over Cargo Truck & Glowing Rad-Green Nuclear Drum"),
            ("war_miner", "SOVIET WAR MINER", "soviet", "2.20x3.48m | Spiked Rotary Mining Drum, Ore Hopper & Roof 20mm Cannon Turret"),
            ("soviet_mcv", "SOVIET MCV", "soviet", "2.36x3.68m | Quad-Track Rig, Crimson Cab, Golden Onion Dome & Claw Crane"),
            ("soviet_sub_apc", "SOVIET AMPHIBIOUS APC", "soviet", "2.05x3.35m | Rubber Hover Skirt, Enclosed Olive/Crimson APC & Twin Prop Fans"),
            ("soviet_zubr_transport", "SOVIET ZUBR HOVER TRANSPORT", "soviet", "2.32x3.65m | Heavy Naval Hovercraft, Bow Ramp, Radar Island & Shrouded Fans"),
            ("terror_drone", "SOVIET TERROR DRONE", "soviet", "1.28x1.42m | 4-Legged Cybernetic Spider Drone, Red Optic & Animated Claws"),
        ],
    ),
    (
        "veh_ground_3.png",
        "RUST-RA2 WAVE-2 GROUND VEHICLES SHEET 3/3 - YURI PSIONIC & SPECIAL",
        [
            ("lasher_tank", "YURI LASHER LIGHT TANK", "yuri", "2.02x3.06m | Bio-Ribbed Purple/Bronze Skirts, Scythe Prow & Sloped Turret"),
            ("gatling_tank", "YURI GATLING TANK", "yuri", "1.96x2.94m | Sculpted Combat Wheels & Dual 6-Barrel Rotary Gatling Yoke"),
            ("gattling_tank", "YURI GATTLING TANK (ALIAS)", "yuri", "1.96x2.94m | Sculpted Combat Wheels & Dual 6-Barrel Rotary Gatling Yoke"),
            ("chaos_drone", "YURI CHAOS DRONE", "yuri", "1.52x1.72m | Quad-Wheel Spider Stance, 8 Gas Nozzles & Bio-Green Glass Dome"),
            ("magnetron", "YURI MAGNETRON", "yuri", "2.04x3.26m | 6-Wheel Beetle Carapace & Curved Bronze Tuning-Fork Emitter"),
            ("mastermind", "YURI MASTERMIND", "yuri", "2.14x3.32m | Bronze Grill Prow & Giant Fluid-Suspended Brain in Glass Dome"),
            ("yuri_slave_miner", "YURI SLAVE MINER", "yuri", "2.24x3.52m | Tracked Mobile Smelter, Molten Furnace Maw, Stacks & Gatling"),
            ("yuri_mcv", "YURI MCV", "yuri", "2.32x3.60m | Pyramidal Bio-Ribbed Spearhead Hull, Green Pods & Bronze Crane"),
            ("yuri_hover_transport", "YURI HOVER TRANSPORT", "yuri", "2.22x3.48m | Rubber Hover Skirt, Purple/Bronze Assault Hull & Twin Thrusters"),
            ("crashed_ufo", "CRASHED FLYING SAUCER", "yuri", "3.30x3.30m | Tilted Metallic Saucer, Breached Girder Notch & Cyan Core Dome"),
        ],
    ),
]


def verify_against_manifest_entry(glb_path: str, entry: dict) -> None:
    """Strictly asserts hierarchy, pivots, sockets, animations, budget & team mask against `wave2_manifest.json`."""
    summary = verify_baked_glb(glb_path, max_lod0_verts=5800)
    assert summary.tex_size == int(entry["tex_size"]), f"{glb_path}: tex_size {summary.tex_size} != {entry['tex_size']}"
    if entry.get("team_color_tint", True):
        assert summary.team_alpha0_pct > 0.05, f"{glb_path}: expected non-zero team_alpha0_pct, got {summary.team_alpha0_pct}%"

    gltf, _ = parse_glb(open(glb_path, "rb").read())
    nodes = gltf.get("nodes", [])
    lod0_meshes = [n["name"] for n in nodes if "mesh" in n and not n.get("name", "").endswith("_LOD1")]
    sockets = [n["name"] for n in nodes if "mesh" not in n]

    exp_meshes = list(entry["existing_mesh_nodes"])
    exp_sockets = list(entry["existing_socket_nodes"])
    exp_anims = list(entry.get("existing_animations", []))

    assert lod0_meshes[0] == "Hull", f"{glb_path}: nodes[0] must be Hull, got {lod0_meshes}"
    assert set(lod0_meshes) == set(exp_meshes) and len(lod0_meshes) == len(exp_meshes), (
        f"{glb_path}: mesh nodes {lod0_meshes} != manifest {exp_meshes}"
    )
    assert set(sockets) == set(exp_sockets) and len(sockets) == len(exp_sockets), (
        f"{glb_path}: socket nodes {sockets} != manifest {exp_sockets}"
    )
    assert list(summary.animation_clips) == exp_anims, (
        f"{glb_path}: animations {summary.animation_clips} != manifest {exp_anims}"
    )

    by_name: Dict[str, Tuple[int, dict]] = {n["name"]: (i, n) for i, n in enumerate(nodes) if "name" in n}
    if "Turret" in by_name:
        ti, tnd = by_name["Turret"]
        hull_children = nodes[by_name["Hull"][0]].get("children", [])
        assert ti in hull_children, f"{glb_path}: Turret must be direct child of Hull"
        tr = tnd.get("translation", [0.0, 0.0, 0.0])
        assert abs(float(tr[0])) < 1e-4, f"{glb_path}: Turret x pivot != 0: {tr}"

    if "Barrel" in by_name:
        bi, bnd = by_name["Barrel"]
        tur_children = nodes[by_name["Turret"][0]].get("children", [])
        assert bi in tur_children, f"{glb_path}: Barrel must be direct child of Turret"
        btr = bnd.get("translation", [0.0, 0.0, 0.0])
        assert abs(float(btr[0])) < 1e-4 and abs(float(btr[2])) < 1e-4, f"{glb_path}: Barrel x/z pivot != 0: {btr}"


def main() -> None:
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    parser = argparse.ArgumentParser(description="Bake & verify Wave-2 Ground Vehicles (29 .glbs + 3 sheets)")
    parser.add_argument("--only", type=str, default="", help="Comma-separated stems to bake")
    parser.add_argument("--sheets-only", action="store_true", help="Skip baking and render comparison sheets only")
    parser.add_argument("--no-sheets", action="store_true", help="Skip rendering comparison sheets")
    parser.add_argument("--sheet-index", type=int, default=0, help="Render only sheet 1, 2, or 3 (0 = all)")
    args = parser.parse_args(argv)

    manifest = load_wave2_manifest()
    entries = {e["stem"]: e for e in manifest["assets"] if e["bucket"] == "veh_ground"}
    assert len(entries) == 29, f"Expected 29 veh_ground entries, got {len(entries)}"
    assert set(entries.keys()) == set(VEH_GROUND_BUILDERS.keys()), (
        f"Builder mismatch: missing={set(entries) - set(VEH_GROUND_BUILDERS)} extra={set(VEH_GROUND_BUILDERS) - set(entries)}"
    )

    ra2_dir = os.path.join(REPO_ROOT, "rust-ra2")
    models_dir = os.path.join(ra2_dir, "assets", "models")
    sheets_dir = os.path.join(REPO_ROOT, "docs", "model_sheets")

    if not args.sheets_only:
        target_stems = [s.strip() for s in args.only.split(",") if s.strip()] if args.only else sorted(entries.keys())
        t0 = time.time()
        for idx, stem in enumerate(target_stems, 1):
            entry = entries[stem]
            out_glb = os.path.join(models_dir, entry["rel_path"])
            ts = time.time()
            summary = VEH_GROUND_BUILDERS[stem](out_glb, int(entry["tex_size"]))
            verify_against_manifest_entry(out_glb, entry)
            dt = time.time() - ts
            print(
                f"[w2_veh_ground {idx:02d}/{len(target_stems):02d}] OK {stem}.glb ({summary.file_kb}KB, {dt:.1f}s) | "
                f"LOD0={summary.lod0_verts}v LOD1={summary.lod1_verts}v tex={summary.tex_size}px "
                f"a0={summary.team_alpha0_pct}% bounds={summary.bounds_min}..{summary.bounds_max}"
            )
        print(f"[w2_veh_ground] Baked & verified {len(target_stems)} ground vehicles in {time.time() - t0:.1f}s!")

    if not args.no_sheets:
        for s_idx, (fname, title, rows) in enumerate(SHEET_GROUPS, 1):
            if args.sheet_index not in (0, s_idx):
                continue
            specs = [
                SheetAssetSpec(
                    asset_id=aid,
                    display_title=dtitle,
                    faction=fac,
                    subtitle=sub,
                    hero_yaw_deg=56.0,
                    hero_pitch_deg=20.0,
                    hero_turret_yaw_deg=-28.0,
                    hero_barrel_pitch_deg=5.0,
                )
                for (aid, dtitle, fac, sub) in rows
            ]
            out_sheet = os.path.join(sheets_dir, fname)
            render_glb_comparison_sheet(ra2_dir, models_dir, out_sheet, specs, sheet_title=title)


if __name__ == "__main__":
    main()
