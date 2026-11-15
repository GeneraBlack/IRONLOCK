"""
Wave-3 Allied & Soviet Buildings (`27` assets) Headless Blender 5.3 CLI, Binary Verifier,
and Multi-Panel Preview Sheet Renderer (`tools/blender_ra2_factory/v3_ultra/w3_soviet_bld_cli.py`, `< 750`L).

Usage:
  blender.exe -b --factory-startup --python-exit-code 1 -P tools/blender_ra2_factory/v3_ultra/w3_soviet_bld_cli.py -- --only allied_barracks,soviet_refinery
  blender.exe -b --factory-startup --python-exit-code 1 -P tools/blender_ra2_factory/v3_ultra/w3_soviet_bld_cli.py -- --build-all
  blender.exe -b --factory-startup --python-exit-code 1 -P tools/blender_ra2_factory/v3_ultra/w3_soviet_bld_cli.py -- --verify-all
  blender.exe -b --factory-startup --python-exit-code 1 -P tools/blender_ra2_factory/v3_ultra/w3_soviet_bld_cli.py -- --render-sheet D:/projekt/rust-rts/engine/target/tmp/w3_sheet.png --cols 5
"""

from __future__ import annotations

import argparse
import importlib
import json
import os
import sys
import time
from typing import Callable, Dict, List, Sequence, Tuple

_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
_REPO_ROOT = os.path.abspath(os.path.join(_THIS_DIR, "..", "..", ".."))
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

import bpy
import numpy as np

from tools.blender_ra2_factory import load_wave2_manifest
from tools.blender_ra2_factory.animation_clips import verify_gltf_animations
from tools.blender_ra2_factory.glb_postprocess import parse_glb
from tools.blender_ra2_factory.pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from tools.blender_ra2_factory.png_io import decode_png_rgba, encode_png_bytes
from tools.blender_ra2_factory.v2_nextgen.glb_finalize_v2 import V2VerificationSummary, verify_v2_glb
from tools.blender_ra2_factory.v2_nextgen.render_hero_slice_sheet import _reset_imported_rig_to_rest
from tools.blender_ra2_factory.v3_ultra.cli_v3 import (
    BUILDING_CLIP_REQ,
    V3_TEAM_PAINT_SRGB,
    _bind_v3_glb_textures,
    _compute_visible_world_bounds,
    _render_centered_vertex_view,
)
from tools.blender_ra2_factory.v3_ultra.verify_all_187_v3 import audit_single_glb
from tools.blender_ra2_factory.verify_sheet import _draw_text, _setup_studio_world_and_lights

_PKG = "tools.blender_ra2_factory.v3_ultra"

W3_OWNED_27_BUILDERS: Dict[str, Tuple[str, str]] = {
    # 14 Allied Wave-3 Buildings
    "allied_barracks": (f"{_PKG}.w3_allied_bld_a", "build_v3_allied_barracks"),
    "allied_refinery": (f"{_PKG}.w3_allied_bld_a", "build_v3_allied_refinery"),
    "allied_airforce_command": (f"{_PKG}.w3_allied_bld_a", "build_v3_allied_airforce_command"),
    "allied_naval_shipyard": (f"{_PKG}.w3_allied_bld_a", "build_v3_allied_naval_shipyard"),
    "allied_service_depot": (f"{_PKG}.w3_allied_bld_b", "build_v3_allied_service_depot"),
    "allied_ore_purifier": (f"{_PKG}.w3_allied_bld_b", "build_v3_allied_ore_purifier"),
    "allied_robot_control": (f"{_PKG}.w3_allied_bld_b", "build_v3_allied_robot_control"),
    "allied_battle_lab": (f"{_PKG}.w3_allied_bld_b", "build_v3_allied_battle_lab"),
    "allied_gap_generator": (f"{_PKG}.w3_allied_bld_b", "build_v3_allied_gap_generator"),
    "allied_chronosphere": (f"{_PKG}.w3_allied_bld_c", "build_v3_allied_chronosphere"),
    "allied_weather_controller": (f"{_PKG}.w3_allied_bld_c", "build_v3_allied_weather_controller"),
    "allied_pillbox": (f"{_PKG}.w3_allied_bld_c", "build_v3_allied_pillbox"),
    "allied_patriot_missile": (f"{_PKG}.w3_allied_bld_c", "build_v3_allied_patriot_missile"),
    "allied_grand_cannon": (f"{_PKG}.w3_allied_bld_c", "build_v3_allied_grand_cannon"),
    # 13 Soviet Wave-3 Buildings
    "soviet_refinery": (f"{_PKG}.w3_soviet_bld_a", "build_v3_soviet_refinery"),
    "soviet_war_factory": (f"{_PKG}.w3_soviet_bld_a", "build_v3_soviet_war_factory"),
    "soviet_radar_tower": (f"{_PKG}.w3_soviet_bld_a", "build_v3_soviet_radar_tower"),
    "soviet_naval_shipyard": (f"{_PKG}.w3_soviet_bld_a", "build_v3_soviet_naval_shipyard"),
    "soviet_service_depot": (f"{_PKG}.w3_soviet_bld_b", "build_v3_soviet_service_depot"),
    "soviet_industrial_plant": (f"{_PKG}.w3_soviet_bld_b", "build_v3_soviet_industrial_plant"),
    "soviet_battle_lab": (f"{_PKG}.w3_soviet_bld_b", "build_v3_soviet_battle_lab"),
    "soviet_nuclear_reactor": (f"{_PKG}.w3_soviet_bld_b", "build_v3_soviet_nuclear_reactor"),
    "soviet_battle_bunker": (f"{_PKG}.w3_soviet_bld_b", "build_v3_soviet_battle_bunker"),
    "soviet_sentry_gun": (f"{_PKG}.w3_soviet_bld_c", "build_v3_soviet_sentry_gun"),
    "soviet_flak_cannon": (f"{_PKG}.w3_soviet_bld_c", "build_v3_soviet_flak_cannon"),
    "soviet_iron_curtain": (f"{_PKG}.w3_soviet_bld_c", "build_v3_soviet_iron_curtain"),
    "soviet_nuclear_silo": (f"{_PKG}.w3_soviet_bld_c", "build_v3_soviet_nuclear_silo"),
}

W3_TURRETED_COMBAT_IDS = frozenset(
    {
        "allied_pillbox",
        "allied_patriot_missile",
        "allied_grand_cannon",
        "soviet_sentry_gun",
        "soviet_flak_cannon",
    }
)
W3_ROTOR_ALLOWED_IDS = frozenset(
    {
        "allied_airforce_command",
        "allied_battle_lab",
        "soviet_radar_tower",
    }
)


def _models_dir() -> str:
    return os.path.join(_REPO_ROOT, "rust-ra2", "assets", "models")


def _load_builder(asset_id: str) -> Callable[[str], V2VerificationSummary]:
    if asset_id not in W3_OWNED_27_BUILDERS:
        raise ValueError(f"Unknown Wave-3 owned asset '{asset_id}'")
    mod_path, fn_name = W3_OWNED_27_BUILDERS[asset_id]
    mod = importlib.import_module(mod_path)
    return getattr(mod, fn_name)


def _manifest_by_stem() -> Dict[str, dict]:
    manifest = load_wave2_manifest()
    return {e["stem"]: e for e in manifest.get("assets", [])}


def verify_w3_owned_glb(asset_id: str, glb_path: str, manifest_entry: dict) -> V2VerificationSummary:
    if not os.path.isfile(glb_path):
        raise FileNotFoundError(f"Missing GLB: {glb_path}")

    req_clips: Sequence[str] = () if asset_id in W3_TURRETED_COMBAT_IDS else BUILDING_CLIP_REQ
    summary = verify_v2_glb(
        glb_path,
        expected_tex_size=2048,
        max_lod0_verts=34_500,
        require_skin=False,
        required_clips=req_clips,
    )
    assert 9_000 <= summary.lod0_verts <= 34_500, (
        f"{asset_id}: lod0_verts={summary.lod0_verts} outside required [9000, 34500] range!"
    )
    assert summary.team_tint_pct >= 1.2, (
        f"{asset_id}: team_tint_pct={summary.team_tint_pct:.2f}% < required 1.2%!"
    )

    raw = open(glb_path, "rb").read()
    gltf, bin_chunk = parse_glb(raw)
    node_names = [n.get("name", "") for n in gltf.get("nodes", [])]
    node_set = set(node_names)

    for req_mesh in manifest_entry.get("existing_mesh_nodes", []):
        assert req_mesh in node_set, f"{asset_id}: missing manifest mesh node '{req_mesh}' in {node_names}"
        assert f"{req_mesh}_LOD1" in node_set, f"{asset_id}: missing '{req_mesh}_LOD1' in {node_names}"

    for req_sock in manifest_entry.get("existing_socket_nodes", []):
        assert req_sock in node_set, f"{asset_id}: missing manifest socket '{req_sock}' in {node_names}"

    if asset_id not in W3_ROTOR_ALLOWED_IDS:
        bad_rotor = [n for n in node_names if n.startswith("Rotor")]
        assert not bad_rotor, f"{asset_id}: unexpected Rotor* node(s) {bad_rotor}"

    if asset_id not in W3_TURRETED_COMBAT_IDS:
        for bad_t in ("Turret", "Barrel"):
            assert bad_t not in node_set, f"{asset_id}: unexpected '{bad_t}' node on non-turreted building"
        verify_gltf_animations(gltf, bytearray(bin_chunk), glb_path=glb_path)

    # Verify all 4 PBR textures are exact 2048x2048 PNGs and alpha==0 pixels have exact R==G==B
    mat0 = gltf["materials"][0]
    pbr0 = mat0["pbrMetallicRoughness"]
    for label, tex_i in (
        ("baseColorTexture", pbr0["baseColorTexture"]["index"]),
        ("normalTexture", mat0["normalTexture"]["index"]),
        ("metallicRoughnessTexture", pbr0["metallicRoughnessTexture"]["index"]),
        ("emissiveTexture", mat0["emissiveTexture"]["index"]),
    ):
        img_i = gltf["textures"][tex_i]["source"]
        bv = gltf["bufferViews"][gltf["images"][img_i]["bufferView"]]
        arr = decode_png_rgba(bin_chunk[bv["byteOffset"] : bv["byteOffset"] + bv["byteLength"]])
        assert arr.shape[:2] == (2048, 2048), f"{asset_id}: {label} shape={arr.shape[:2]} != (2048, 2048)"
        if label == "baseColorTexture":
            a0 = arr[:, :, 3] == 0
            assert np.any(a0), f"{asset_id}: zero Alpha==0 team pixels in baseColorTexture!"
            assert np.array_equal(arr[:, :, 0][a0], arr[:, :, 1][a0]) and np.array_equal(arr[:, :, 1][a0], arr[:, :, 2][a0]), (
                f"{asset_id}: non-neutral RGB on Alpha==0 team pixels!"
            )

    return summary


def render_w3_contact_sheet(asset_ids: List[str], out_png: str, cols: int = 5, tile_w: int = 440, tile_h: int = 360) -> None:
    tmp_dir = os.path.dirname(os.path.abspath(out_png))
    os.makedirs(tmp_dir, exist_ok=True)
    rows = (len(asset_ids) + cols - 1) // cols
    canvas = np.full((rows * tile_h, cols * tile_w, 4), (18, 22, 28, 255), dtype=np.uint8)
    temp_files: List[str] = []

    try:
        for idx, aid in enumerate(asset_ids):
            glb_path = os.path.join(_models_dir(), f"{aid}.glb")
            reset_clean_blender_scene()
            configure_cycles_gpu(ao_distance=2.5)
            bpy.ops.import_scene.gltf(filepath=glb_path)
            _reset_imported_rig_to_rest()
            lod0_meshes = [o for o in bpy.data.objects if o.type == "MESH" and not o.name.endswith("_LOD1")]
            for o in bpy.data.objects:
                if o.type == "MESH" and o.name.endswith("_LOD1"):
                    o.hide_render = True
                    o.hide_viewport = True

            faction = "allied" if aid.startswith("allied_") else "soviet"
            temp_files.extend(_bind_v3_glb_textures(glb_path, lod0_meshes, V3_TEAM_PAINT_SRGB[faction], tmp_dir))

            bmin, bmax = _compute_visible_world_bounds(lod0_meshes)
            center = 0.5 * (bmin + bmax)
            _setup_studio_world_and_lights((center.x, center.y, center.z), max((bmax - bmin).length * 0.5, 0.8))
            gnd = bpy.data.objects.get("RA2_StudioGround")
            if gnd is not None:
                gnd.scale = (8.0, 8.0, 1.0)

            cam_data = bpy.data.cameras.new("SheetCam")
            cam_obj = bpy.data.objects.new("SheetCam", cam_data)
            bpy.context.collection.objects.link(cam_obj)

            cell_png = os.path.join(tmp_dir, f"_cell_{aid}.png")
            temp_files.append(cell_png)
            img = _render_centered_vertex_view(
                cam_obj, lod0_meshes, yaw_deg=42.0, pitch_deg=24.0, lens_mm=52.0,
                width=tile_w, height=tile_h - 32, samples=16, out_png=cell_png, margin_scale=1.14,
            )
            r_i, c_i = divmod(idx, cols)
            y0, x0 = r_i * tile_h, c_i * tile_w
            canvas[y0 + 32 : y0 + tile_h, x0 : x0 + tile_w, :3] = img[:, :, :3]
            _draw_text(canvas, x0 + 8, y0 + 8, aid.upper(), scale=1, color=(235, 242, 255, 255))
        with open(out_png, "wb") as f_png:
            f_png.write(encode_png_bytes(canvas))
    finally:
        for tf in temp_files:
            if os.path.isfile(tf):
                try:
                    os.remove(tf)
                except OSError:
                    pass


def main(argv: List[str]) -> int:
    ap = argparse.ArgumentParser(description="Wave-3 Allied & Soviet 27 Buildings CLI")
    ap.add_argument("--build-all", action="store_true")
    ap.add_argument("--skip-valid", action="store_true")
    ap.add_argument("--only", type=str, default="")
    ap.add_argument("--verify-all", action="store_true")
    ap.add_argument("--render-sheet", type=str, default="")
    ap.add_argument("--cols", type=int, default=5)
    args = ap.parse_args(argv)

    manifest_map = _manifest_by_stem()
    all_ids = list(W3_OWNED_27_BUILDERS.keys())
    selected = [s.strip() for s in args.only.split(",") if s.strip()] if args.only else all_ids
    ref_png = os.path.join(_REPO_ROOT, "docs", "screenshots", "ra2_ultra_v3_full_3d.png")
    ref_mtime = os.path.getmtime(ref_png) if os.path.isfile(ref_png) else 0.0

    failures: List[str] = []
    if args.build_all or (args.only and not args.verify_all and not args.render_sheet):
        for aid in selected:
            out_glb = os.path.join(_models_dir(), f"{aid}.glb")
            if args.skip_valid and os.path.isfile(out_glb) and os.path.getmtime(out_glb) > ref_mtime:
                m_ok, errs_ok = audit_single_glb(manifest_map[aid], _models_dir())
                if m_ok is not None and not errs_ok:
                    print(f"[W3-SKIP-VALID] {aid:28s} | LOD0={m_ok.lod0_verts:5d}V | tint={m_ok.team_tint_pct:5.2f}%", flush=True)
                    continue
            t0 = time.time()
            try:
                fn = _load_builder(aid)
                sm = fn(out_glb)
                verify_w3_owned_glb(aid, out_glb, manifest_map[aid])
                print(
                    f"[W3-OK] {aid:28s} | LOD0={sm.lod0_verts:5d}V LOD1={sm.lod1_verts:5d}V "
                    f"| HP={sm.high_tris:7d}T | tint={sm.team_tint_pct:5.2f}% | {time.time() - t0:5.1f}s",
                    flush=True,
                )
            except Exception as exc:
                msg = f"[W3-FAIL] {aid}: {exc}"
                print(msg, flush=True)
                failures.append(msg)
            time.sleep(1.5)
        if failures:
            raise RuntimeError(" ; ".join(failures))

    if args.verify_all:
        rows = []
        for aid in selected:
            out_glb = os.path.join(_models_dir(), f"{aid}.glb")
            sm = verify_w3_owned_glb(aid, out_glb, manifest_map[aid])
            rows.append((aid, sm.lod0_verts, sm.lod1_verts, sm.team_tint_pct, len(sm.animation_clips)))
            print(
                f"[VERIFY-OK] {aid:28s} | LOD0={sm.lod0_verts:5d}V LOD1={sm.lod1_verts:5d}V "
                f"| tint={sm.team_tint_pct:5.2f}% | clips={list(sm.animation_clips)}",
                flush=True,
            )
        print(json.dumps({"verified_count": len(rows), "min_verts": min(r[1] for r in rows), "max_verts": max(r[1] for r in rows), "min_tint": min(r[3] for r in rows)}))

    if args.render_sheet:
        render_w3_contact_sheet(selected, args.render_sheet, cols=args.cols)
        print(f"[SHEET-OK] {args.render_sheet}", flush=True)

    return 0


if __name__ == "__main__":
    raw_argv = sys.argv
    cli_argv = raw_argv[raw_argv.index("--") + 1 :] if "--" in raw_argv else []
    sys.exit(main(cli_argv))
