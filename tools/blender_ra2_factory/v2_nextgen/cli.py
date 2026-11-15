"""
CLI Entrypoint for Next-Gen v2 Blender 5.3 Asset Factory (`tools/blender_ra2_factory/v2_nextgen/cli.py`).

Usage inside Blender 5.3 headless:
  blender.exe -b --factory-startup -P tools/blender_ra2_factory/v2_nextgen/cli.py -- --all
  blender.exe -b --factory-startup -P tools/blender_ra2_factory/v2_nextgen/cli.py -- --only allied_conyard,grizzly_tank
  blender.exe -b --factory-startup -P tools/blender_ra2_factory/v2_nextgen/cli.py -- --render-sheet
  blender.exe -b --factory-startup -P tools/blender_ra2_factory/v2_nextgen/cli.py -- --verify-all
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
from dataclasses import asdict
from typing import Callable, Dict, List

# Ensure repository root (`d:\projekt\rust-rts`) is on `sys.path` when invoked via `blender -P`
_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
_REPO_ROOT = os.path.abspath(os.path.join(_THIS_DIR, "..", "..", ".."))
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

from tools.blender_ra2_factory.animation_clips import REQUIRED_RA2_CLIPS
from tools.blender_ra2_factory.v2_nextgen import (
    ALL_HERO_SLICE_IDS,
    BUILDING_VEHICLE_TEX_SIZE,
    INFANTRY_TEX_SIZE,
)
from tools.blender_ra2_factory.v2_nextgen.glb_finalize_v2 import (
    V2VerificationSummary,
    verify_v2_glb,
)
from tools.blender_ra2_factory.v2_nextgen.hero_allied_buildings import (
    build_v2_allied_conyard,
    build_v2_allied_power_plant,
    build_v2_allied_prism_tower,
    build_v2_allied_war_factory,
)
from tools.blender_ra2_factory.v2_nextgen.hero_soviet_buildings import (
    build_v2_soviet_barracks,
    build_v2_soviet_conyard,
    build_v2_soviet_tesla_coil,
    build_v2_soviet_tesla_reactor,
)
from tools.blender_ra2_factory.v2_nextgen.hero_vehicles import (
    build_v2_apocalypse_tank,
    build_v2_grizzly_tank,
    build_v2_kirov_airship,
)
from tools.blender_ra2_factory.v2_nextgen.render_hero_slice_sheet import (
    render_v2_hero_slice_sheet,
)
from tools.blender_ra2_factory.v2_nextgen.skinned_infantry import (
    build_v2_allied_gi,
    build_v2_soviet_conscript,
)

V2_BUILDERS: Dict[str, Callable[[str], V2VerificationSummary]] = {
    "allied_conyard": build_v2_allied_conyard,
    "allied_power_plant": build_v2_allied_power_plant,
    "allied_war_factory": build_v2_allied_war_factory,
    "allied_prism_tower": build_v2_allied_prism_tower,
    "soviet_conyard": build_v2_soviet_conyard,
    "soviet_barracks": build_v2_soviet_barracks,
    "soviet_tesla_reactor": build_v2_soviet_tesla_reactor,
    "soviet_tesla_coil": build_v2_soviet_tesla_coil,
    "grizzly_tank": build_v2_grizzly_tank,
    "apocalypse_tank": build_v2_apocalypse_tank,
    "kirov_airship": build_v2_kirov_airship,
    "allied_gi": build_v2_allied_gi,
    "soviet_conscript": build_v2_soviet_conscript,
}

BUILDING_CLIP_REQ = ("Idle", "Producing", "Constructing")


def main() -> None:
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    ap = argparse.ArgumentParser(description="Next-Gen v2 Blender 5.3 Asset Factory CLI")
    ap.add_argument("--all", action="store_true", help="Build all 13 hero .glb assets")
    ap.add_argument("--only", type=str, default="", help="Comma-separated subset of asset_ids to build")
    ap.add_argument("--verify-all", action="store_true", help="Verify all 13 hero .glb assets on disk")
    ap.add_argument("--render-sheet", action="store_true", help="Render docs/model_sheets/v2_nextgen_hero_slice.png")
    ap.add_argument(
        "--models-dir",
        type=str,
        default=os.path.join(_REPO_ROOT, "rust-ra2", "assets", "models"),
    )
    ap.add_argument(
        "--sheet-out",
        type=str,
        default=os.path.join(_REPO_ROOT, "docs", "model_sheets", "v2_nextgen_hero_slice.png"),
    )
    args = ap.parse_args(argv)

    to_build: List[str] = []
    if args.all:
        to_build = list(ALL_HERO_SLICE_IDS)
    elif args.only:
        to_build = [s.strip() for s in args.only.split(",") if s.strip()]

    for aid in to_build:
        if aid not in V2_BUILDERS:
            raise ValueError(f"Unknown v2 hero asset_id '{aid}' (valid: {list(V2_BUILDERS)})")
        t0 = time.time()
        out_p = os.path.join(args.models_dir, f"{aid}.glb")
        print(f"\n=== [v2_nextgen] Building {aid}.glb -> {out_p} ===", flush=True)
        summary = V2_BUILDERS[aid](out_p)
        dt = time.time() - t0
        print(
            f"=== [v2_nextgen] DONE {aid}.glb in {dt:.1f}s | {summary.file_kb} KB | "
            f"HP={summary.high_tris}t -> LOD0={summary.lod0_verts}v ({summary.low_tris}t) "
            f"LOD1={summary.lod1_verts}v | tex={summary.tex_size}px | nrm_std={summary.normal_std_xy} | "
            f"ao={summary.ao_min_max} | skin={summary.skin_count}({summary.joint_count}j) | "
            f"clips={summary.animation_clips} | bounds={summary.bounds_min}..{summary.bounds_max} ===",
            flush=True,
        )

    if args.verify_all:
        rows = []
        for aid in ALL_HERO_SLICE_IDS:
            p = os.path.join(args.models_dir, f"{aid}.glb")
            is_inf = aid in ("allied_gi", "soviet_conscript")
            is_bld = aid in (
                "allied_conyard",
                "allied_power_plant",
                "allied_war_factory",
                "allied_prism_tower",
                "soviet_conyard",
                "soviet_barracks",
                "soviet_tesla_reactor",
                "soviet_tesla_coil",
            )
            req_clips = REQUIRED_RA2_CLIPS if is_inf else (BUILDING_CLIP_REQ if is_bld else ())
            s = verify_v2_glb(
                p,
                expected_tex_size=INFANTRY_TEX_SIZE if is_inf else BUILDING_VEHICLE_TEX_SIZE,
                require_skin=is_inf,
                required_clips=req_clips,
            )
            rows.append(asdict(s))
            print(f"[verify-all] PASS {aid}.glb: {json.dumps(asdict(s))}", flush=True)
        print(f"[verify-all] ALL {len(rows)} V2 HERO ASSETS PASSED WITH 0 ERRORS!", flush=True)

    if args.render_sheet:
        render_v2_hero_slice_sheet(repo_root=_REPO_ROOT, out_png_path=args.sheet_out)


if __name__ == "__main__":
    main()
