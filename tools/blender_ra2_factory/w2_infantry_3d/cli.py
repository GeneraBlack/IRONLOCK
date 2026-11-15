"""
Headless Blender 5.3 Batch Baker, Manifest Contract Verifier & Sheet Renderer (`cli.py`).

Usage:
  blender.exe --background --factory-startup --python tools/blender_ra2_factory/w2_infantry_3d/cli.py -- --build-all --render-sheets
"""

from __future__ import annotations

import argparse
import json
import math
import os
import sys
import tempfile
import time
from typing import Dict, List, Sequence, Tuple

import bpy
import numpy as np

# Allow direct execution via `blender --python .../w2_infantry_3d/cli.py`
_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
_TOOLS_DIR = os.path.abspath(os.path.join(_THIS_DIR, "..", ".."))
if _TOOLS_DIR not in sys.path:
    sys.path.insert(0, _TOOLS_DIR)

from blender_ra2_factory import load_wave2_manifest
from blender_ra2_factory.glb_postprocess import GlbVerificationSummary, parse_glb, verify_baked_glb
from blender_ra2_factory.pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from blender_ra2_factory.png_io import decode_png_rgba, encode_png_bytes
from blender_ra2_factory.verify_sheet import (
    FACTION_TEAM_SRGB,
    _bind_glb_textures_to_imported_meshes,
    _compute_visible_world_bounds,
    _draw_text,
    _render_camera_view,
    _setup_studio_world_and_lights,
)
from blender_ra2_factory.w2_infantry_3d import (
    INFANTRY_3D_BUILDERS,
    INFANTRY_3D_CATALOG,
    SHEET_SPLITS,
    InfantryAssetMeta,
)


def _render_infantry_ref_card(
    ra2_dir: str,
    meta: InfantryAssetMeta,
    team_rgb: Tuple[int, int, int],
    card_w: int = 784,
    card_h: int = 500,
    bg_rgb: Tuple[int, int, int] = (30, 35, 45),
) -> np.ndarray:
    """
    Renders Column 1 for infantry: authentic Westwood `stand[3]` (45° SW front-left view matching
    Column 2's 45° yaw!) tinted to `team_rgb` + authentic Westwood cameo icon inset!
    """
    canvas = np.empty((card_h, card_w, 3), dtype=np.uint8)
    canvas[:] = bg_rgb

    cat_path = os.path.join(ra2_dir, "assets", "sprites", "sprites_catalog.json")
    catalog = json.load(open(cat_path, "r", encoding="utf-8")) if os.path.isfile(cat_path) else {}
    entry = catalog.get("infantry", {}).get(meta.sprite_id)

    if entry is not None:
        atlas_path = os.path.join(ra2_dir, entry["atlas"])
        if os.path.isfile(atlas_path):
            atlas = decode_png_rgba(open(atlas_path, "rb").read())
            cw, ch = int(entry["cell_width"]), int(entry["cell_height"])
            cols = max(1, int(entry["columns"]))
            stand_frames = entry.get("stand", [0, 1, 2, 3])
            # `stand[3]` is Westwood SW front-left 45° isometric direction matching `yaw=45°`!
            fi = int(stand_frames[3]) if len(stand_frames) > 3 else int(entry.get("idle", 0))
            r, c = fi // cols, fi % cols
            cell = atlas[r * ch : (r + 1) * ch, c * cw : (c + 1) * cw].astype(np.float32)
            a = atlas[r * ch : (r + 1) * ch, c * cw : (c + 1) * cw, 3]

            rendered = np.empty((ch, cw, 3), dtype=np.uint8)
            rendered[:] = bg_rgb
            sh = (a >= 80) & (a <= 145)
            rendered[sh] = tuple(max(8, int(v * 0.48)) for v in bg_rgb)

            tm = (a >= 160) & (a <= 225)
            lum = (0.299 * cell[:, :, 0] + 0.587 * cell[:, :, 1] + 0.114 * cell[:, :, 2]) / 255.0
            for ci in range(3):
                tc = np.clip(team_rgb[ci] * lum * 1.65, 0, 255).astype(np.uint8)
                rendered[:, :, ci] = np.where(tm, tc, rendered[:, :, ci])

            op = a >= 230
            for ci in range(3):
                rendered[:, :, ci] = np.where(op, cell[:, :, ci].astype(np.uint8), rendered[:, :, ci])

            vis = a >= 80
            ys, xs = np.nonzero(vis)
            if len(xs) > 0:
                pad = 6
                x0, x1 = max(0, int(xs.min()) - pad), min(cw, int(xs.max()) + pad + 1)
                y0, y1 = max(0, int(ys.min()) - pad), min(ch, int(ys.max()) + pad + 1)
                crop = rendered[y0:y1, x0:x1]
            else:
                crop = rendered

            avail_w = int(card_w * 0.58)
            scale = min((avail_w - 32) / max(crop.shape[1], 1), (card_h - 64) / max(crop.shape[0], 1))
            new_w = max(1, int(round(crop.shape[1] * scale)))
            new_h = max(1, int(round(crop.shape[0] * scale)))
            yi = np.clip((np.arange(new_h) / scale).astype(np.int32), 0, crop.shape[0] - 1)
            xi = np.clip((np.arange(new_w) / scale).astype(np.int32), 0, crop.shape[1] - 1)
            scaled = crop[yi[:, None], xi[None, :]]

            ox = (avail_w - new_w) // 2 + 12
            oy = (card_h - new_h) // 2 + 8
            canvas[oy : oy + new_h, ox : ox + new_w] = scaled

    # Overlay Westwood Cameo Icon on right side of Column 1
    cameo_full = os.path.join(ra2_dir, meta.cameo_rel)
    if os.path.isfile(cameo_full):
        cam_rgb = decode_png_rgba(open(cameo_full, "rb").read())[:, :, :3]
        ch_c, cw_c = cam_rgb.shape[:2]
        scale_c = min(230 / max(cw_c, 1), 186 / max(ch_c, 1))
        nw, nh = max(1, int(round(cw_c * scale_c))), max(1, int(round(ch_c * scale_c)))
        yi = np.clip((np.arange(nh) / scale_c).astype(np.int32), 0, ch_c - 1)
        xi = np.clip((np.arange(nw) / scale_c).astype(np.int32), 0, cw_c - 1)
        scaled_c = cam_rgb[yi[:, None], xi[None, :]]
        ix, iy = card_w - nw - 26, (card_h - nh) // 2
        canvas[iy - 3 : iy + nh + 3, ix - 3 : ix + nw + 3] = (90, 104, 128)
        canvas[iy : iy + nh, ix : ix + nw] = scaled_c

    if meta.stem != meta.sprite_id:
        _draw_text(canvas, 16, 14, f"REF: {meta.sprite_id} (STEM: {meta.stem})", color=(160, 180, 210), scale=1)

    return canvas


def _render_infantry_glb_pair(
    glb_path: str,
    meta: InfantryAssetMeta,
    cell_w: int = 784,
    cell_h: int = 500,
    samples: int = 34,
) -> Tuple[np.ndarray, np.ndarray]:
    """Renders Column 2 (`45°/30°` rest game view) and Column 3 (`54mm` posed close-up hero shot)."""
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=0.8)

    bpy.ops.import_scene.gltf(filepath=glb_path)

    lod0_meshes: List[bpy.types.Object] = []
    by_name: Dict[str, bpy.types.Object] = {}
    for obj in bpy.data.objects:
        if obj.name.endswith("_LOD1"):
            obj.hide_render = True
            obj.hide_viewport = True
            continue
        by_name[obj.name] = obj
        if obj.type == "MESH":
            lod0_meshes.append(obj)

    team_srgb = FACTION_TEAM_SRGB.get(meta.sheet_faction, FACTION_TEAM_SRGB["allied"])
    tmp_dir = os.environ.get("TMP") or os.environ.get("TEMP") or tempfile.gettempdir()
    os.makedirs(tmp_dir, exist_ok=True)
    temp_files = _bind_glb_textures_to_imported_meshes(glb_path, lod0_meshes, team_srgb, tmp_dir)

    bmin, bmax = _compute_visible_world_bounds(lod0_meshes)
    span_radius = max((bmax - bmin).length * 0.5, 0.8)
    center = 0.5 * (bmin + bmax)
    _setup_studio_world_and_lights((center.x, center.y, center.z), span_radius)

    cam_data = bpy.data.cameras.new("RA2_InfSheetCam")
    cam_obj = bpy.data.objects.new("RA2_InfSheetCam", cam_data)
    bpy.context.collection.objects.link(cam_obj)

    out_tac = os.path.join(tmp_dir, f"{meta.stem}_inf_tac.png")
    out_hero = os.path.join(tmp_dir, f"{meta.stem}_inf_hero.png")
    temp_files.extend([out_tac, out_hero])

    tac_rgb = _render_camera_view(
        cam_obj,
        bmin,
        bmax,
        yaw_deg=45.0,
        pitch_deg=30.0,
        lens_mm=118.0,
        width=cell_w,
        height=cell_h,
        samples=samples,
        out_png=out_tac,
        margin_scale=1.08,
    )

    # Apply subtle articulated action pose in Column 3 to showcase clean limb pivots & zero ghost AO!
    def _rot_limb(name: str, rx: float = 0.0, ry: float = 0.0, rz: float = 0.0) -> None:
        o = by_name.get(name)
        if o is not None:
            o.rotation_mode = "XYZ"
            o.rotation_euler = (o.rotation_euler.x + rx, o.rotation_euler.y + ry, o.rotation_euler.z + rz)

    if meta.is_dog:
        _rot_limb("LeftLeg", rx=-0.26)
        _rot_limb("RightLeg", rx=+0.24)
        _rot_limb("Head", rx=-0.08, rz=+0.10)
        _rot_limb("WeaponArm", rx=+0.22)
    elif meta.stem == "yuri_prime":
        _rot_limb("Torso", rz=-0.08)
        _rot_limb("Head", rz=+0.16)
    else:
        _rot_limb("LeftLeg", rx=-0.22)
        _rot_limb("RightLeg", rx=+0.22)
        _rot_limb("Torso", rz=-0.10)
        _rot_limb("Head", rz=+0.16)
        _rot_limb("WeaponArm", rx=-0.08)

    bpy.context.view_layer.update()
    bmin_h, bmax_h = _compute_visible_world_bounds(lod0_meshes)

    hero_rgb = _render_camera_view(
        cam_obj,
        bmin_h,
        bmax_h,
        yaw_deg=meta.hero_yaw_deg,
        pitch_deg=meta.hero_pitch_deg,
        lens_mm=54.0,
        width=cell_w,
        height=cell_h,
        samples=samples,
        out_png=out_hero,
        margin_scale=1.02,
    )

    for tf in temp_files:
        if os.path.isfile(tf):
            try:
                os.remove(tf)
            except OSError:
                pass

    return tac_rgb, hero_rgb


def render_infantry_sheet(
    ra2_dir: str,
    models_dir: str,
    out_png_path: str,
    metas: Sequence[InfantryAssetMeta],
    sheet_title: str,
) -> str:
    sheet_w = 2400
    pad_x = 12
    col_gap = 12
    cell_w = (sheet_w - 2 * pad_x - 2 * col_gap) // 3
    cell_h = 460
    row_hdr_h = 46
    row_gap = 14
    top_banner_h = 92

    n_rows = len(metas)
    sheet_h = top_banner_h + n_rows * (row_hdr_h + cell_h + row_gap) + 12

    sheet = np.empty((sheet_h, sheet_w, 3), dtype=np.uint8)
    sheet[:] = (16, 19, 25)
    sheet[0:top_banner_h, :] = (22, 27, 36)
    sheet[top_banner_h - 2 : top_banner_h, :] = (64, 118, 205)
    _draw_text(sheet, 24, 18, sheet_title, color=(240, 245, 255), scale=3)

    col_titles = [
        "1. ORIGINAL RA2 .SHP (SW 45) + CAMEO",
        "2. CYCLES PBR .GLB (45 YAW / 30 PITCH)",
        "3. CLOSE-UP 3D PBR + ARTICULATED POSE",
    ]
    for ci, ctitle in enumerate(col_titles):
        cx0 = pad_x + ci * (cell_w + col_gap)
        _draw_text(sheet, cx0 + 10, 58, ctitle, color=(155, 190, 240), scale=2)

    for ri, meta in enumerate(metas):
        glb_path = os.path.join(models_dir, f"{meta.stem}.glb")
        summary = verify_baked_glb(glb_path, max_lod0_verts=5800)
        team_srgb = FACTION_TEAM_SRGB.get(meta.sheet_faction, FACTION_TEAM_SRGB["allied"])

        ref_rgb = _render_infantry_ref_card(ra2_dir, meta, team_srgb, card_w=cell_w, card_h=cell_h)
        tac_rgb, hero_rgb = _render_infantry_glb_pair(glb_path, meta, cell_w=cell_w, cell_h=cell_h, samples=34)

        ry0 = top_banner_h + 8 + ri * (row_hdr_h + cell_h + row_gap)
        sheet[ry0 : ry0 + row_hdr_h - 4, pad_x : sheet_w - pad_x] = (26, 31, 42)
        sheet[ry0 : ry0 + row_hdr_h - 4, pad_x : pad_x + 8] = team_srgb
        _draw_text(sheet, pad_x + 20, ry0 + 8, f"{meta.display_title} ({meta.stem}.GLB)", color=(245, 248, 255), scale=2)
        stats_txt = (
            f"{meta.subtitle}  |  LOD0:{summary.lod0_verts}V  LOD1:{summary.lod1_verts}V  "
            f"TEX:{summary.tex_size}PX  A0:{summary.team_alpha0_pct}%  CLIPS:{len(summary.animation_clips)}"
        )
        _draw_text(sheet, pad_x + 20, ry0 + 26, stats_txt, color=(165, 178, 200), scale=1)

        cy0 = ry0 + row_hdr_h
        for ci, card in enumerate((ref_rgb, tac_rgb, hero_rgb)):
            cx0 = pad_x + ci * (cell_w + col_gap)
            sheet[cy0 - 1 : cy0 + cell_h + 1, cx0 - 1 : cx0 + cell_w + 1] = (52, 62, 78)
            sheet[cy0 : cy0 + cell_h, cx0 : cx0 + cell_w] = card

    os.makedirs(os.path.dirname(os.path.abspath(out_png_path)), exist_ok=True)
    with open(out_png_path, "wb") as f:
        f.write(encode_png_bytes(sheet))
    print(f"[w2_infantry_3d] Wrote {out_png_path} ({sheet_w}x{sheet_h}px)")
    return out_png_path


def verify_all_41_against_manifest(repo_root: str) -> List[GlbVerificationSummary]:
    manifest = load_wave2_manifest()
    expected = {a["stem"]: a for a in manifest["assets"] if a["bucket"] == "infantry_3d"}
    assert len(expected) == 41, f"Expected 41 infantry_3d entries in manifest, got {len(expected)}"
    assert set(INFANTRY_3D_BUILDERS.keys()) == set(expected.keys()), (
        f"Builder registry mismatch: missing={set(expected) - set(INFANTRY_3D_BUILDERS)}, "
        f"extra={set(INFANTRY_3D_BUILDERS) - set(expected)}"
    )

    models_dir = os.path.join(repo_root, "rust-ra2", "assets", "models")
    summaries: List[GlbVerificationSummary] = []

    for stem, entry in sorted(expected.items()):
        glb_path = os.path.join(models_dir, f"{stem}.glb")
        assert os.path.isfile(glb_path), f"Missing built GLB: {glb_path}"
        summary = verify_baked_glb(glb_path, max_lod0_verts=5800)
        assert summary.tex_size == int(entry["tex_size"]), f"{stem}: tex_size {summary.tex_size} != {entry['tex_size']}"
        assert summary.team_alpha0_pct > 0.2, f"{stem}: team_alpha0_pct={summary.team_alpha0_pct}% must be > 0.2%"
        assert list(summary.animation_clips) == list(entry["existing_animations"]), (
            f"{stem}: clips {summary.animation_clips} != {entry['existing_animations']}"
        )

        gltf, _ = parse_glb(open(glb_path, "rb").read())
        lod0_meshes = [
            nd["name"] for nd in gltf["nodes"]
            if "mesh" in nd and not nd.get("name", "").endswith("_LOD1")
        ]
        sockets = [
            nd["name"] for nd in gltf["nodes"]
            if "mesh" not in nd and not nd.get("children")
        ]
        assert set(lod0_meshes) == set(entry["existing_mesh_nodes"]), (
            f"{stem}: mesh nodes {lod0_meshes} != {entry['existing_mesh_nodes']}"
        )
        assert set(sockets) == set(entry["existing_socket_nodes"]), (
            f"{stem}: socket nodes {sockets} != {entry['existing_socket_nodes']}"
        )
        summaries.append(summary)

    print(
        f"[w2_infantry_3d] Verified all 41 GLBs against wave2_manifest.json! "
        f"LOD0 range: {min(s.lod0_verts for s in summaries)}..{max(s.lod0_verts for s in summaries)} verts"
    )
    return summaries


def main(argv: List[str]) -> None:
    parser = argparse.ArgumentParser(description="Build & verify all 41 Wave-2 articulated 3D infantry GLBs.")
    parser.add_argument("--repo-root", default=os.path.abspath(os.path.join(_TOOLS_DIR, "..")))
    parser.add_argument("--build-all", action="store_true")
    parser.add_argument("--only", default="", help="Comma-separated subset of stems to build")
    parser.add_argument("--render-sheets", action="store_true")
    parser.add_argument("--sheets", default="1,2,3,4", help="Comma-separated sheet indices 1..4")
    parser.add_argument("--verify-only", action="store_true")
    args = parser.parse_args(argv)

    repo_root = os.path.abspath(args.repo_root)
    models_dir = os.path.join(repo_root, "rust-ra2", "assets", "models")
    ra2_dir = os.path.join(repo_root, "rust-ra2")
    sheets_dir = os.path.join(repo_root, "docs", "model_sheets")

    backend = configure_cycles_gpu(ao_distance=0.55)
    print(f"[w2_infantry_3d] Cycles compute backend: {backend}")

    if args.build_all or args.only:
        stems = (
            [s.strip() for s in args.only.split(",") if s.strip()]
            if args.only
            else [m.stem for m in INFANTRY_3D_CATALOG]
        )
        t0 = time.time()
        for idx, stem in enumerate(stems, 1):
            t_s = time.time()
            out_glb = os.path.join(models_dir, f"{stem}.glb")
            summary = INFANTRY_3D_BUILDERS[stem](out_glb)
            dt = time.time() - t_s
            dims = tuple(round(summary.bounds_max[k] - summary.bounds_min[k], 3) for k in range(3))
            print(
                f"  [{idx:2d}/{len(stems):2d}] {stem:20s} -> LOD0={summary.lod0_verts:4d}v "
                f"LOD1={summary.lod1_verts:4d}v A0={summary.team_alpha0_pct:5.2f}% "
                f"dims={dims} ({dt:.2f}s)"
            )
        print(f"[w2_infantry_3d] Built {len(stems)} GLBs in {time.time() - t0:.1f}s")

    if args.build_all or args.verify_only:
        verify_all_41_against_manifest(repo_root)

    if args.render_sheets:
        want = {int(x.strip()) for x in args.sheets.split(",") if x.strip()}
        for idx, (fname, title, metas) in enumerate(SHEET_SPLITS, 1):
            if idx in want:
                render_infantry_sheet(
                    ra2_dir=ra2_dir,
                    models_dir=models_dir,
                    out_png_path=os.path.join(sheets_dir, fname),
                    metas=metas,
                    sheet_title=title,
                )


if __name__ == "__main__":
    cli_argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    main(cli_argv)
