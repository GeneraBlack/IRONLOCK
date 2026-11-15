"""
Headless Blender 5.3 + Cycles HIP GPU CLI (`w2_bld_allied_soviet/cli.py`).

Usage:
    blender.exe --background --factory-startup --python tools/blender_ra2_factory/w2_bld_allied_soviet/cli.py -- [options]

Options:
    --only a,b,c     Build only comma-separated stems (must belong to `bld_allied_soviet`)
    --no-sheet       Skip comparison sheet rendering
    --sheet-only     Skip `.glb` baking and render comparison sheets only
    --sheets 1,2,3   Which of `{1, 2, 3}` comparison sheets to render
"""

from __future__ import annotations

import argparse
import os
import sys
import time
from typing import Any, Dict, List

THIS_DIR = os.path.dirname(os.path.abspath(__file__))
FACTORY_PARENT_DIR = os.path.dirname(os.path.dirname(THIS_DIR))
REPO_ROOT = os.path.dirname(FACTORY_PARENT_DIR)

if FACTORY_PARENT_DIR not in sys.path:
    sys.path.insert(0, FACTORY_PARENT_DIR)

from blender_ra2_factory import load_wave2_manifest  # noqa: E402
from blender_ra2_factory.glb_postprocess import parse_glb, verify_baked_glb  # noqa: E402
from blender_ra2_factory.verify_sheet import render_glb_comparison_sheet  # noqa: E402
from blender_ra2_factory.w2_bld_allied_soviet import (  # noqa: E402
    ALLIED_SOVIET_BUILDERS,
    SHEET_1_SPECS,
    SHEET_2_SPECS,
    SHEET_3_SPECS,
)


def _assert_manifest_contract(stem: str, glb_path: str, entry: Dict[str, Any]) -> None:
    """Verifies `verify_baked_glb` (with `max_lod0_verts=None`) AND exact manifest meshes/sockets!"""
    summary = verify_baked_glb(glb_path)
    with open(glb_path, "rb") as f:
        gltf_json, _ = parse_glb(f.read())
    node_names = {n.get("name", "") for n in gltf_json.get("nodes", [])}

    for req_mesh in entry.get("existing_mesh_nodes", []):
        if req_mesh not in node_names:
            raise AssertionError(f"{stem}: missing required mesh node '{req_mesh}' (nodes={sorted(node_names)})")

    for req_sock in entry.get("existing_socket_nodes", []):
        if req_sock not in node_names:
            raise AssertionError(f"{stem}: missing required socket node '{req_sock}' (nodes={sorted(node_names)})")

    if summary.team_alpha0_pct < 2.0:
        raise AssertionError(f"{stem}: team_alpha0_pct={summary.team_alpha0_pct}% too low for team_color_tint=True!")


def _parse_args() -> argparse.Namespace:
    argv = sys.argv
    extra = argv[argv.index("--") + 1 :] if "--" in argv else []
    p = argparse.ArgumentParser(description="Wave-2 Allied & Soviet 31 Buildings Baker + Sheet Renderer")
    p.add_argument("--only", type=str, default="", help="Comma-separated stems to bake")
    p.add_argument("--no-sheet", action="store_true", help="Skip rendering comparison sheets")
    p.add_argument("--sheet-only", action="store_true", help="Render comparison sheets only")
    p.add_argument("--sheets", type=str, default="1,2,3", help="Comma-separated sheet indices (1,2,3)")
    return p.parse_args(extra)


def main() -> None:
    args = _parse_args()
    manifest = load_wave2_manifest()
    bucket_entries: Dict[str, Dict[str, Any]] = {
        a["stem"]: a for a in manifest["assets"] if a.get("bucket") == "bld_allied_soviet"
    }
    if set(bucket_entries.keys()) != set(ALLIED_SOVIET_BUILDERS.keys()):
        missing = set(bucket_entries.keys()) - set(ALLIED_SOVIET_BUILDERS.keys())
        extra = set(ALLIED_SOVIET_BUILDERS.keys()) - set(bucket_entries.keys())
        raise RuntimeError(f"Builder registry mismatch! missing={missing}, extra={extra}")

    ra2_dir = os.path.join(REPO_ROOT, "rust-ra2")
    models_dir = os.path.join(ra2_dir, "assets", "models")
    sheets_dir = os.path.join(REPO_ROOT, "docs", "model_sheets")

    if not args.sheet_only:
        targets: List[str] = (
            [s.strip() for s in args.only.split(",") if s.strip()]
            if args.only
            else list(ALLIED_SOVIET_BUILDERS.keys())
        )
        t_all = time.time()
        for idx, stem in enumerate(targets, 1):
            if stem not in ALLIED_SOVIET_BUILDERS:
                raise KeyError(f"Unknown or non-owned stem: {stem}")
            out_glb = os.path.join(models_dir, f"{stem}.glb")
            t0 = time.time()
            summary = ALLIED_SOVIET_BUILDERS[stem](out_glb)
            _assert_manifest_contract(stem, out_glb, bucket_entries[stem])
            dt = time.time() - t0
            print(
                f"[{idx:02d}/{len(targets):02d}] {stem:26s} | LOD0:{summary.lod0_verts:5d}V "
                f"LOD1:{summary.lod1_verts:4d}V | A0:{summary.team_alpha0_pct:4.1f}% | {dt:5.1f}s",
                flush=True,
            )
        print(f"[w2_bld_allied_soviet] Baked & verified {len(targets)} .glb models in {time.time() - t_all:.1f}s", flush=True)

    if not args.no_sheet:
        sheet_map = {
            "1": (
                SHEET_1_SPECS,
                os.path.join(sheets_dir, "bld_allied_soviet_1.png"),
                "WAVE 2 ALLIED & SOVIET BUILDINGS (PART 1/3 - ALLIED BASE & SUPPORT)",
            ),
            "2": (
                SHEET_2_SPECS,
                os.path.join(sheets_dir, "bld_allied_soviet_2.png"),
                "WAVE 2 ALLIED & SOVIET BUILDINGS (PART 2/3 - ALLIED DEF/SUPER & SOVIET BASE)",
            ),
            "3": (
                SHEET_3_SPECS,
                os.path.join(sheets_dir, "bld_allied_soviet_3.png"),
                "WAVE 2 ALLIED & SOVIET BUILDINGS (PART 3/3 - SOVIET INDUSTRIAL, DEF & SUPER)",
            ),
        }
        wanted = [s.strip() for s in args.sheets.split(",") if s.strip()]
        for sk in wanted:
            specs, out_png, title = sheet_map[sk]
            render_glb_comparison_sheet(ra2_dir, models_dir, out_png, specs, sheet_title=title)


if __name__ == "__main__":
    main()
