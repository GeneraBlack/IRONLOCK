"""
CLI Builder, Strict Manifest/16-Color Verifier & Temp Preview Sheet Renderer for all 29
Wave-3 Ground Vehicles (`tools/blender_ra2_factory/v3_ultra/w3_veh_ground_cli.py`).

Usage:
  blender.exe -b --factory-startup --python-exit-code 1 -P tools/blender_ra2_factory/v3_ultra/w3_veh_ground_cli.py -- --all
  blender.exe -b --factory-startup --python-exit-code 1 -P tools/blender_ra2_factory/v3_ultra/w3_veh_ground_cli.py -- --only prism_tank,rhino_tank
  blender.exe -b --factory-startup --python-exit-code 1 -P tools/blender_ra2_factory/v3_ultra/w3_veh_ground_cli.py -- --verify-only
  blender.exe -b --factory-startup --python-exit-code 1 -P tools/blender_ra2_factory/v3_ultra/w3_veh_ground_cli.py -- --render-preview <tmp_png>
"""

from __future__ import annotations

import argparse
import os
import shutil
import sys
import tempfile
import time
from typing import Callable, Dict, List, Sequence, Tuple

_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
_REPO_ROOT = os.path.abspath(os.path.join(_THIS_DIR, "..", "..", ".."))
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

import bpy
import numpy as np

from tools.blender_ra2_factory import load_wave2_manifest
from tools.blender_ra2_factory.glb_postprocess import parse_glb
from tools.blender_ra2_factory.pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from tools.blender_ra2_factory.png_io import decode_png_rgba, encode_png_bytes
from tools.blender_ra2_factory.v2_nextgen.glb_finalize_v2 import V2VerificationSummary, verify_v2_glb
from tools.blender_ra2_factory.v2_nextgen.render_hero_slice_sheet import _reset_imported_rig_to_rest
from tools.blender_ra2_factory.v3_ultra import MAX_V3_UNIT_LOD0_VERTS, V3_VEHICLE_TEX_SIZE
from tools.blender_ra2_factory.v3_ultra.cli_v3 import _bind_v3_glb_textures, _render_centered_vertex_view
from tools.blender_ra2_factory.v3_ultra.verify_all_187_v3 import audit_single_glb
from tools.blender_ra2_factory.v3_ultra.w3_veh_ground_allied_a import ALLIED_GROUND_A_BUILDERS
from tools.blender_ra2_factory.v3_ultra.w3_veh_ground_allied_b import ALLIED_GROUND_B_BUILDERS
from tools.blender_ra2_factory.v3_ultra.w3_veh_ground_soviet_a import SOVIET_GROUND_A_BUILDERS
from tools.blender_ra2_factory.v3_ultra.w3_veh_ground_soviet_b import SOVIET_GROUND_B_BUILDERS
from tools.blender_ra2_factory.v3_ultra.w3_veh_ground_yuri_a import YURI_GROUND_A_BUILDERS
from tools.blender_ra2_factory.v3_ultra.w3_veh_ground_yuri_b import YURI_GROUND_B_BUILDERS
from tools.blender_ra2_factory.verify_sheet import (
    _compute_visible_world_bounds,
    _draw_text,
    _setup_studio_world_and_lights,
)

MIN_V3_VEH_LOD0_VERTS: int = 7_500

W3_UNIQUE_BUILDERS: Dict[str, Callable[[str], V2VerificationSummary]] = {
    **ALLIED_GROUND_A_BUILDERS,
    **ALLIED_GROUND_B_BUILDERS,
    **SOVIET_GROUND_A_BUILDERS,
    **SOVIET_GROUND_B_BUILDERS,
    **YURI_GROUND_A_BUILDERS,
    **YURI_GROUND_B_BUILDERS,
}

# Exact 29 Ground Vehicle stems owned by `v3-w3-veh-ground` (NEVER touch `grizzly_tank` or `apocalypse_tank`!)
W3_VEH_GROUND_STEMS: Tuple[str, ...] = (
    # Allied (9)
    "prism_tank",
    "mirage_tank",
    "ifv",
    "robot_tank",
    "tank_destroyer",
    "battle_fortress",
    "chrono_miner",
    "allied_mcv",
    "allied_lcv",
    # Soviet (10)
    "rhino_tank",
    "tesla_tank",
    "flak_track",
    "v3_rocket_launcher",
    "demolition_truck",
    "war_miner",
    "soviet_mcv",
    "soviet_sub_apc",
    "soviet_zubr_transport",
    "terror_drone",
    # Yuri & Special (10, where `gattling_tank` copies verified `gatling_tank`)
    "lasher_tank",
    "gatling_tank",
    "gattling_tank",
    "chaos_drone",
    "magnetron",
    "mastermind",
    "yuri_slave_miner",
    "yuri_mcv",
    "yuri_hover_transport",
    "crashed_ufo",
)


def _manifest_veh_ground_map() -> Dict[str, dict]:
    manifest = load_wave2_manifest()
    out: Dict[str, dict] = {}
    for entry in manifest["assets"]:
        stem = entry["stem"]
        if stem in W3_VEH_GROUND_STEMS:
            out[stem] = entry
    if len(out) != 29:
        raise RuntimeError(f"Expected 29 veh_ground manifest entries, found {len(out)}")
    return out


def verify_w3_veh_ground_glb(stem: str, glb_path: str, entry: dict) -> V2VerificationSummary:
    """Runs Gate 1-5 checks: `verify_v2_glb`, `8k-22.8k` LOD0 verts, exact manifest nodes/sockets/pivots & 16-color grayscale!"""
    req_clips = tuple(entry.get("existing_animations", []))
    summary = verify_v2_glb(
        glb_path,
        max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS,
        expected_tex_size=V3_VEHICLE_TEX_SIZE,
        required_clips=req_clips,
    )
    if summary.lod0_verts < MIN_V3_VEH_LOD0_VERTS or summary.lod0_verts > MAX_V3_UNIT_LOD0_VERTS:
        raise AssertionError(
            f"[{stem}] LOD0 split verts {summary.lod0_verts} outside [{MIN_V3_VEH_LOD0_VERTS}, {MAX_V3_UNIT_LOD0_VERTS}]"
        )

    gltf, bin_chunk = parse_glb(open(glb_path, "rb").read())
    nodes = gltf.get("nodes", [])
    if not nodes or nodes[0].get("name") != "Hull":
        raise AssertionError(f"[{stem}] nodes[0] must be 'Hull', got {nodes[0].get('name') if nodes else None}")

    name_to_idx = {n.get("name", ""): i for i, n in enumerate(nodes)}
    parent_of: Dict[str, str] = {}
    for n in nodes:
        p_nm = n.get("name", "")
        for c_i in n.get("children", []):
            parent_of[nodes[int(c_i)].get("name", "")] = p_nm

    lod0_mesh_names = [n["name"] for n in nodes if "mesh" in n and not n["name"].endswith("_LOD1")]
    exp_meshes = list(entry["existing_mesh_nodes"])
    if set(lod0_mesh_names) != set(exp_meshes):
        raise AssertionError(f"[{stem}] mesh nodes {lod0_mesh_names} != manifest {exp_meshes}")
    for m_nm in exp_meshes:
        if f"{m_nm}_LOD1" not in name_to_idx:
            raise AssertionError(f"[{stem}] missing '{m_nm}_LOD1' child mesh")

    sock_names = [
        n["name"] for n in nodes
        if "mesh" not in n and not n["name"].endswith("_LOD1") and n["name"] != "Scene"
    ]
    exp_socks = list(entry["existing_socket_nodes"])
    if set(sock_names) != set(exp_socks):
        raise AssertionError(f"[{stem}] socket nodes {sock_names} != manifest {exp_socks}")

    anim_names = [a.get("name", "") for a in gltf.get("animations", [])]
    if set(anim_names) != set(req_clips):
        raise AssertionError(f"[{stem}] animations {anim_names} != manifest {list(req_clips)}")

    if "Turret" in name_to_idx:
        if parent_of.get("Turret") != "Hull":
            raise AssertionError(f"[{stem}] 'Turret' must be direct child of 'Hull', got {parent_of.get('Turret')}")
        t_tr = nodes[name_to_idx["Turret"]].get("translation", [0.0, 0.0, 0.0])
        if abs(float(t_tr[0])) > 1e-4:
            raise AssertionError(f"[{stem}] 'Turret' translation.x must be 0.0, got {t_tr}")

    if "Barrel" in name_to_idx:
        if parent_of.get("Barrel") != "Turret":
            raise AssertionError(f"[{stem}] 'Barrel' must be direct child of 'Turret', got {parent_of.get('Barrel')}")
        b_tr = nodes[name_to_idx["Barrel"]].get("translation", [0.0, 0.0, 0.0])
        if abs(float(b_tr[0])) > 1e-4 or abs(float(b_tr[2])) > 1e-4:
            raise AssertionError(f"[{stem}] 'Barrel' translation.x & .z must be 0.0, got {b_tr}")

    # Strict 16-Color Multiplayer + Full `verify_all_187_v3` Audit Check!
    _audit_m, audit_errs = audit_single_glb(entry, os.path.dirname(os.path.abspath(glb_path)))
    if audit_errs:
        raise AssertionError(f"[{stem}] verify_all_187_v3 failed: {'; '.join(audit_errs)}")

    base_tex_i = gltf["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"]["index"]
    img_i = gltf["textures"][base_tex_i]["source"]
    bv = gltf["bufferViews"][gltf["images"][img_i]["bufferView"]]
    base_rgba = decode_png_rgba(bytes(bin_chunk[bv["byteOffset"] : bv["byteOffset"] + bv["byteLength"]]))
    if summary.team_tint_pct < 0.85:
        raise AssertionError(f"[{stem}] team_tint_pct={summary.team_tint_pct:.2f}% < 0.85% minimum!")
    team_mask = base_rgba[:, :, 3] == 0
    if team_mask.any():
        r, g, b = base_rgba[team_mask, 0], base_rgba[team_mask, 1], base_rgba[team_mask, 2]
        bad = int(np.sum((r != g) | (g != b)))
        if bad > 0:
            raise AssertionError(f"[{stem}] {bad} team pixels (alpha==0) violate neutral grayscale R==G==B!")
    return summary


def build_and_verify_stem(stem: str, models_dir: str, manifest_map: Dict[str, dict]) -> V2VerificationSummary:
    out_path = os.path.join(models_dir, f"{stem}.glb")
    if stem == "gattling_tank":
        src_path = os.path.join(models_dir, "gatling_tank.glb")
        if not os.path.isfile(src_path):
            W3_UNIQUE_BUILDERS["gatling_tank"](src_path)
            verify_w3_veh_ground_glb("gatling_tank", src_path, manifest_map["gatling_tank"])
        shutil.copy2(src_path, out_path)
    else:
        W3_UNIQUE_BUILDERS[stem](out_path)
    return verify_w3_veh_ground_glb(stem, out_path, manifest_map[stem])


def render_preview_contact_sheet(
    out_png: str,
    models_dir: str,
    stems: Sequence[str],
    cols: int = 5,
    cell_px: int = 380,
    team_mode: str = "faction",
) -> None:
    """Renders an isometric Cycles GPU inspection sheet to `out_png` and cleans all temporary texture files."""
    tmp_dir = os.environ.get("TMP") or os.environ.get("TEMP") or tempfile.gettempdir()
    rows = (len(stems) + cols - 1) // cols
    canvas = np.full((rows * cell_px, cols * cell_px, 4), (20, 23, 29, 255), dtype=np.uint8)
    manifest_map = _manifest_veh_ground_map()

    multi_palette = [
        (38, 108, 235), (220, 38, 38), (235, 185, 32), (32, 196, 108),
        (235, 118, 28), (42, 198, 218), (152, 54, 218), (235, 72, 164),
    ]

    for idx, stem in enumerate(stems):
        glb_path = os.path.join(models_dir, f"{stem}.glb")
        faction = manifest_map[stem]["faction"]
        if team_mode == "multi16":
            team_srgb = multi_palette[idx % len(multi_palette)]
        else:
            team_srgb = (44, 112, 232) if faction.startswith("allie") else ((214, 38, 38) if faction.startswith("soviet") else (148, 52, 216))

        reset_clean_blender_scene()
        configure_cycles_gpu(ao_distance=2.2)
        bpy.ops.import_scene.gltf(filepath=glb_path)
        _reset_imported_rig_to_rest()

        all_meshes = [o for o in bpy.data.objects if o.type == "MESH"]
        lod0_meshes = []
        for o in all_meshes:
            if o.name.endswith("_LOD1"):
                o.hide_render = True
                o.hide_viewport = True
            else:
                lod0_meshes.append(o)

        tmp_tex_files = _bind_v3_glb_textures(glb_path, lod0_meshes, team_srgb, tmp_dir)
        bmin, bmax = _compute_visible_world_bounds(lod0_meshes)
        center = 0.5 * (bmin + bmax)
        _setup_studio_world_and_lights((center.x, center.y, center.z), max((bmax - bmin).length * 0.5, 0.8))
        gnd = bpy.data.objects.get("RA2_StudioGround")
        if gnd is not None:
            gnd.scale = (8.0, 8.0, 1.0)
        cam_data = bpy.data.cameras.new("RA2_W3PrevCam")
        cam_obj = bpy.data.objects.new("RA2_W3PrevCam", cam_data)
        bpy.context.collection.objects.link(cam_obj)

        cell_tmp = os.path.join(tmp_dir, f"_w3_prev_cell_{stem}.png")
        try:
            rgb = _render_centered_vertex_view(
                cam_obj=cam_obj, lod0_meshes=lod0_meshes,
                yaw_deg=38.0, pitch_deg=26.0, lens_mm=62.0,
                width=cell_px, height=cell_px, samples=24,
                out_png=cell_tmp, margin_scale=1.12,
            )
            r_i, c_i = divmod(idx, cols)
            y0, x0 = r_i * cell_px, c_i * cell_px
            canvas[y0 : y0 + cell_px, x0 : x0 + cell_px, :3] = rgb
            _draw_text(canvas[:, :, :3], x0 + 8, y0 + 8, stem.upper()[:18], color=(240, 244, 250), scale=2)
        finally:
            for tf in [*tmp_tex_files, cell_tmp]:
                try:
                    if os.path.isfile(tf):
                        os.remove(tf)
                except OSError:
                    pass

    os.makedirs(os.path.dirname(os.path.abspath(out_png)), exist_ok=True)
    with open(out_png, "wb") as f:
        f.write(encode_png_bytes(canvas))
    print(f"[W3_PREVIEW] Wrote {out_png} ({canvas.shape[1]}x{canvas.shape[0]})", flush=True)


def main() -> None:
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    ap = argparse.ArgumentParser(description="Wave-3 v3_ultra Ground Vehicles CLI")
    ap.add_argument("--all", action="store_true", help="Build and verify all 29 ground vehicle .glbs")
    ap.add_argument("--only", type=str, default="", help="Comma-separated subset of stems to build & verify")
    ap.add_argument("--skip-valid", action="store_true", help="Skip re-baking .glbs that already pass verify_all_187_v3")
    ap.add_argument("--verify-only", action="store_true", help="Verify existing .glbs without rebuilding")
    ap.add_argument("--render-preview", type=str, default="", help="Render preview sheet PNG to given path")
    ap.add_argument("--team-mode", type=str, default="faction", choices=("faction", "multi16"))
    ap.add_argument("--models-dir", type=str, default=os.path.join(_REPO_ROOT, "rust-ra2", "assets", "models"))
    args = ap.parse_args(argv)

    manifest_map = _manifest_veh_ground_map()
    stems = [s.strip() for s in args.only.split(",") if s.strip()] if args.only else list(W3_VEH_GROUND_STEMS)
    for s in stems:
        if s not in W3_VEH_GROUND_STEMS:
            raise ValueError(f"Stem '{s}' is not owned by v3-w3-veh-ground!")

    if args.verify_only:
        for idx, stem in enumerate(stems, 1):
            s = verify_w3_veh_ground_glb(stem, os.path.join(args.models_dir, f"{stem}.glb"), manifest_map[stem])
            print(
                f"[VERIFY {idx:02d}/{len(stems):02d}] {stem:24s} | {s.file_kb:5d} KB | LOD0={s.lod0_verts:5d}v | "
                f"LOD1={s.lod1_verts:4d}v | team={s.team_tint_pct:5.1f}% | clips={len(s.animation_clips)}",
                flush=True,
            )
    elif args.all or (args.only and not args.render_preview):
        t_all = time.perf_counter()
        for idx, stem in enumerate(stems, 1):
            glb_p = os.path.join(args.models_dir, f"{stem}.glb")
            if args.skip_valid and os.path.isfile(glb_p):
                m_ok, errs_ok = audit_single_glb(manifest_map[stem], args.models_dir)
                if not errs_ok and m_ok is not None:
                    s_ok = verify_w3_veh_ground_glb(stem, glb_p, manifest_map[stem])
                    print(
                        f"[SKIP   {idx:02d}/{len(stems):02d}] {stem:24s} | {s_ok.file_kb:5d} KB | "
                        f"LOD0={s_ok.lod0_verts:5d}v | team={s_ok.team_tint_pct:5.1f}% (already valid v3_ultra)",
                        flush=True,
                    )
                    continue
            t0 = time.perf_counter()
            s = build_and_verify_stem(stem, args.models_dir, manifest_map)
            dt = time.perf_counter() - t0
            print(
                f"[BUILT  {idx:02d}/{len(stems):02d}] {stem:24s} | {dt:5.1f}s | {s.file_kb:5d} KB | "
                f"HP={s.high_tris:7d}t | LOD0={s.lod0_verts:5d}v | team={s.team_tint_pct:5.1f}% | clips={len(s.animation_clips)}",
                flush=True,
            )
            if stem != "gattling_tank" and idx < len(stems):
                time.sleep(1.5)
        print(f"[DONE] Built & verified {len(stems)} v3_ultra ground vehicles in {time.perf_counter() - t_all:.1f}s", flush=True)

    if args.render_preview:
        render_preview_contact_sheet(args.render_preview, args.models_dir, stems, team_mode=args.team_mode)


if __name__ == "__main__":
    main()
