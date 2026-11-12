use super::{
    decode_png_to_rgba8, encode_rgba8_to_ktx2, infer_texture_color_space, mip_level_count,
    Ktx2ColorSpace, MipFilter,
};
use crate::cameo_renderer::{render_glb_to_cameo_png, CAMEO_HEIGHT, CAMEO_WIDTH};
use crate::westwood::encode_rgba_to_png;
use anyhow::{bail, Context, Result};
use rts_sdk::archive::{normalize_archive_rel_path, should_exclude_from_archive};
use rts_sdk::loader::LoadedMod;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Relative archive path for the packed RGBA8 PNG cameo sprite atlas.
pub const CAMEO_ATLAS_PNG_REL: &str = "cameos/cameo_atlas.png";
/// Relative archive path for the mipmapped sRGB KTX2 cameo sprite atlas.
pub const CAMEO_ATLAS_KTX2_REL: &str = "cameos/cameo_atlas.ktx2";
/// Relative archive path for the JSON cameo sprite atlas UV/rect table.
pub const CAMEO_ATLAS_JSON_REL: &str = "cameos/cameo_atlas.json";

/// Input sprite provided to [`pack_cameo_atlas`].
#[derive(Debug, Clone)]
pub struct CameoSpriteInput {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Pixel rectangle and normalized `[0.0..=1.0]` UV coordinates of a single cameo in the atlas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameoAtlasEntry {
    pub id: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
}

/// Serializable JSON metadata written to `cameos/cameo_atlas.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameoAtlasManifest {
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub mip_levels: u32,
    pub padding_px: u32,
    pub png_path: String,
    pub ktx2_path: String,
    pub entries: BTreeMap<String, CameoAtlasEntry>,
}

/// Complete in-memory output of [`pack_cameo_atlas`].
#[derive(Debug, Clone)]
pub struct PackedCameoAtlas {
    pub manifest: CameoAtlasManifest,
    pub png_bytes: Vec<u8>,
    pub ktx2_bytes: Vec<u8>,
    pub json_bytes: Vec<u8>,
}

/// Summary of `.ktx2` mipmapped textures and cameo atlas artifacts cooked into a staged mod folder.
#[derive(Debug, Clone, Default)]
pub struct TextureCookReport {
    pub cooked_ktx2_files: Vec<String>,
    pub synthesized_cameo_pngs: Vec<String>,
    pub packed_cameo_count: usize,
    pub atlas_dimensions: (u32, u32),
}

#[inline]
fn round_up_multiple_of_4(v: u32) -> u32 {
    let v = v.max(4);
    (v + 3) & !3
}

fn blit_sprite_with_extruded_gutter(
    atlas: &mut [u8],
    atlas_w: u32,
    atlas_h: u32,
    dst_x: u32,
    dst_y: u32,
    sprite: &CameoSpriteInput,
    extrude_px: u32,
) {
    let sw = sprite.width as usize;
    let sh = sprite.height as usize;
    let aw = atlas_w as usize;

    for sy in 0..sh {
        let src_off = sy * sw * 4;
        let dst_off = ((dst_y as usize + sy) * aw + dst_x as usize) * 4;
        atlas[dst_off..dst_off + sw * 4].copy_from_slice(&sprite.rgba[src_off..src_off + sw * 4]);
    }

    if extrude_px == 0 {
        return;
    }

    // Extrude 1..extrude_px border texels outward to prevent bilinear/mipmap bleeding
    let min_gx = dst_x.saturating_sub(extrude_px);
    let max_gx = (dst_x + sprite.width + extrude_px).min(atlas_w);
    let min_gy = dst_y.saturating_sub(extrude_px);
    let max_gy = (dst_y + sprite.height + extrude_px).min(atlas_h);

    for ay in min_gy..max_gy {
        for ax in min_gx..max_gx {
            if ax >= dst_x && ax < dst_x + sprite.width && ay >= dst_y && ay < dst_y + sprite.height
            {
                continue;
            }
            let clamped_sx = ax
                .clamp(dst_x, dst_x + sprite.width - 1)
                .saturating_sub(dst_x) as usize;
            let clamped_sy = ay
                .clamp(dst_y, dst_y + sprite.height - 1)
                .saturating_sub(dst_y) as usize;
            let src_idx = (clamped_sy * sw + clamped_sx) * 4;
            let dst_idx = (ay as usize * aw + ax as usize) * 4;
            atlas[dst_idx..dst_idx + 4].copy_from_slice(&sprite.rgba[src_idx..src_idx + 4]);
        }
    }
}

/// Deterministically packs cameo icons (sorted lexicographically by `id`) into a padded shelf
/// sprite atlas and encodes `cameo_atlas.png`, mipmapped `cameo_atlas.ktx2`, and `cameo_atlas.json`.
pub fn pack_cameo_atlas(sprites: &[CameoSpriteInput], padding_px: u32) -> Result<PackedCameoAtlas> {
    let mut sorted: BTreeMap<String, &CameoSpriteInput> = BTreeMap::new();
    for sp in sprites {
        if sp.id.trim().is_empty() {
            continue;
        }
        if sp.width == 0 || sp.height == 0 {
            bail!("Cameo sprite '{}' has zero dimensions", sp.id);
        }
        let expected = (sp.width as usize) * (sp.height as usize) * 4;
        if sp.rgba.len() != expected {
            bail!(
                "Cameo sprite '{}' RGBA buffer length mismatch: expected {}, got {}",
                sp.id,
                expected,
                sp.rgba.len()
            );
        }
        sorted.insert(sp.id.clone(), sp);
    }

    let gutter = padding_px.max(1);
    if sorted.is_empty() {
        let (atlas_width, atlas_height) = (4u32, 4u32);
        let rgba = vec![0u8; (atlas_width * atlas_height * 4) as usize];
        let png_bytes = encode_rgba_to_png(atlas_width, atlas_height, &rgba)?;
        let ktx2_bytes = encode_rgba8_to_ktx2(
            atlas_width,
            atlas_height,
            &rgba,
            Ktx2ColorSpace::Srgb,
            MipFilter::Lanczos3,
        )?;
        let manifest = CameoAtlasManifest {
            atlas_width,
            atlas_height,
            mip_levels: mip_level_count(atlas_width, atlas_height),
            padding_px: gutter,
            png_path: CAMEO_ATLAS_PNG_REL.to_string(),
            ktx2_path: CAMEO_ATLAS_KTX2_REL.to_string(),
            entries: BTreeMap::new(),
        };
        let json_bytes = serde_json::to_vec_pretty(&manifest)?;
        return Ok(PackedCameoAtlas {
            manifest,
            png_bytes,
            ktx2_bytes,
            json_bytes,
        });
    }

    let total_area: u64 = sorted
        .values()
        .map(|s| (s.width + gutter * 2) as u64 * (s.height + gutter * 2) as u64)
        .sum();
    let max_single_w = sorted
        .values()
        .map(|s| s.width + gutter * 2)
        .max()
        .unwrap_or(CAMEO_WIDTH + gutter * 2);
    let target_w = round_up_multiple_of_4(
        ((total_area as f64).sqrt().ceil() as u32)
            .max(max_single_w)
            .next_power_of_two()
            .min(4096)
            .max(max_single_w),
    );

    struct Placement<'a> {
        id: String,
        x: u32,
        y: u32,
        sprite: &'a CameoSpriteInput,
    }

    let mut placements = Vec::with_capacity(sorted.len());
    let mut cursor_x = gutter;
    let mut cursor_y = gutter;
    let mut row_h = 0u32;
    let mut used_w = 0u32;

    for (id, sp) in &sorted {
        let cell_w = sp.width + gutter * 2;
        if cursor_x + sp.width + gutter > target_w && cursor_x > gutter {
            cursor_x = gutter;
            cursor_y += row_h + gutter * 2;
            row_h = 0;
        }
        placements.push(Placement {
            id: id.clone(),
            x: cursor_x,
            y: cursor_y,
            sprite: sp,
        });
        used_w = used_w.max(cursor_x + sp.width + gutter);
        cursor_x += cell_w;
        row_h = row_h.max(sp.height);
    }

    let atlas_width = round_up_multiple_of_4(used_w.max(max_single_w));
    let atlas_height = round_up_multiple_of_4(cursor_y + row_h + gutter);
    let mut atlas_rgba = vec![0u8; (atlas_width as usize) * (atlas_height as usize) * 4];
    let mut entries = BTreeMap::new();

    for p in &placements {
        blit_sprite_with_extruded_gutter(
            &mut atlas_rgba,
            atlas_width,
            atlas_height,
            p.x,
            p.y,
            p.sprite,
            gutter.min(2),
        );
        let uv_min = [
            p.x as f32 / atlas_width as f32,
            p.y as f32 / atlas_height as f32,
        ];
        let uv_max = [
            (p.x + p.sprite.width) as f32 / atlas_width as f32,
            (p.y + p.sprite.height) as f32 / atlas_height as f32,
        ];
        entries.insert(
            p.id.clone(),
            CameoAtlasEntry {
                id: p.id.clone(),
                x: p.x,
                y: p.y,
                width: p.sprite.width,
                height: p.sprite.height,
                uv_min,
                uv_max,
            },
        );
    }

    let png_bytes = encode_rgba_to_png(atlas_width, atlas_height, &atlas_rgba)?;
    let ktx2_bytes = encode_rgba8_to_ktx2(
        atlas_width,
        atlas_height,
        &atlas_rgba,
        Ktx2ColorSpace::Srgb,
        MipFilter::Lanczos3,
    )?;
    let manifest = CameoAtlasManifest {
        atlas_width,
        atlas_height,
        mip_levels: mip_level_count(atlas_width, atlas_height),
        padding_px: gutter,
        png_path: CAMEO_ATLAS_PNG_REL.to_string(),
        ktx2_path: CAMEO_ATLAS_KTX2_REL.to_string(),
        entries,
    };
    let json_bytes = serde_json::to_vec_pretty(&manifest)?;

    Ok(PackedCameoAtlas {
        manifest,
        png_bytes,
        ktx2_bytes,
        json_bytes,
    })
}

fn collect_png_files_recursive(root: &Path, current: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !current.is_dir() {
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(current)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();

    for path in entries {
        let rel =
            normalize_archive_rel_path(&path.strip_prefix(root).unwrap_or(&path).to_string_lossy());
        if rel.is_empty() || should_exclude_from_archive(&rel) {
            continue;
        }
        if path.is_dir() {
            collect_png_files_recursive(root, &path, out)?;
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("png"))
        {
            out.push(path);
        }
    }
    Ok(())
}

/// Cooks `.ktx2` mipmapped textures for every `.png` texture in `stage_dir` (preserving `.png`
/// files for VFS backwards compatibility), synthesizes missing cameo icons from local `.glb` models,
/// and packs `cameos/cameo_atlas.{png,ktx2,json}`. Never modifies `mod.toml`.
pub fn cook_staged_mod_textures_and_atlas(
    stage_dir: &Path,
    loaded: &LoadedMod,
) -> Result<TextureCookReport> {
    let mut report = TextureCookReport::default();
    let mut cameo_inputs: BTreeMap<String, CameoSpriteInput> = BTreeMap::new();

    // 1. Gather or render cameos for units & buildings whose icon or .glb lives in `stage_dir`
    struct EntityVisualRef<'a> {
        id: &'a str,
        is_building: bool,
        model: &'a str,
        icon: Option<&'a str>,
        scale: [f32; 3],
        primary_color: [f32; 3],
        team_color_tint: bool,
    }

    let mut entity_refs: Vec<EntityVisualRef<'_>> = Vec::new();
    for (id, b) in &loaded.buildings {
        entity_refs.push(EntityVisualRef {
            id,
            is_building: true,
            model: &b.visual.model,
            icon: b.visual.icon.as_deref(),
            scale: b.visual.scale,
            primary_color: b.visual.primary_color,
            team_color_tint: b.visual.team_color_tint,
        });
    }
    for (id, u) in &loaded.units {
        entity_refs.push(EntityVisualRef {
            id,
            is_building: false,
            model: &u.visual.model,
            icon: u.visual.icon.as_deref(),
            scale: u.visual.scale,
            primary_color: u.visual.primary_color,
            team_color_tint: u.visual.team_color_tint,
        });
    }

    for ent in entity_refs {
        let candidate_rels: Vec<String> = {
            let mut v = Vec::new();
            if let Some(ic) = ent.icon.map(str::trim).filter(|s| !s.is_empty()) {
                v.push(normalize_archive_rel_path(ic));
            }
            v.push(format!("assets/ui/cameos/{}_icon.png", ent.id));
            v.push(format!("assets/ui/cameos/{}.png", ent.id));
            v
        };

        let mut loaded_icon = false;
        for rel in &candidate_rels {
            let local_png = stage_dir.join(rel);
            if local_png.is_file() {
                if let Ok(png_bytes) = fs::read(&local_png) {
                    if let Ok((w, h, rgba)) = decode_png_to_rgba8(&png_bytes) {
                        cameo_inputs.insert(
                            ent.id.to_string(),
                            CameoSpriteInput {
                                id: ent.id.to_string(),
                                width: w,
                                height: h,
                                rgba,
                            },
                        );
                        loaded_icon = true;
                        break;
                    }
                }
            }
        }

        if !loaded_icon && !ent.model.trim().is_empty() {
            let local_glb = stage_dir.join(normalize_archive_rel_path(ent.model));
            if local_glb.is_file() {
                if let Ok(glb_bytes) = fs::read(&local_glb) {
                    if let Ok(png_bytes) = render_glb_to_cameo_png(
                        &glb_bytes,
                        ent.scale,
                        ent.primary_color,
                        ent.team_color_tint,
                        ent.is_building,
                    ) {
                        let synth_rel = format!("assets/ui/cameos/{}_icon.png", ent.id);
                        let synth_disk = stage_dir.join(&synth_rel);
                        if let Some(parent) = synth_disk.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::write(&synth_disk, &png_bytes)?;
                        report.synthesized_cameo_pngs.push(synth_rel);
                        let (w, h, rgba) = decode_png_to_rgba8(&png_bytes).unwrap_or_else(|_| {
                            (
                                CAMEO_WIDTH,
                                CAMEO_HEIGHT,
                                vec![0u8; (CAMEO_WIDTH * CAMEO_HEIGHT * 4) as usize],
                            )
                        });
                        cameo_inputs.insert(
                            ent.id.to_string(),
                            CameoSpriteInput {
                                id: ent.id.to_string(),
                                width: w,
                                height: h,
                                rgba,
                            },
                        );
                    }
                }
            }
        }
    }

    // 2. Also pick up any standalone cameo PNGs in `assets/ui/cameos/` not yet registered by entity ID
    let cameos_dir = stage_dir.join("assets/ui/cameos");
    if cameos_dir.is_dir() {
        let mut extra_pngs = Vec::new();
        collect_png_files_recursive(stage_dir, &cameos_dir, &mut extra_pngs)?;
        for png_path in extra_pngs {
            let Some(stem) = png_path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let clean_id = stem.strip_suffix("_icon").unwrap_or(stem);
            if cameo_inputs.contains_key(clean_id) || clean_id == "cameo_atlas" {
                continue;
            }
            if let Ok(png_bytes) = fs::read(&png_path) {
                if let Ok((w, h, rgba)) = decode_png_to_rgba8(&png_bytes) {
                    cameo_inputs.insert(
                        clean_id.to_string(),
                        CameoSpriteInput {
                            id: clean_id.to_string(),
                            width: w,
                            height: h,
                            rgba,
                        },
                    );
                }
            }
        }
    }

    // 3. Pack and write `cameos/cameo_atlas.{png,ktx2,json}`
    let sprite_vec: Vec<CameoSpriteInput> = cameo_inputs.into_values().collect();
    let packed_atlas = pack_cameo_atlas(&sprite_vec, 2)?;
    let atlas_png_disk = stage_dir.join(CAMEO_ATLAS_PNG_REL);
    let atlas_ktx2_disk = stage_dir.join(CAMEO_ATLAS_KTX2_REL);
    let atlas_json_disk = stage_dir.join(CAMEO_ATLAS_JSON_REL);
    if let Some(parent) = atlas_png_disk.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&atlas_png_disk, &packed_atlas.png_bytes)?;
    fs::write(&atlas_ktx2_disk, &packed_atlas.ktx2_bytes)?;
    fs::write(&atlas_json_disk, &packed_atlas.json_bytes)?;

    report.packed_cameo_count = packed_atlas.manifest.entries.len();
    report.atlas_dimensions = (
        packed_atlas.manifest.atlas_width,
        packed_atlas.manifest.atlas_height,
    );

    let mut cooked_set = BTreeSet::new();
    cooked_set.insert(CAMEO_ATLAS_KTX2_REL.to_string());

    // 4. Bake mipmapped `.ktx2` siblings for every `.png` texture in `stage_dir`
    let mut all_pngs = Vec::new();
    collect_png_files_recursive(stage_dir, stage_dir, &mut all_pngs)?;
    for png_path in all_pngs {
        let rel_png = normalize_archive_rel_path(
            &png_path
                .strip_prefix(stage_dir)
                .unwrap_or(&png_path)
                .to_string_lossy(),
        );
        if rel_png == CAMEO_ATLAS_PNG_REL {
            continue;
        }
        let png_bytes = fs::read(&png_path)
            .with_context(|| format!("Failed to read PNG {}", png_path.display()))?;
        let (w, h, rgba) = decode_png_to_rgba8(&png_bytes)
            .with_context(|| format!("Invalid PNG texture '{}' during KTX2 cook", rel_png))?;
        let color_space = infer_texture_color_space(&rel_png);
        let ktx2_bytes = encode_rgba8_to_ktx2(w, h, &rgba, color_space, MipFilter::Lanczos3)?;
        let ktx2_path = png_path.with_extension("ktx2");
        fs::write(&ktx2_path, &ktx2_bytes)
            .with_context(|| format!("Failed to write cooked KTX2 {}", ktx2_path.display()))?;
        let rel_ktx2 = normalize_archive_rel_path(
            &ktx2_path
                .strip_prefix(stage_dir)
                .unwrap_or(&ktx2_path)
                .to_string_lossy(),
        );
        cooked_set.insert(rel_ktx2);
    }

    report.cooked_ktx2_files = cooked_set.into_iter().collect();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture_cooker::{inspect_ktx2_header, VK_FORMAT_R8G8B8A8_SRGB};

    #[test]
    fn packs_deterministic_cameo_atlas_with_non_overlapping_rects_and_valid_uvs() {
        let make_sprite = |id: &str, r: u8, g: u8, b: u8| CameoSpriteInput {
            id: id.to_string(),
            width: 96,
            height: 72,
            rgba: [r, g, b, 255].repeat(96 * 72),
        };

        let inputs_a = vec![
            make_sprite("z_mammoth", 200, 40, 40),
            make_sprite("a_grizzly", 40, 120, 220),
            make_sprite("m_prism", 120, 210, 255),
        ];
        let mut inputs_b = inputs_a.clone();
        inputs_b.reverse();

        let atlas_a = pack_cameo_atlas(&inputs_a, 2).expect("pack_cameo_atlas A");
        let atlas_b = pack_cameo_atlas(&inputs_b, 2).expect("pack_cameo_atlas B");

        // Deterministic regardless of input order
        assert_eq!(atlas_a.manifest, atlas_b.manifest);
        assert_eq!(atlas_a.png_bytes, atlas_b.png_bytes);
        assert_eq!(atlas_a.ktx2_bytes, atlas_b.ktx2_bytes);
        assert_eq!(atlas_a.manifest.entries.len(), 3);

        // Verify KTX2 header & full mipmap chain
        let ktx_info = inspect_ktx2_header(&atlas_a.ktx2_bytes).expect("valid atlas KTX2");
        assert_eq!(ktx_info.vk_format, VK_FORMAT_R8G8B8A8_SRGB);
        assert_eq!(ktx_info.width, atlas_a.manifest.atlas_width);
        assert_eq!(ktx_info.height, atlas_a.manifest.atlas_height);
        assert_eq!(ktx_info.level_count, atlas_a.manifest.mip_levels);
        assert_eq!(
            ktx_info.levels.last().map(|l| (l.width, l.height)),
            Some((1, 1))
        );

        // Verify UV bounds & pairwise non-overlap (including gutter)
        let rects: Vec<&CameoAtlasEntry> = atlas_a.manifest.entries.values().collect();
        for (i, r1) in rects.iter().enumerate() {
            assert!(r1.uv_min[0] >= 0.0 && r1.uv_min[1] >= 0.0);
            assert!(r1.uv_max[0] <= 1.0 && r1.uv_max[1] <= 1.0);
            assert!(r1.uv_min[0] < r1.uv_max[0] && r1.uv_min[1] < r1.uv_max[1]);
            for r2 in rects.iter().skip(i + 1) {
                let separated = r1.x + r1.width < r2.x
                    || r2.x + r2.width < r1.x
                    || r1.y + r1.height < r2.y
                    || r2.y + r2.height < r1.y;
                assert!(separated, "Atlas rects overlapped: {r1:?} vs {r2:?}");
            }
        }
    }
}
