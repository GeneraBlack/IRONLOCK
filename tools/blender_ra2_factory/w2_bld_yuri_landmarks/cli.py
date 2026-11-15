"""
Headless Blender 5.3 + Cycles HIP GPU CLI for Wave-2 `bld_yuri_landmarks` (`56` `.glb` assets + `5` sheets).

Usage (always set `TMP`/`TEMP` on `D:` first!):
    blender.exe --background --factory-startup --python tools/blender_ra2_factory/w2_bld_yuri_landmarks/cli.py -- --all
"""

from __future__ import annotations

import argparse
import os
import sys
import tempfile
import time
from typing import Any, Callable, Dict, List, Tuple

_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
_PKG_DIR = os.path.dirname(_THIS_DIR)
_TOOLS_DIR = os.path.dirname(_PKG_DIR)
_REPO_ROOT = os.path.dirname(_TOOLS_DIR)
if _TOOLS_DIR not in sys.path:
    sys.path.insert(0, _TOOLS_DIR)

from blender_ra2_factory import load_wave2_manifest  # noqa: E402
from blender_ra2_factory.glb_postprocess import GlbVerificationSummary, parse_glb, verify_baked_glb  # noqa: E402
from blender_ra2_factory.shp_vxl_blueprint import inspect_sprite_blueprint  # noqa: E402
from blender_ra2_factory.verify_sheet import SheetAssetSpec, render_glb_comparison_sheet  # noqa: E402
from blender_ra2_factory.w2_bld_yuri_landmarks.campaign_landmarks import (  # noqa: E402
    CAMPAIGN_LANDMARK_BUILDERS,
    CAMPAIGN_LANDMARK_SPECS,
)
from blender_ra2_factory.w2_bld_yuri_landmarks.map_props import (  # noqa: E402
    MAP_PROP_BUILDERS,
    MAP_PROP_SPECS,
)
from blender_ra2_factory.w2_bld_yuri_landmarks.tech_fort_buildings import (  # noqa: E402
    TECH_FORT_BUILDERS,
    TECH_FORT_SPECS,
)
from blender_ra2_factory.w2_bld_yuri_landmarks.yuri_buildings import (  # noqa: E402
    YURI_BUILDING_BUILDERS,
    YURI_BUILDING_SPECS,
)

BUCKET_NAME = "bld_yuri_landmarks"

ALL_BUILDERS: Dict[str, Callable[[str], GlbVerificationSummary]] = {
    **YURI_BUILDING_BUILDERS,
    **TECH_FORT_BUILDERS,
    **CAMPAIGN_LANDMARK_BUILDERS,
    **MAP_PROP_BUILDERS,
}

ALL_SPECS: List[SheetAssetSpec] = (
    YURI_BUILDING_SPECS + TECH_FORT_SPECS + CAMPAIGN_LANDMARK_SPECS + MAP_PROP_SPECS
)

SHEET_PARTITIONS: List[Tuple[int, str, List[SheetAssetSpec]]] = [
    (1, "WAVE-2 BLD_YURI_LANDMARKS (1/5): YURI CORE, PRODUCTION & TECH", YURI_BUILDING_SPECS[0:11]),
    (2, "WAVE-2 BLD_YURI_LANDMARKS (2/5): YURI SUPERWEAPONS & SHARED TECH", YURI_BUILDING_SPECS[11:16] + TECH_FORT_SPECS[0:6]),
    (3, "WAVE-2 BLD_YURI_LANDMARKS (3/5): SHARED TECH & WORLD LANDMARKS", TECH_FORT_SPECS[6:9] + CAMPAIGN_LANDMARK_SPECS[0:8]),
    (4, "WAVE-2 BLD_YURI_LANDMARKS (4/5): CAMPAIGN WONDERS & URBAN PROPS", CAMPAIGN_LANDMARK_SPECS[8:15] + MAP_PROP_SPECS[0:4]),
    (5, "WAVE-2 BLD_YURI_LANDMARKS (5/5): VEGETATION, ORE & GEM PROPS", MAP_PROP_SPECS[4:16]),
]


def _load_bucket_manifest_map() -> Dict[str, Dict[str, Any]]:
    manifest = load_wave2_manifest()
    out: Dict[str, Dict[str, Any]] = {}
    for entry in manifest["assets"]:
        if entry["bucket"] == BUCKET_NAME:
            rel_path = entry["rel_path"].replace("\\", "/")
            assert rel_path.endswith(".glb")
            asset_id = rel_path[:-4]
            out[asset_id] = entry
    assert len(out) == 56, f"Expected 56 {BUCKET_NAME} manifest entries, got {len(out)}"
    missing = set(out.keys()) ^ set(ALL_BUILDERS.keys())
    assert not missing, f"Builder registry mismatch vs wave2_manifest.json: {sorted(missing)}"
    assert len(ALL_SPECS) == 56 and sum(len(p[2]) for p in SHEET_PARTITIONS) == 56
    return out


def verify_against_manifest_entry(
    glb_path: str,
    summary: GlbVerificationSummary,
    entry: Dict[str, Any],
) -> None:
    """Enforces exact Wave-2 manifest invariants on `glb_path`."""
    aid = entry["rel_path"]
    assert summary.tex_size == int(entry["tex_size"]), f"{aid}: tex_size {summary.tex_size} != {entry['tex_size']}"
    max_v = 5800 if entry["is_unit"] else 11800
    assert summary.lod0_verts <= max_v, f"{aid}: LOD0 {summary.lod0_verts} > {max_v}"

    if entry["team_color_tint"]:
        assert summary.team_alpha0_pct >= 0.4, f"{aid}: team_color_tint=True but team_alpha0_pct={summary.team_alpha0_pct}%"
    else:
        assert summary.team_alpha0_pct == 0.0, f"{aid}: team_color_tint=False MUST have 0.0% Alpha=0, got {summary.team_alpha0_pct}%!"

    with open(glb_path, "rb") as f_glb:
        gltf, _ = parse_glb(f_glb.read())
    nodes = gltf.get("nodes", [])
    lod0_mesh_names = [
        n["name"]
        for n in nodes
        if "mesh" in n and not n.get("name", "").endswith("_LOD1")
    ]
    socket_names = [
        n["name"]
        for n in nodes
        if "mesh" not in n and n.get("name", "") != "" and not n.get("name", "").startswith("Scene")
        and any(n["name"].startswith(pfx) for pfx in ("Muzzle_", "Exhaust_", "DamageSmoke_"))
    ]
    assert set(lod0_mesh_names) == set(entry["existing_mesh_nodes"]), (
        f"{aid}: mesh nodes {lod0_mesh_names} != manifest {entry['existing_mesh_nodes']}"
    )
    assert set(socket_names) == set(entry["existing_socket_nodes"]), (
        f"{aid}: socket nodes {socket_names} != manifest {entry['existing_socket_nodes']}"
    )
    for nd in nodes:
        nm = nd.get("name", "")
        if nm in ("Turret", "Barrel", "Rotor"):
            tr = nd.get("translation", [0.0, 0.0, 0.0])
            assert abs(float(tr[0])) < 1e-4 and abs(float(tr[2])) < 1e-4, (
                f"{aid}: articulated node {nm} pivot {tr} violates x=0, z=0!"
            )


def _parse_args() -> argparse.Namespace:
    argv = sys.argv
    user_args = argv[argv.index("--") + 1 :] if "--" in argv else []
    parser = argparse.ArgumentParser(description="Wave-2 bld_yuri_landmarks Headless Blender 5.3 + Cycles HIP GPU Factory")
    parser.add_argument("--repo-root", default=_REPO_ROOT)
    parser.add_argument("--all", action="store_true", help="Build all 56 .glb models and render all 5 comparison sheets")
    parser.add_argument("--only", default="", help="Comma-separated asset IDs (e.g. yuri_conyard,props/tree_pine)")
    parser.add_argument("--sheets-only", action="store_true", help="Skip .glb baking and render sheets only")
    parser.add_argument("--no-sheets", action="store_true", help="Bake .glb models without rendering sheets")
    parser.add_argument("--sheet-index", type=int, default=0, help="Render only sheet 1..5 (0 = all 5)")
    return parser.parse_args(user_args)


def main() -> None:
    args = _parse_args()
    repo_root = os.path.abspath(args.repo_root)
    ra2_dir = os.path.join(repo_root, "rust-ra2")
    models_dir = os.path.join(ra2_dir, "assets", "models")
    sheets_dir = os.path.join(repo_root, "docs", "model_sheets")

    manifest_map = _load_bucket_manifest_map()
    selected_ids = (
        [s.strip() for s in args.only.split(",") if s.strip()]
        if args.only
        else [sp.asset_id for sp in ALL_SPECS]
    )

    t0 = time.perf_counter()
    print(f"[w2_bld_yuri_landmarks] Repo root: {repo_root} | Selected: {len(selected_ids)} assets")

    if not args.sheets_only:
        for idx, asset_id in enumerate(selected_ids, start=1):
            if asset_id not in ALL_BUILDERS:
                raise ValueError(f"Unknown asset_id in bld_yuri_landmarks: {asset_id}")
            entry = manifest_map[asset_id]
            if not asset_id.startswith("props/"):
                sbp = inspect_sprite_blueprint(ra2_dir, asset_id)
                if sbp is not None:
                    print(
                        f"[{idx:02d}/{len(selected_ids):02d}] [sprite] {asset_id}: fp={sbp.toml_footprint} "
                        f"xz={sbp.world_size_xz}m est_h={sbp.estimated_height_m}m"
                    )
            t_b = time.perf_counter()
            out_glb = os.path.join(models_dir, *asset_id.split("/")) + ".glb"
            summary = ALL_BUILDERS[asset_id](out_glb)
            verify_against_manifest_entry(out_glb, summary, entry)
            dt_b = time.perf_counter() - t_b
            print(
                f"[{idx:02d}/{len(selected_ids):02d}] [OK] {asset_id}.glb ({dt_b:.2f}s, {summary.file_kb} KB) | "
                f"LOD0={summary.lod0_verts}v LOD1={summary.lod1_verts}v tex={summary.tex_size}px "
                f"bounds={summary.bounds_min}..{summary.bounds_max} ao={summary.ao_min_max} "
                f"rough={summary.rough_min_max} a0={summary.team_alpha0_pct}%"
            )

    if (args.all or args.sheets_only or args.sheet_index > 0) and not args.no_sheets:
        # Ensure `<tmp>/props` exists because `verify_sheet.py` writes `<tmp>/props/<stem>_col2_tac.png`!
        os.makedirs(os.path.join(tempfile.gettempdir(), "props"), exist_ok=True)
        for s_idx, s_title, s_specs in SHEET_PARTITIONS:
            if args.sheet_index > 0 and s_idx != args.sheet_index:
                continue
            # Verify every .glb on the sheet against manifest before rendering
            for sp in s_specs:
                gp = os.path.join(models_dir, *sp.asset_id.split("/")) + ".glb"
                ver = verify_baked_glb(gp, max_lod0_verts=5800 if manifest_map[sp.asset_id]["is_unit"] else 11800)
                verify_against_manifest_entry(gp, ver, manifest_map[sp.asset_id])
            out_sheet = os.path.join(sheets_dir, f"bld_yuri_landmarks_{s_idx}.png")
            render_glb_comparison_sheet(
                ra2_dir=ra2_dir,
                models_dir=models_dir,
                out_png_path=out_sheet,
                asset_specs=s_specs,
                sheet_title=s_title,
            )

    print(f"[w2_bld_yuri_landmarks] Completed in {time.perf_counter() - t0:.2f}s")


if __name__ == "__main__":
    main()
