use super::{PreviewStageState, PREVIEW_STAGE_HEIGHT, PREVIEW_STAGE_WIDTH};
use crate::ModResource;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};

/// Allocates the `512x384` RGBA8 offscreen render target `Image` with
/// `RENDER_ATTACHMENT | TEXTURE_BINDING | COPY_SRC` usages for `egui` & GPU readback.
pub fn build_preview_stage_render_target_image() -> Image {
    let pixel_count = (PREVIEW_STAGE_WIDTH * PREVIEW_STAGE_HEIGHT) as usize;
    let mut initial_rgba = vec![0u8; pixel_count * 4];
    for y in 0..PREVIEW_STAGE_HEIGHT {
        let v = y as f32 / PREVIEW_STAGE_HEIGHT.max(1) as f32;
        let r = (16.0 + (1.0 - v) * 14.0) as u8;
        let g = (22.0 + (1.0 - v) * 18.0) as u8;
        let b = (32.0 + (1.0 - v) * 24.0) as u8;
        for x in 0..PREVIEW_STAGE_WIDTH {
            let idx = ((y * PREVIEW_STAGE_WIDTH + x) as usize) * 4;
            initial_rgba[idx] = r;
            initial_rgba[idx + 1] = g;
            initial_rgba[idx + 2] = b;
            initial_rgba[idx + 3] = 255;
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: PREVIEW_STAGE_WIDTH,
            height: PREVIEW_STAGE_HEIGHT,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        initial_rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC;
    image
}

/// Builds a metric `1m` studio floor grid + pedestal mesh (`8m x 8m`) for the isolated preview stage.
pub(super) fn build_preview_floor_grid_mesh() -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let mut push_quad = |x0: f32, z0: f32, x1: f32, z1: f32, y: f32, col: [f32; 4]| {
        let base = positions.len() as u32;
        positions.extend_from_slice(&[[x0, y, z0], [x1, y, z0], [x1, y, z1], [x0, y, z1]]);
        normals.extend_from_slice(&[[0.0, 1.0, 0.0]; 4]);
        colors.extend_from_slice(&[col; 4]);
        indices.extend_from_slice(&[base, base + 3, base + 1, base + 1, base + 3, base + 2]);
    };

    push_quad(-4.0, -4.0, 4.0, 4.0, -0.02, [0.11, 0.14, 0.19, 1.0]);

    for i in -4i32..=4i32 {
        let f = i as f32;
        let half_t = if i == 0 { 0.024 } else { 0.011 };
        let col_x = if i == 0 {
            [0.24, 0.62, 0.92, 1.0]
        } else {
            [0.22, 0.28, 0.36, 1.0]
        };
        let col_z = if i == 0 {
            [0.90, 0.34, 0.32, 1.0]
        } else {
            [0.22, 0.28, 0.36, 1.0]
        };
        push_quad(-4.0, f - half_t, 4.0, f + half_t, -0.005, col_x);
        push_quad(f - half_t, -4.0, f + half_t, 4.0, -0.004, col_z);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Builds a 3-axis RGB cross gizmo mesh rendered on `RenderLayers::layer(3)` for socket inspection.
pub(super) fn build_socket_cross_mesh() -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let mut add_box = |min: [f32; 3], max: [f32; 3], col: [f32; 4]| {
        let [x0, y0, z0] = min;
        let [x1, y1, z1] = max;
        let faces: [([[f32; 3]; 4], [f32; 3]); 6] = [
            (
                [[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]],
                [0.0, 1.0, 0.0],
            ),
            (
                [[x0, y0, z1], [x1, y0, z1], [x1, y0, z0], [x0, y0, z0]],
                [0.0, -1.0, 0.0],
            ),
            (
                [[x0, y0, z1], [x0, y1, z1], [x1, y1, z1], [x1, y0, z1]],
                [0.0, 0.0, 1.0],
            ),
            (
                [[x1, y0, z0], [x1, y1, z0], [x0, y1, z0], [x0, y0, z0]],
                [0.0, 0.0, -1.0],
            ),
            (
                [[x1, y0, z1], [x1, y1, z1], [x1, y1, z0], [x1, y0, z0]],
                [1.0, 0.0, 0.0],
            ),
            (
                [[x0, y0, z0], [x0, y1, z0], [x0, y1, z1], [x0, y0, z1]],
                [-1.0, 0.0, 0.0],
            ),
        ];
        for (verts, n) in faces {
            let base = positions.len() as u32;
            positions.extend_from_slice(&verts);
            normals.extend_from_slice(&[n; 4]);
            colors.extend_from_slice(&[col; 4]);
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    };

    let r = 0.018;
    let arm = 0.22;
    add_box([-arm, -r, -r], [arm, r, r], [1.0, 0.28, 0.28, 1.0]);
    add_box([-r, -arm, -r], [r, arm, r], [0.28, 1.0, 0.42, 1.0]);
    add_box([-r, -r, -arm], [r, r, arm], [0.28, 0.72, 1.0, 1.0]);

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Handles `[📸 Capture Cameo PNG]` requests (`96x72` RGBA PNG) via `2x2` SSAA `.glb` baking.
pub(super) fn process_pending_cameo_capture(
    state: &mut PreviewStageState,
    mod_res: Option<&ModResource>,
    images: Option<&Assets<Image>>,
) {
    let Some((req_id, raw_out_path)) = state.capture_cameo_request.take() else {
        return;
    };
    let clean_id = req_id.trim();
    let out_path = if raw_out_path.as_os_str().is_empty() {
        let fname = if clean_id.is_empty() {
            "captured_cameo.png".to_string()
        } else {
            format!("{clean_id}.png")
        };
        if let Some(mr) = mod_res {
            mr.0.root_dir.join("assets/ui/cameos").join(fname)
        } else {
            std::path::PathBuf::from("assets/ui/cameos").join(fname)
        }
    } else if raw_out_path.is_absolute()
        || mod_res.is_some_and(|mr| raw_out_path.starts_with(&mr.0.root_dir))
    {
        raw_out_path
    } else if let Some(mr) = mod_res {
        mr.0.root_dir.join(&raw_out_path)
    } else {
        raw_out_path
    };

    let mut scale = [1.0_f32; 3];
    let mut prim_col = state.faction_tint_rgb;
    let mut tint = true;
    let mut is_bldg = false;
    let mut model_candidates: Vec<String> = Vec::new();

    if let Some(mr) = mod_res {
        if let Some(u) = mr.0.units.get(clean_id) {
            scale = u.visual.scale;
            prim_col = u.visual.primary_color;
            tint = u.visual.team_color_tint;
            is_bldg = false;
            if !u.visual.model.trim().is_empty() {
                model_candidates.push(u.visual.model.clone());
            }
        } else if let Some(b) = mr.0.buildings.get(clean_id) {
            scale = b.visual.scale;
            prim_col = b.visual.primary_color;
            tint = b.visual.team_color_tint;
            is_bldg = true;
            if !b.visual.model.trim().is_empty() {
                model_candidates.push(b.visual.model.clone());
            }
        }
    }
    if let Some(ref inspected) = state.inspected_model_path {
        if !inspected.trim().is_empty() {
            model_candidates.push(inspected.clone());
        }
    }
    if clean_id.to_ascii_lowercase().ends_with(".glb") {
        model_candidates.push(clean_id.to_string());
    }

    let mut glb_bytes_opt: Option<Vec<u8>> = None;
    for cand in &model_candidates {
        if let Some(mr) = mod_res {
            if let Some(bytes) = mr.0.read_asset_bytes(cand) {
                glb_bytes_opt = Some(bytes);
                break;
            }
            let disk = mr.0.resolve_asset_path(cand);
            if let Ok(bytes) = std::fs::read(&disk) {
                glb_bytes_opt = Some(bytes);
                break;
            }
        }
        if let Ok(bytes) = std::fs::read(cand) {
            glb_bytes_opt = Some(bytes);
            break;
        }
    }

    let png_result: Result<Vec<u8>, String> = if let Some(glb_bytes) = glb_bytes_opt {
        rts_asset_pipeline::cameo_renderer::render_glb_to_cameo_png(
            &glb_bytes, scale, prim_col, tint, is_bldg,
        )
        .map_err(|e| e.to_string())
    } else {
        let cw = rts_asset_pipeline::cameo_renderer::CAMEO_WIDTH as usize;
        let ch = rts_asset_pipeline::cameo_renderer::CAMEO_HEIGHT as usize;
        let mut rgba = vec![0u8; cw * ch * 4];
        if let Some(img) = images.and_then(|imgs| imgs.get(&state.preview_image)) {
            let sw = img.texture_descriptor.size.width.max(1) as usize;
            let sh = img.texture_descriptor.size.height.max(1) as usize;
            if img.data.len() >= sw * sh * 4 {
                for oy in 0..ch {
                    let sy = (oy * sh / ch).min(sh - 1);
                    for ox in 0..cw {
                        let sx = (ox * sw / cw).min(sw - 1);
                        let src = (sy * sw + sx) * 4;
                        let dst = (oy * cw + ox) * 4;
                        rgba[dst..dst + 4].copy_from_slice(&img.data[src..src + 4]);
                    }
                }
            }
        } else {
            for oy in 0..ch {
                for ox in 0..cw {
                    let dst = (oy * cw + ox) * 4;
                    rgba[dst] = (prim_col[0].clamp(0.0, 1.0) * 180.0 + 24.0) as u8;
                    rgba[dst + 1] = (prim_col[1].clamp(0.0, 1.0) * 180.0 + 30.0) as u8;
                    rgba[dst + 2] = (prim_col[2].clamp(0.0, 1.0) * 180.0 + 42.0) as u8;
                    rgba[dst + 3] = 255;
                }
            }
        }
        rts_asset_pipeline::westwood::encode_rgba_to_png(cw as u32, ch as u32, &rgba)
            .map_err(|e| e.to_string())
    };

    match png_result {
        Ok(png_bytes) => {
            if let Some(parent) = out_path.parent() {
                if let Err(err) = std::fs::create_dir_all(parent) {
                    state.last_cameo_capture_status = Some(format!(
                        "Cameo capture failed creating directory {}: {err}",
                        parent.display()
                    ));
                    return;
                }
            }
            match std::fs::write(&out_path, &png_bytes) {
                Ok(()) => {
                    state.last_cameo_capture_status = Some(format!(
                        "Saved 96x72 cameo PNG ({} B) -> {}",
                        png_bytes.len(),
                        out_path.display()
                    ));
                }
                Err(err) => {
                    state.last_cameo_capture_status = Some(format!(
                        "Cameo capture write failed ({}): {err}",
                        out_path.display()
                    ));
                }
            }
        }
        Err(err) => {
            state.last_cameo_capture_status = Some(format!("Cameo bake failed: {err}"));
        }
    }
}
