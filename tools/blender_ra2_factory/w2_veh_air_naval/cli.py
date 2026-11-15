"""
Headless Blender 5.3 CLI Runner & Manifest Verifier for Wave-2 `veh_air_naval` (`w2_veh_air_naval/cli.py`).

Builds all 24 Aircraft, Helicopters, Warships, Submarines, Aquatic Units & Projectiles
in `bucket == "veh_air_naval"`, verifies 100% of every `wave2_manifest.json` contract, and renders:
  - `docs/model_sheets/veh_air_naval_1.png` (12 models: 10 Aircraft/Choppers + 2 Aquatics)
  - `docs/model_sheets/veh_air_naval_2.png` (12 models: 7 Naval Warships/Subs + 5 Projectiles)
"""

from __future__ import annotations

import argparse
import os
import sys
import time
from typing import Any, Callable, Dict, List

_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
_PKG_DIR = os.path.dirname(_THIS_DIR)
_TOOLS_DIR = os.path.dirname(_PKG_DIR)
_REPO_ROOT = os.path.dirname(_TOOLS_DIR)
if _TOOLS_DIR not in sys.path:
    sys.path.insert(0, _TOOLS_DIR)

from blender_ra2_factory import load_wave2_manifest  # noqa: E402
from blender_ra2_factory.glb_postprocess import GlbVerificationSummary, parse_glb, verify_baked_glb  # noqa: E402
from blender_ra2_factory.verify_sheet import SheetAssetSpec, render_glb_comparison_sheet  # noqa: E402
from blender_ra2_factory.w2_veh_air_naval.aircraft import AIRCRAFT_BUILDERS  # noqa: E402
from blender_ra2_factory.w2_veh_air_naval.aquatics_projectiles import AQUATIC_PROJECTILE_BUILDERS  # noqa: E402
from blender_ra2_factory.w2_veh_air_naval.naval_ships import NAVAL_SHIP_BUILDERS  # noqa: E402

VEH_AIR_NAVAL_BUILDERS: Dict[str, Callable[[str], GlbVerificationSummary]] = {
    **AIRCRAFT_BUILDERS,
    **NAVAL_SHIP_BUILDERS,
    **AQUATIC_PROJECTILE_BUILDERS,
}

# ==============================================================================
# Sheet 1 (12 Models): 10 Aircraft / Helicopters + 2 Sculpted Aquatic Units
# ==============================================================================

SHEET_1_SPECS: List[SheetAssetSpec] = [
    SheetAssetSpec(
        asset_id="kirov_airship",
        display_title="SOVIET KIROV AIRSHIP",
        faction="soviet",
        subtitle="2.42X5.80M | RIBBED ZEPPELIN, SHARK GRIN & GLOWING EYES, GONDOLA, X-TAIL & TWIN PROPS",
        hero_yaw_deg=36.0,
        hero_pitch_deg=18.0,
    ),
    SheetAssetSpec(
        asset_id="harrier",
        display_title="ALLIED HARRIER VTOL STRIKE JET",
        faction="allied",
        subtitle="2.55X2.95M | ANHEDRAL SWEPT WINGS, CHEEK INTAKES, BUBBLE CANOPY & 4 MAVERICK MISSILES",
        hero_yaw_deg=42.0,
        hero_pitch_deg=22.0,
    ),
    SheetAssetSpec(
        asset_id="black_eagle",
        display_title="KOREAN BLACK EAGLE STEALTH FIGHTER",
        faction="allied",
        subtitle="2.65X3.05M | FORWARD-SWEPT WINGS, FOREPLANE CANARDS, CANTED TWIN TAILS & DUAL STRIKE PODS",
        hero_yaw_deg=40.0,
        hero_pitch_deg=22.0,
    ),
    SheetAssetSpec(
        asset_id="hornet_fighter",
        display_title="ALLIED HORNET CARRIER INTERCEPTOR",
        faction="allied",
        subtitle="2.10X2.42M | CARRIER SWEPT WINGS, LEX STRAKES, TWIN CANTED TAILS & WINGTIP MISSILES",
        hero_yaw_deg=40.0,
        hero_pitch_deg=22.0,
    ),
    SheetAssetSpec(
        asset_id="spy_plane",
        display_title="SOVIET SPY RECONNAISSANCE PLANE",
        faction="soviet",
        subtitle="3.20X2.80M | HIGH RECON WINGS, TWIN TURBOPROP NACELLES, T-TAIL & BELLY OPTIC TURRET",
        hero_yaw_deg=38.0,
        hero_pitch_deg=22.0,
    ),
    SheetAssetSpec(
        asset_id="paradrop_plane",
        display_title="HEAVY PARADROP CARGO TRANSPORT PLANE",
        faction="allied",
        subtitle="4.10X3.85M | 4 TURBOPROP ENGINES, HIGH CARGO WINGS, FLIGHT DECK GLASS & REAR DROP RAMP",
        hero_yaw_deg=38.0,
        hero_pitch_deg=22.0,
    ),
    SheetAssetSpec(
        asset_id="nighthawk_chopper",
        display_title="ALLIED NIGHT HAWK STEALTH HELICOPTER",
        faction="allied",
        subtitle="2.05X3.38M | FACETED STEALTH HULL, 5-BLADE ROTOR, FENESTRON TAIL & CHIN FLIR SENSOR",
        hero_yaw_deg=46.0,
        hero_pitch_deg=20.0,
    ),
    SheetAssetSpec(
        asset_id="siege_chopper",
        display_title="SOVIET SIEGE ARTILLERY CHOPPER",
        faction="soviet",
        subtitle="2.02X3.32M | COAXIAL ROTOR, TANDEM GLASS COCKPIT, ROCKET PODS & UNDERSLUNG 130MM CANNON",
        hero_yaw_deg=48.0,
        hero_pitch_deg=19.0,
        hero_turret_yaw_deg=-18.0,
        hero_barrel_pitch_deg=5.0,
    ),
    SheetAssetSpec(
        asset_id="osprey_asw",
        display_title="ALLIED OSPREY ASW NAVAL HELICOPTER",
        faction="allied",
        subtitle="2.45X2.85M | NOSE SEARCH RADAR DOME, DIPPING SONAR, MK-46 TORPEDO PYLONS & H-TAIL ROTOR",
        hero_yaw_deg=42.0,
        hero_pitch_deg=22.0,
    ),
    SheetAssetSpec(
        asset_id="floating_disc",
        display_title="YURI FLOATING DISC SAUCER",
        faction="yuri",
        subtitle="2.85X2.85M | SEGMENTED BRONZE/PURPLE ROTOR RIM, PSYCHIC BRAIN CUPOLA & VENTRAL LASER CORE",
        hero_yaw_deg=32.0,
        hero_pitch_deg=24.0,
        hero_turret_yaw_deg=25.0,
    ),
    SheetAssetSpec(
        asset_id="dolphin",
        display_title="ALLIED SONIC DOLPHIN",
        faction="allied",
        subtitle="1.22X2.42M | SCULPTED CETACEAN BODY, DORSAL/PECTORAL/FLUKES & STRAPPED SONIC TRANSDUCER",
        hero_yaw_deg=54.0,
        hero_pitch_deg=19.0,
    ),
    SheetAssetSpec(
        asset_id="giant_squid",
        display_title="SOVIET GIANT SQUID",
        faction="soviet",
        subtitle="1.28X3.35M | ARROW-FINNED MANTLE, NEURAL COLLAR, GLOWING EYES & 8 CURVING TENTACLES",
        hero_yaw_deg=48.0,
        hero_pitch_deg=21.0,
    ),
]

# ==============================================================================
# Sheet 2 (12 Models): 7 Naval Warships & Submarines + 5 Heavy Projectiles/Bombs
# ==============================================================================

SHEET_2_SPECS: List[SheetAssetSpec] = [
    SheetAssetSpec(
        asset_id="allied_destroyer",
        display_title="ALLIED NAVAL DESTROYER",
        faction="allied",
        subtitle="2.15X5.15M | FLARED NATO HULL, BRIDGE RADAR MAST, ASW LAUNCHERS, HELIPAD & 127MM GUN",
        hero_yaw_deg=42.0,
        hero_pitch_deg=20.0,
        hero_turret_yaw_deg=-22.0,
        hero_barrel_pitch_deg=8.0,
    ),
    SheetAssetSpec(
        asset_id="aegis_cruiser",
        display_title="ALLIED AEGIS GUIDED-MISSILE CRUISER",
        faction="allied",
        subtitle="2.20X5.20M | OCTAGONAL SPY-1 PHASED-ARRAY SUPERSTRUCTURE, VLS CELLS & TWIN SAM BATTERY",
        hero_yaw_deg=40.0,
        hero_pitch_deg=21.0,
        hero_turret_yaw_deg=-20.0,
    ),
    SheetAssetSpec(
        asset_id="aircraft_carrier",
        display_title="ALLIED NUCLEAR AIRCRAFT CARRIER",
        faction="allied",
        subtitle="2.45X5.55M | ANGLED TARMAC FLIGHT DECK, STARBOARD ISLAND, BLAST DEFLECTORS & 2 HORNETS",
        hero_yaw_deg=38.0,
        hero_pitch_deg=22.0,
    ),
    SheetAssetSpec(
        asset_id="sea_scorpion",
        display_title="SOVIET SEA SCORPION HYDROFOIL",
        faction="soviet",
        subtitle="1.92X4.20M | BOW/STERN HYDROFOIL WINGS, CRIMSON ARMOR BELT, BRIDGE & NAVAL FLAK CANNON",
        hero_yaw_deg=46.0,
        hero_pitch_deg=20.0,
        hero_turret_yaw_deg=18.0,
        hero_barrel_pitch_deg=6.0,
    ),
    SheetAssetSpec(
        asset_id="dreadnought",
        display_title="SOVIET DREADNOUGHT MISSILE BATTLESHIP",
        faction="soviet",
        subtitle="2.28X5.42M | ARMORED FOREDECK, TWIN CRUISE MISSILE CRADLES, PAGODA MAST & TWIN STACKS",
        hero_yaw_deg=40.0,
        hero_pitch_deg=21.0,
        hero_turret_yaw_deg=-15.0,
    ),
    SheetAssetSpec(
        asset_id="typhoon_sub",
        display_title="SOVIET TYPHOON ATTACK SUBMARINE",
        faction="soviet",
        subtitle="1.82X5.05M | WIDE TEARDROP HULL, 10 SLBM HATCHES, CONNING SAIL, DIVING PLANES & PUMP-JET",
        hero_yaw_deg=44.0,
        hero_pitch_deg=20.0,
    ),
    SheetAssetSpec(
        asset_id="boomer_sub",
        display_title="YURI BOOMER STRATEGIC SUBMARINE",
        faction="yuri",
        subtitle="1.96X5.28M | BRONZE/PURPLE MANTA HULL, 8 MISSILE SPINE SILOS, PSYCHIC SAIL & TORPEDOES",
        hero_yaw_deg=42.0,
        hero_pitch_deg=21.0,
    ),
    SheetAssetSpec(
        asset_id="v3_rocket",
        display_title="SOVIET V3 BALLISTIC ROCKET",
        faction="soviet",
        subtitle="0.62X2.15M | LATHE OGIVE WARHEAD, HAZARD COLLAR, CRUCIFORM DELTA FINS & ROCKET BELL",
        hero_yaw_deg=52.0,
        hero_pitch_deg=20.0,
    ),
    SheetAssetSpec(
        asset_id="dreadnought_missile",
        display_title="SOVIET DREADNOUGHT CRUISE MISSILE",
        faction="soviet",
        subtitle="0.60X2.05M | SUPERSONIC OGIVE NOSE, VENTRAL RAMJET SCOOP, STUB WINGS & SWEPT TAIL FINS",
        hero_yaw_deg=52.0,
        hero_pitch_deg=20.0,
    ),
    SheetAssetSpec(
        asset_id="boomer_cruise_missile",
        display_title="YURI BOOMER CRUISE MISSILE",
        faction="yuri",
        subtitle="0.52X1.85M | PURPLE/BRONZE WARHEAD, PSYCHIC SEEKER TIP, GULL WINGS & PLASMA THRUSTER",
        hero_yaw_deg=52.0,
        hero_pitch_deg=20.0,
    ),
    SheetAssetSpec(
        asset_id="kirov_bomb",
        display_title="SOVIET KIROV SHARK DEMOLITION BOMB",
        faction="soviet",
        subtitle="0.68X1.40M | HEAVY CAST CASING, NOSE VANE FUZE, SUSPENSION LUGS & ANNULAR RING FINS",
        hero_yaw_deg=50.0,
        hero_pitch_deg=22.0,
    ),
    SheetAssetSpec(
        asset_id="nuclear_icbm",
        display_title="SOVIET THERMONUCLEAR ICBM",
        faction="soviet",
        subtitle="0.92X3.15M | MULTI-STAGE CASING, CRIMSON MIRV CONE, RAD PORTS & 5-BELL BOOSTER CLUSTER",
        hero_yaw_deg=48.0,
        hero_pitch_deg=20.0,
    ),
]


def verify_against_wave2_manifest(glb_path: str, entry: Dict[str, Any]) -> GlbVerificationSummary:
    """Asserts 100% agreement between `glb_path` and its `wave2_manifest.json` contract."""
    stem = entry["stem"]
    summary = verify_baked_glb(glb_path, max_lod0_verts=5800)
    gltf, _ = parse_glb(open(glb_path, "rb").read())

    mesh_nodes = sorted(
        n["name"]
        for n in gltf.get("nodes", [])
        if "mesh" in n and not n.get("name", "").endswith("_LOD1")
    )
    socket_nodes = sorted(
        n["name"]
        for n in gltf.get("nodes", [])
        if "mesh" not in n and n.get("name", "") in ("Muzzle_01", "Muzzle_02", "Exhaust_01", "DamageSmoke_01")
    )

    exp_meshes = sorted(entry["existing_mesh_nodes"])
    exp_sockets = sorted(entry["existing_socket_nodes"])
    if summary.tex_size != entry["tex_size"]:
        raise AssertionError(f"{stem}: tex_size={summary.tex_size} != expected {entry['tex_size']}")
    if mesh_nodes != exp_meshes:
        raise AssertionError(f"{stem}: mesh_nodes={mesh_nodes} != expected {exp_meshes}")
    if socket_nodes != exp_sockets:
        raise AssertionError(f"{stem}: socket_nodes={socket_nodes} != expected {exp_sockets}")
    if summary.team_alpha0_pct <= 0.0:
        raise AssertionError(f"{stem}: expected non-zero Alpha=0 team mask, got {summary.team_alpha0_pct}%")
    if abs(summary.bounds_min[1]) > 1e-3:
        raise AssertionError(f"{stem}: expected min_y == 0.0, got {summary.bounds_min[1]:.5f}")
    return summary


def _parse_args() -> argparse.Namespace:
    argv = sys.argv
    user_args = argv[argv.index("--") + 1 :] if "--" in argv else []
    parser = argparse.ArgumentParser(description="Wave-2 veh_air_naval Headless Blender 5.3 + Cycles HIP GPU Builder")
    parser.add_argument("--repo-root", default=_REPO_ROOT, help="Isolated worktree root")
    parser.add_argument("--all", action="store_true", help="Build all 24 GLBs and render both 12-model sheets")
    parser.add_argument("--only", default="", help="Comma-separated subset of asset stems to build")
    parser.add_argument("--sheets-only", action="store_true", help="Skip GLB baking and only render sheets")
    parser.add_argument("--no-sheets", action="store_true", help="Skip rendering comparison sheets")
    return parser.parse_args(user_args)


def main() -> None:
    args = _parse_args()
    repo_root = os.path.abspath(args.repo_root)
    ra2_dir = os.path.join(repo_root, "rust-ra2")
    models_dir = os.path.join(ra2_dir, "assets", "models")
    sheets_dir = os.path.join(repo_root, "docs", "model_sheets")

    manifest = load_wave2_manifest()
    bucket_entries = {a["stem"]: a for a in manifest["assets"] if a["bucket"] == "veh_air_naval"}
    if set(bucket_entries.keys()) != set(VEH_AIR_NAVAL_BUILDERS.keys()):
        missing = set(bucket_entries.keys()) - set(VEH_AIR_NAVAL_BUILDERS.keys())
        extra = set(VEH_AIR_NAVAL_BUILDERS.keys()) - set(bucket_entries.keys())
        raise RuntimeError(f"Builder mismatch vs wave2_manifest: missing={missing} extra={extra}")

    selected = [s.strip() for s in args.only.split(",") if s.strip()] if args.only else sorted(bucket_entries.keys())
    t0 = time.perf_counter()
    print(f"[w2_veh_air_naval] Worktree root: {repo_root} | Selected: {len(selected)} assets")

    if not args.sheets_only:
        for idx, stem in enumerate(selected, 1):
            if stem not in VEH_AIR_NAVAL_BUILDERS:
                raise ValueError(f"Unknown veh_air_naval stem: {stem}")
            t_m = time.perf_counter()
            out_glb = os.path.join(models_dir, f"{stem}.glb")
            VEH_AIR_NAVAL_BUILDERS[stem](out_glb)
            s = verify_against_wave2_manifest(out_glb, bucket_entries[stem])
            dt_m = time.perf_counter() - t_m
            print(
                f"[{idx:02d}/{len(selected):02d}] {stem}.glb ({dt_m:.2f}s, {s.file_kb} KB) | "
                f"LOD0={s.lod0_verts}v LOD1={s.lod1_verts}v TEX={s.tex_size}px | "
                f"bounds=({s.bounds_min[0]:.2f},{s.bounds_min[1]:.4f},{s.bounds_min[2]:.2f}).."
                f"({s.bounds_max[0]:.2f},{s.bounds_max[1]:.2f},{s.bounds_max[2]:.2f}) | "
                f"nrm={s.normal_std_xy} ao={s.ao_min_max} rough={s.rough_min_max} a0={s.team_alpha0_pct}%"
            )

    if (args.all or args.sheets_only) and not args.no_sheets:
        s1_specs = [sp for sp in SHEET_1_SPECS if sp.asset_id in selected] if args.only else SHEET_1_SPECS
        s2_specs = [sp for sp in SHEET_2_SPECS if sp.asset_id in selected] if args.only else SHEET_2_SPECS
        if s1_specs:
            render_glb_comparison_sheet(
                ra2_dir=ra2_dir,
                models_dir=models_dir,
                out_png_path=os.path.join(sheets_dir, "veh_air_naval_1.png"),
                asset_specs=s1_specs,
                sheet_title="WAVE-2 VEH_AIR_NAVAL SHEET 1/2: AIRCRAFT, HELICOPTERS & AQUATICS (12 MODELS)",
            )
        if s2_specs:
            render_glb_comparison_sheet(
                ra2_dir=ra2_dir,
                models_dir=models_dir,
                out_png_path=os.path.join(sheets_dir, "veh_air_naval_2.png"),
                asset_specs=s2_specs,
                sheet_title="WAVE-2 VEH_AIR_NAVAL SHEET 2/2: WARSHIPS, SUBMARINES & PROJECTILES (12 MODELS)",
            )

    print(f"[w2_veh_air_naval] Completed all tasks in {time.perf_counter() - t0:.2f}s")


if __name__ == "__main__":
    main()
