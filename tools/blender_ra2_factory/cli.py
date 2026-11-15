"""
Headless Blender 5.3 CLI Entrypoint for `tools/blender_ra2_factory` (`cli.py`).

Example invocation (always set `TMP`/`TEMP` on `D:` first!):
    blender.exe --background --factory-startup --python tools/blender_ra2_factory/cli.py -- --pilot-6
"""

from __future__ import annotations

import argparse
import os
import sys
import time

# Ensure `<repo_root>/tools` is importable when launched via `blender.exe --python`
_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
_TOOLS_DIR = os.path.dirname(_THIS_DIR)
_REPO_ROOT = os.path.dirname(_TOOLS_DIR)
if _TOOLS_DIR not in sys.path:
    sys.path.insert(0, _TOOLS_DIR)

from blender_ra2_factory.pilot_models import PILOT_BUILDERS  # noqa: E402
from blender_ra2_factory.shp_vxl_blueprint import inspect_sprite_blueprint, inspect_vxl_blueprint  # noqa: E402
from blender_ra2_factory.verify_sheet import PILOT_6_SPECS, render_glb_comparison_sheet  # noqa: E402


def _parse_blender_cli_args() -> argparse.Namespace:
    argv = sys.argv
    user_args = argv[argv.index("--") + 1 :] if "--" in argv else []
    parser = argparse.ArgumentParser(description="rust-ra2 Headless Blender 5.3 + Cycles HIP GPU Asset Factory")
    parser.add_argument("--repo-root", default=_REPO_ROOT, help="Repository / worktree root path")
    parser.add_argument("--pilot-6", action="store_true", help="Build all 6 Pilot .glb assets and render pilot_6_vs_sprites.png")
    parser.add_argument("--only", default="", help="Comma-separated subset of pilot asset IDs to build")
    parser.add_argument("--sheet-only", action="store_true", help="Skip .glb rebuild and only render pilot_6_vs_sprites.png")
    parser.add_argument("--no-sheet", action="store_true", help="Skip rendering pilot_6_vs_sprites.png")
    return parser.parse_args(user_args)


def main() -> None:
    args = _parse_blender_cli_args()
    repo_root = os.path.abspath(args.repo_root)
    ra2_dir = os.path.join(repo_root, "rust-ra2")
    models_dir = os.path.join(ra2_dir, "assets", "models")
    sheet_path = os.path.join(repo_root, "docs", "model_sheets", "pilot_6_vs_sprites.png")

    selected_ids = (
        [s.strip() for s in args.only.split(",") if s.strip()]
        if args.only
        else list(PILOT_BUILDERS.keys())
    )

    t0 = time.perf_counter()
    print(f"[blender_ra2_factory] Repo root: {repo_root}")

    if not args.sheet_only:
        for asset_id in selected_ids:
            if asset_id not in PILOT_BUILDERS:
                raise ValueError(f"Unknown pilot asset_id: {asset_id}")
            sbp = inspect_sprite_blueprint(ra2_dir, asset_id)
            if sbp is not None:
                print(
                    f"[blueprint] {asset_id}: footprint={sbp.toml_footprint} world={sbp.world_size_xz}m "
                    f"est_h={sbp.estimated_height_m}m team_frac={sbp.team_pixel_fraction}"
                )
            else:
                vbp = inspect_vxl_blueprint(ra2_dir, asset_id)
                if vbp is not None:
                    print(
                        f"[blueprint] {asset_id}: target={vbp.target_width_x}x{vbp.target_length_z}m "
                        f"est_h={vbp.estimated_height_y}m tur_y={vbp.turret_pivot_y}m dual={vbp.dual_muzzle}"
                    )

            t_m = time.perf_counter()
            out_glb = os.path.join(models_dir, f"{asset_id}.glb")
            s = PILOT_BUILDERS[asset_id](out_glb)
            dt_m = time.perf_counter() - t_m
            print(
                f"[built] {asset_id}.glb in {dt_m:.2f}s ({s.file_kb} KB) | "
                f"LOD0={s.lod0_verts}v LOD1={s.lod1_verts}v | "
                f"bounds=({s.bounds_min[0]:.2f},{s.bounds_min[1]:.4f},{s.bounds_min[2]:.2f}).."
                f"({s.bounds_max[0]:.2f},{s.bounds_max[1]:.2f},{s.bounds_max[2]:.2f}) | "
                f"nrm_std={s.normal_std_xy} ao={s.ao_min_max} rough={s.rough_min_max} "
                f"metal={s.metal_min_max} a0={s.team_alpha0_pct}%"
            )

    if (args.pilot_6 or args.sheet_only) and not args.no_sheet:
        specs = [sp for sp in PILOT_6_SPECS if sp.asset_id in selected_ids]
        render_glb_comparison_sheet(ra2_dir, models_dir, sheet_path, specs)

    print(f"[blender_ra2_factory] Finished in {time.perf_counter() - t0:.2f}s")


if __name__ == "__main__":
    main()
