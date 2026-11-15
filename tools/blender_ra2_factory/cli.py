"""
Unified Headless Blender 5.3 CLI Entrypoint for `tools/blender_ra2_factory` (`cli.py`).

Supports `pilot_6` (Wave-1) plus all 5 Wave-2 subpackages (`187` total `.glb` assets):
  - `pilot_6`
  - `w2_bld_allied_soviet`
  - `w2_bld_yuri_landmarks`
  - `w2_veh_ground`
  - `w2_veh_air_naval`
  - `w2_infantry_3d`

Example invocations (always point `TMP`/`TEMP` to `D:` first!):
    blender.exe --background --factory-startup --python tools/blender_ra2_factory/cli.py -- --all --no-sheet
    blender.exe --background --factory-startup --python tools/blender_ra2_factory/cli.py -- --bucket w2_veh_ground --only prism_tank --no-sheet
"""

from __future__ import annotations

import argparse
import glob
import os
import sys
import time
from typing import Callable, List

_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
_TOOLS_DIR = os.path.dirname(_THIS_DIR)
_REPO_ROOT = os.path.dirname(_TOOLS_DIR)
if _TOOLS_DIR not in sys.path:
    sys.path.insert(0, _TOOLS_DIR)

BUCKET_ORDER: List[str] = [
    "pilot_6",
    "w2_bld_allied_soviet",
    "w2_bld_yuri_landmarks",
    "w2_veh_ground",
    "w2_veh_air_naval",
    "w2_infantry_3d",
]

BUCKET_ALIASES = {
    "pilot": "pilot_6",
    "pilot-6": "pilot_6",
    "pilot_6": "pilot_6",
    "bld_allied_soviet": "w2_bld_allied_soviet",
    "w2_bld_allied_soviet": "w2_bld_allied_soviet",
    "bld_yuri_landmarks": "w2_bld_yuri_landmarks",
    "w2_bld_yuri_landmarks": "w2_bld_yuri_landmarks",
    "veh_ground": "w2_veh_ground",
    "w2_veh_ground": "w2_veh_ground",
    "veh_air_naval": "w2_veh_air_naval",
    "w2_veh_air_naval": "w2_veh_air_naval",
    "infantry_3d": "w2_infantry_3d",
    "w2_infantry_3d": "w2_infantry_3d",
}


def _with_blender_args(sub_args: List[str], fn: Callable[[], None]) -> None:
    saved = list(sys.argv)
    try:
        sys.argv = [saved[0], "--", *sub_args]
        fn()
    finally:
        sys.argv = saved


def _run_pilot_6(repo_root: str, only: str, sheet_only: bool, no_sheet: bool) -> None:
    from blender_ra2_factory.pilot_models import PILOT_BUILDERS
    from blender_ra2_factory.shp_vxl_blueprint import inspect_sprite_blueprint, inspect_vxl_blueprint
    from blender_ra2_factory.verify_sheet import PILOT_6_SPECS, render_glb_comparison_sheet

    ra2_dir = os.path.join(repo_root, "rust-ra2")
    models_dir = os.path.join(ra2_dir, "assets", "models")
    sheet_path = os.path.join(repo_root, "docs", "model_sheets", "pilot_6_vs_sprites.png")
    selected_ids = [s.strip() for s in only.split(",") if s.strip()] if only else list(PILOT_BUILDERS.keys())

    if not sheet_only:
        for asset_id in selected_ids:
            if asset_id not in PILOT_BUILDERS:
                raise ValueError(f"Unknown pilot_6 asset_id: {asset_id}")
            sbp = inspect_sprite_blueprint(ra2_dir, asset_id)
            if sbp is not None:
                print(f"[blueprint] {asset_id}: fp={sbp.toml_footprint} world={sbp.world_size_xz}m")
            else:
                vbp = inspect_vxl_blueprint(ra2_dir, asset_id)
                if vbp is not None:
                    print(f"[blueprint] {asset_id}: target={vbp.target_width_x}x{vbp.target_length_z}m")
            t_m = time.perf_counter()
            out_glb = os.path.join(models_dir, f"{asset_id}.glb")
            s = PILOT_BUILDERS[asset_id](out_glb)
            print(
                f"[pilot_6] {asset_id}.glb ({time.perf_counter() - t_m:.2f}s, {s.file_kb} KB) | "
                f"LOD0={s.lod0_verts}v LOD1={s.lod1_verts}v a0={s.team_alpha0_pct}%"
            )

    if not no_sheet:
        specs = [sp for sp in PILOT_6_SPECS if sp.asset_id in selected_ids]
        render_glb_comparison_sheet(ra2_dir, models_dir, sheet_path, specs)


def _run_bucket(bucket: str, repo_root: str, only: str, sheet_only: bool, no_sheet: bool) -> None:
    canon = BUCKET_ALIASES.get(bucket)
    if canon is None:
        raise ValueError(f"Unknown bucket '{bucket}'. Valid buckets: {', '.join(BUCKET_ORDER)}")

    print(f"\n=== [blender_ra2_factory] Running bucket: {canon} ===")
    if canon == "pilot_6":
        _run_pilot_6(repo_root, only, sheet_only, no_sheet)
    elif canon == "w2_bld_allied_soviet":
        from blender_ra2_factory.w2_bld_allied_soviet import cli as bld_as_cli

        bld_as_cli.REPO_ROOT = repo_root
        sub = (["--only", only] if only else []) + (["--sheet-only"] if sheet_only else []) + (["--no-sheet"] if no_sheet else [])
        _with_blender_args(sub, bld_as_cli.main)
    elif canon == "w2_bld_yuri_landmarks":
        from blender_ra2_factory.w2_bld_yuri_landmarks import cli as bld_yl_cli

        sub = ["--repo-root", repo_root, "--all"]
        sub += (["--only", only] if only else []) + (["--sheets-only"] if sheet_only else []) + (["--no-sheets"] if no_sheet else [])
        _with_blender_args(sub, bld_yl_cli.main)
    elif canon == "w2_veh_ground":
        from blender_ra2_factory.w2_veh_ground import cli as vg_cli

        vg_cli.REPO_ROOT = repo_root
        sub = (["--only", only] if only else []) + (["--sheets-only"] if sheet_only else []) + (["--no-sheets"] if no_sheet else [])
        _with_blender_args(sub, vg_cli.main)
    elif canon == "w2_veh_air_naval":
        from blender_ra2_factory.w2_veh_air_naval import cli as van_cli

        sub = ["--repo-root", repo_root, "--all"]
        sub += (["--only", only] if only else []) + (["--sheets-only"] if sheet_only else []) + (["--no-sheets"] if no_sheet else [])
        _with_blender_args(sub, van_cli.main)
    elif canon == "w2_infantry_3d":
        from blender_ra2_factory.w2_infantry_3d import cli as inf_cli

        sub = ["--repo-root", repo_root]
        if not sheet_only:
            sub += ["--only", only] if only else ["--build-all"]
        if not no_sheet:
            sub += ["--render-sheets"]
        inf_cli.main(sub)


def _verify_all_glbs(repo_root: str) -> None:
    from blender_ra2_factory.glb_postprocess import verify_baked_glb

    models_dir = os.path.join(repo_root, "rust-ra2", "assets", "models")
    top = sorted(glob.glob(os.path.join(models_dir, "*.glb")))
    props = sorted(glob.glob(os.path.join(models_dir, "props", "*.glb")))
    all_paths = top + props
    for gp in all_paths:
        verify_baked_glb(gp)
    print(f"[blender_ra2_factory] Verified {len(all_paths)}/{len(all_paths)} .glb files ({len(top)} top + {len(props)} props)!")


def _parse_blender_cli_args() -> argparse.Namespace:
    argv = sys.argv
    user_args = argv[argv.index("--") + 1 :] if "--" in argv else []
    p = argparse.ArgumentParser(description="Unified Headless Blender 5.3 + Cycles HIP GPU RA2 Asset Factory")
    p.add_argument("--repo-root", default=_REPO_ROOT, help="Repository root path")
    p.add_argument("--all", action="store_true", help="Run pilot_6 + all 5 Wave-2 subpackages (all 187 .glb models)")
    p.add_argument("--bucket", default="", help="Bucket name: pilot_6 | w2_bld_allied_soviet | w2_bld_yuri_landmarks | w2_veh_ground | w2_veh_air_naval | w2_infantry_3d | all")
    p.add_argument("--pilot-6", action="store_true", help="Alias for --bucket pilot_6")
    p.add_argument("--only", default="", help="Comma-separated asset IDs within the selected bucket")
    p.add_argument("--sheet-only", "--sheets-only", dest="sheet_only", action="store_true", help="Render comparison sheets only")
    p.add_argument("--no-sheet", "--no-sheets", dest="no_sheet", action="store_true", help="Skip comparison sheet rendering")
    p.add_argument("--verify-only", action="store_true", help="Verify all baked .glb assets on disk without rebuilding")
    return p.parse_args(user_args)


def main() -> None:
    args = _parse_blender_cli_args()
    repo_root = os.path.abspath(args.repo_root)
    t0 = time.perf_counter()
    print(f"[blender_ra2_factory] Repo root: {repo_root}")

    if args.verify_only:
        _verify_all_glbs(repo_root)
        return

    if args.all or args.bucket.strip().lower() == "all":
        if args.only:
            raise ValueError("--only cannot be combined with --all; specify --bucket <name> --only <ids>")
        for b in BUCKET_ORDER:
            _run_bucket(b, repo_root, "", args.sheet_only, args.no_sheet)
    elif args.bucket:
        for b in [x.strip() for x in args.bucket.split(",") if x.strip()]:
            _run_bucket(b, repo_root, args.only, args.sheet_only, args.no_sheet)
    elif args.pilot_6 or args.only or args.sheet_only:
        _run_bucket("pilot_6", repo_root, args.only, args.sheet_only, args.no_sheet)
    else:
        raise SystemExit("Specify --all, --bucket <name>, --pilot-6, or --verify-only (pass -- --help for usage).")

    print(f"[blender_ra2_factory] Finished in {time.perf_counter() - t0:.2f}s")


if __name__ == "__main__":
    main()
