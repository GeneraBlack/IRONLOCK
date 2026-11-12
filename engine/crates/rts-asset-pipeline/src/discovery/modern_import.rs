use crate::cameo_renderer::render_glb_to_cameo_png;
use crate::glb_optimizer::{inspect_glb_bytes, optimize_glb_bytes, parse_glb_chunks};
use crate::westwood::{convert_westwood_wav_to_pcm16, normalize_id};
use anyhow::{bail, Context, Result};
use rts_sdk::schema::{SocketOffset, VisualArchetype, VisualDef};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModernAssetKind {
    GlbModel,
    PngImage,
    OggAudio,
    FlacAudio,
    WavAudio,
    Mp4Video,
}

impl ModernAssetKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::GlbModel => "glTF 2.0 Binary 3D Model (.glb)",
            Self::PngImage => "PNG RGBA Texture / Cameo (.png)",
            Self::OggAudio => "Ogg Vorbis Audio (.ogg)",
            Self::FlacAudio => "FLAC Lossless Audio (.flac)",
            Self::WavAudio => "RIFF PCM Wave Audio (.wav)",
            Self::Mp4Video => "MP4 Video Cutscene (.mp4)",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModernAssetImportOptions {
    pub asset_id: Option<String>,
    pub as_building: bool,
    pub bake_cameo: bool,
    pub optimize_glb: bool,
}

impl Default for ModernAssetImportOptions {
    fn default() -> Self {
        Self {
            asset_id: None,
            as_building: false,
            bake_cameo: true,
            optimize_glb: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModernAssetImportReport {
    pub asset_id: String,
    pub kind: ModernAssetKind,
    pub destination_path: PathBuf,
    pub relative_asset_path: String,
    pub bounds_min: Option<[f32; 3]>,
    pub bounds_max: Option<[f32; 3]>,
    pub suggested_footprint: Option<[u32; 2]>,
    pub animation_clips: Vec<String>,
    pub detected_sockets: Vec<(String, [f32; 3])>,
    pub visual_def: Option<VisualDef>,
    pub baked_cameo_path: Option<PathBuf>,
    pub toml_snippet: String,
}

fn classify_modern_extension(path: &Path) -> Result<ModernAssetKind> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "glb" => Ok(ModernAssetKind::GlbModel),
        "png" => Ok(ModernAssetKind::PngImage),
        "ogg" => Ok(ModernAssetKind::OggAudio),
        "flac" => Ok(ModernAssetKind::FlacAudio),
        "wav" => Ok(ModernAssetKind::WavAudio),
        "mp4" => Ok(ModernAssetKind::Mp4Video),
        _ => bail!(
            "Unsupported modern asset extension '.{ext}' for {} (expected .glb, .png, .ogg, .flac, .wav, .mp4)",
            path.display()
        ),
    }
}

fn validate_modern_asset_bytes(kind: ModernAssetKind, raw: &[u8], path: &Path) -> Result<Vec<u8>> {
    match kind {
        ModernAssetKind::GlbModel => {
            let _ = parse_glb_chunks(raw)
                .with_context(|| format!("Invalid binary glTF 2.0 (.glb) in {}", path.display()))?;
            Ok(raw.to_vec())
        }
        ModernAssetKind::PngImage => {
            if raw.len() < 24 || &raw[0..8] != b"\x89PNG\r\n\x1a\n" || &raw[12..16] != b"IHDR" {
                bail!("Invalid PNG header in {}", path.display());
            }
            Ok(raw.to_vec())
        }
        ModernAssetKind::OggAudio => {
            if raw.len() < 4 || &raw[0..4] != b"OggS" {
                bail!("Invalid Ogg Vorbis ('OggS') header in {}", path.display());
            }
            Ok(raw.to_vec())
        }
        ModernAssetKind::FlacAudio => {
            if raw.len() < 4 || &raw[0..4] != b"fLaC" {
                bail!("Invalid FLAC ('fLaC') header in {}", path.display());
            }
            Ok(raw.to_vec())
        }
        ModernAssetKind::WavAudio => {
            if raw.len() < 12 || &raw[0..4] != b"RIFF" || &raw[8..12] != b"WAVE" {
                bail!("Invalid RIFF/WAVE header in {}", path.display());
            }
            if let Ok(pcm16) = convert_westwood_wav_to_pcm16(raw) {
                Ok(pcm16)
            } else {
                Ok(raw.to_vec())
            }
        }
        ModernAssetKind::Mp4Video => {
            if raw.len() < 8 {
                bail!("MP4 container too short (< 8 bytes) in {}", path.display());
            }
            let box_type = &raw[4..8];
            let valid_box = matches!(
                box_type,
                b"ftyp" | b"moov" | b"mdat" | b"free" | b"skip" | b"wide" | b"styp"
            );
            if !valid_box {
                bail!(
                    "Invalid ISO BMFF / MP4 top-level box {:?} in {}",
                    String::from_utf8_lossy(box_type),
                    path.display()
                );
            }
            Ok(raw.to_vec())
        }
    }
}

fn round3(v: f32) -> f32 {
    (v * 1000.0).round() / 1000.0
}

/// Imports a modern `.glb`, `.png`, `.ogg`, `.flac`, `.wav`, or `.mp4` asset into a mod directory,
/// auto-inspecting `.glb` binary chunks to extract AABB, scale, tile footprint, `AnimClipMap`,
/// and articulated socket nodes (`Turret`, `Barrel`, `Rotor`, `Muzzle_*`) into a ready `VisualDef`.
pub fn import_modern_asset(
    source_path: &Path,
    mod_path: &Path,
    opts: &ModernAssetImportOptions,
) -> Result<ModernAssetImportReport> {
    if !source_path.is_file() {
        bail!(
            "Modern asset source file does not exist: {}",
            source_path.display()
        );
    }

    let kind = classify_modern_extension(source_path)?;
    let raw_bytes = fs::read(source_path)
        .with_context(|| format!("Failed to read {}", source_path.display()))?;
    let mut validated = validate_modern_asset_bytes(kind, &raw_bytes, source_path)?;

    let stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imported_asset");
    let asset_id = opts
        .asset_id
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(normalize_id)
        .unwrap_or_else(|| normalize_id(stem));

    match kind {
        ModernAssetKind::GlbModel => {
            let max_lod0 = if opts.as_building { 12_000 } else { 6_000 };
            if opts.optimize_glb {
                let (opt_bytes, _) = optimize_glb_bytes(&validated, max_lod0, 0.22, false)?;
                validated = opt_bytes;
            }
            let info = inspect_glb_bytes(&validated)?;

            let rel_model = format!("assets/models/{asset_id}.glb");
            let dest_model = mod_path.join(&rel_model);
            if let Some(parent) = dest_model.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&dest_model, &validated)
                .with_context(|| format!("Failed to write {}", dest_model.display()))?;

            let ext_x = (info.bounds_max[0] - info.bounds_min[0]).abs().max(0.1);
            let ext_y = (info.bounds_max[1] - info.bounds_min[1]).abs().max(0.1);
            let ext_z = (info.bounds_max[2] - info.bounds_min[2]).abs().max(0.1);

            let tile_world_size = 2.0_f32;
            let footprint = if opts.as_building {
                let tx = ((ext_x / tile_world_size).round() as u32).clamp(1, 6);
                let tz = ((ext_z / tile_world_size).round() as u32).clamp(1, 6);
                [tx, tz]
            } else {
                [1, 1]
            };

            // Compute sensible visual scale so imported units/buildings fit tile metrics cleanly
            let max_horiz = ext_x.max(ext_z);
            let desired_span = if opts.as_building {
                (footprint[0].max(footprint[1]) as f32) * 1.75
            } else {
                1.45
            };
            let uniform_s = if (0.35..=8.0).contains(&max_horiz) {
                1.0
            } else {
                round3((desired_span / max_horiz).clamp(0.05, 20.0))
            };
            let scale = [uniform_s, uniform_s, uniform_s];

            let mut sockets: Vec<SocketOffset> = info
                .detected_sockets
                .iter()
                .map(|(name, off)| SocketOffset {
                    name: name.clone(),
                    offset: [round3(off[0]), round3(off[1]), round3(off[2])],
                })
                .collect();
            sockets.sort_by(|a, b| a.name.cmp(&b.name));
            sockets.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));

            let archetype = if opts.as_building {
                if info.has_turret {
                    VisualArchetype::Turret
                } else {
                    VisualArchetype::ConstructionYard
                }
            } else if info.has_rotor {
                VisualArchetype::Aircraft
            } else if info.has_turret {
                VisualArchetype::Tank
            } else if info.mapped_animations.walk.is_some() {
                VisualArchetype::Infantry
            } else {
                VisualArchetype::GenericBox
            };

            let primary_color = [0.24, 0.52, 0.86];
            let mut baked_cameo_path = None;
            let mut icon_rel = None;
            if opts.bake_cameo {
                let rel_c = format!("assets/ui/cameos/{asset_id}_icon.png");
                let out_c = mod_path.join(&rel_c);
                if let Some(p) = out_c.parent() {
                    fs::create_dir_all(p)?;
                }
                if let Ok(png) = render_glb_to_cameo_png(
                    &validated,
                    scale,
                    primary_color,
                    true,
                    opts.as_building,
                ) {
                    fs::write(&out_c, png)?;
                    baked_cameo_path = Some(out_c);
                    icon_rel = Some(rel_c);
                }
            }

            let visual_def = VisualDef {
                model: rel_model.clone(),
                icon: icon_rel.clone(),
                archetype,
                scale,
                primary_color,
                team_color_tint: true,
                has_turret: info.has_turret,
                barrel_recoil_distance: if info.has_barrel { 0.18 } else { 0.0 },
                spin_speed_deg_per_sec: if info.has_rotor { 540.0 } else { 0.0 },
                sockets,
                animations: info.mapped_animations.clone(),
                has_damage_states: info
                    .detected_sockets
                    .iter()
                    .any(|(n, _)| n.to_ascii_lowercase().starts_with("damagesmoke")),
                ..Default::default()
            };

            let section_hdr = if opts.as_building {
                "buildings"
            } else {
                "units"
            };
            let mut snippet = format!(
                "# Bounds AABB: min=[{:.3}, {:.3}, {:.3}], max=[{:.3}, {:.3}, {:.3}] (extents=[{:.2}, {:.2}, {:.2}])\n",
                info.bounds_min[0],
                info.bounds_min[1],
                info.bounds_min[2],
                info.bounds_max[0],
                info.bounds_max[1],
                info.bounds_max[2],
                ext_x,
                ext_y,
                ext_z
            );
            if opts.as_building {
                snippet.push_str(&format!(
                    "footprint = [{}, {}]\n",
                    footprint[0], footprint[1]
                ));
            }
            snippet.push_str(&format!("[{section_hdr}.visual]\n"));
            snippet.push_str(&format!("model = \"{}\"\n", visual_def.model));
            if let Some(ref ic) = visual_def.icon {
                snippet.push_str(&format!("icon = \"{ic}\"\n"));
            }
            snippet.push_str(&format!(
                "scale = [{:.2}, {:.2}, {:.2}]\n",
                visual_def.scale[0], visual_def.scale[1], visual_def.scale[2]
            ));
            if visual_def.has_turret {
                snippet.push_str("has_turret = true\n");
            }
            if visual_def.barrel_recoil_distance > 0.0 {
                snippet.push_str(&format!(
                    "barrel_recoil_distance = {:.2}\n",
                    visual_def.barrel_recoil_distance
                ));
            }
            if visual_def.spin_speed_deg_per_sec > 0.0 {
                snippet.push_str(&format!(
                    "spin_speed_deg_per_sec = {:.1}\n",
                    visual_def.spin_speed_deg_per_sec
                ));
            }
            if !visual_def.sockets.is_empty() {
                snippet.push_str("sockets = [\n");
                for s in &visual_def.sockets {
                    snippet.push_str(&format!(
                        "    {{ name = \"{}\", offset = [{:.3}, {:.3}, {:.3}] }},\n",
                        s.name, s.offset[0], s.offset[1], s.offset[2]
                    ));
                }
                snippet.push_str("]\n");
            }

            Ok(ModernAssetImportReport {
                asset_id,
                kind,
                destination_path: dest_model,
                relative_asset_path: rel_model,
                bounds_min: Some(info.bounds_min),
                bounds_max: Some(info.bounds_max),
                suggested_footprint: Some(footprint),
                animation_clips: info.animation_clips,
                detected_sockets: info.detected_sockets,
                visual_def: Some(visual_def),
                baked_cameo_path,
                toml_snippet: snippet,
            })
        }
        ModernAssetKind::PngImage => {
            let rel = format!("assets/ui/cameos/{asset_id}.png");
            let dest = mod_path.join(&rel);
            if let Some(p) = dest.parent() {
                fs::create_dir_all(p)?;
            }
            fs::write(&dest, &validated)?;
            let snippet = format!("icon = \"{rel}\"\n");
            Ok(ModernAssetImportReport {
                asset_id,
                kind,
                destination_path: dest,
                relative_asset_path: rel,
                bounds_min: None,
                bounds_max: None,
                suggested_footprint: None,
                animation_clips: Vec::new(),
                detected_sockets: Vec::new(),
                visual_def: None,
                baked_cameo_path: None,
                toml_snippet: snippet,
            })
        }
        ModernAssetKind::OggAudio | ModernAssetKind::FlacAudio | ModernAssetKind::WavAudio => {
            let ext = match kind {
                ModernAssetKind::OggAudio => "ogg",
                ModernAssetKind::FlacAudio => "flac",
                ModernAssetKind::WavAudio => "wav",
                _ => unreachable!(),
            };
            let subdir = if kind == ModernAssetKind::WavAudio {
                "assets/audio/sfx"
            } else {
                "assets/audio/music"
            };
            let rel = format!("{subdir}/{asset_id}.{ext}");
            let dest = mod_path.join(&rel);
            if let Some(p) = dest.parent() {
                fs::create_dir_all(p)?;
            }
            fs::write(&dest, &validated)?;
            let snippet = if kind == ModernAssetKind::WavAudio {
                format!("[weapon_sfx]\n{asset_id} = \"{rel}\"\n")
            } else {
                format!(
                    "[[music_tracks]]\nid = \"{asset_id}\"\ndisplay_name = \"{stem}\"\npath = \"{rel}\"\nmood = \"combat\"\nfactions = [\"allies\", \"soviets\", \"yuri\"]\nfaction_tags = [\"allies\", \"soviets\", \"yuri\"]\n"
                )
            };
            Ok(ModernAssetImportReport {
                asset_id,
                kind,
                destination_path: dest,
                relative_asset_path: rel,
                bounds_min: None,
                bounds_max: None,
                suggested_footprint: None,
                animation_clips: Vec::new(),
                detected_sockets: Vec::new(),
                visual_def: None,
                baked_cameo_path: None,
                toml_snippet: snippet,
            })
        }
        ModernAssetKind::Mp4Video => {
            let rel = format!("assets/videos/{asset_id}.mp4");
            let dest = mod_path.join(&rel);
            if let Some(p) = dest.parent() {
                fs::create_dir_all(p)?;
            }
            fs::write(&dest, &validated)?;
            let snippet = format!("intro_video = \"{rel}\"\nradar_video = \"{rel}\"\n");
            Ok(ModernAssetImportReport {
                asset_id,
                kind,
                destination_path: dest,
                relative_asset_path: rel,
                bounds_min: None,
                bounds_max: None,
                suggested_footprint: None,
                animation_clips: Vec::new(),
                detected_sockets: Vec::new(),
                visual_def: None,
                baked_cameo_path: None,
                toml_snippet: snippet,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_modern_asset_supports_glb_png_ogg_flac_wav_and_mp4_with_cameo_bake() {
        let tmp = std::env::temp_dir().join("rts_test_import_modern_assets");
        let _ = fs::remove_dir_all(&tmp);
        let src_dir = tmp.join("src");
        let out_mod = tmp.join("mod_ws");
        fs::create_dir_all(&src_dir).unwrap();
        fs::create_dir_all(&out_mod).unwrap();

        // 1. Articulated Tank .glb -> auto-extracts AABB, Turret/Barrel/Muzzle sockets, LOD1, and 96x72 cameo PNG
        let src_glb = src_dir.join("custom_heavy_tank.glb");
        let vis_seed = VisualDef {
            model: "assets/models/custom_heavy_tank.glb".to_string(),
            archetype: VisualArchetype::Tank,
            scale: [1.8, 1.1, 2.4],
            primary_color: [0.25, 0.55, 0.85],
            team_color_tint: true,
            ..Default::default()
        };
        crate::mesh_builder::write_single_glb(&src_glb, "custom_heavy_tank", &vis_seed).unwrap();

        let glb_rep = import_modern_asset(
            &src_glb,
            &out_mod,
            &ModernAssetImportOptions {
                asset_id: Some("custom_heavy_tank".to_string()),
                as_building: false,
                bake_cameo: true,
                optimize_glb: true,
            },
        )
        .expect("should import .glb");

        assert_eq!(glb_rep.kind, ModernAssetKind::GlbModel);
        assert!(glb_rep.destination_path.is_file());
        assert!(glb_rep.bounds_min.is_some() && glb_rep.bounds_max.is_some());
        let vd = glb_rep.visual_def.expect("VisualDef generated");
        assert!(vd.has_turret);
        assert!(vd.barrel_recoil_distance > 0.0);
        assert!(!vd.sockets.is_empty());
        let cameo_p = glb_rep.baked_cameo_path.expect("cameo baked");
        assert!(cameo_p.is_file());

        // 2. PNG image
        let png_src = src_dir.join("badge.png");
        fs::copy(&cameo_p, &png_src).unwrap();
        let png_rep = import_modern_asset(&png_src, &out_mod, &ModernAssetImportOptions::default())
            .expect("should import .png");
        assert_eq!(png_rep.kind, ModernAssetKind::PngImage);
        assert!(png_rep.destination_path.is_file());

        // 3. Ogg, FLAC, WAV, MP4 headers
        let ogg_src = src_dir.join("battle_theme.ogg");
        fs::write(
            &ogg_src,
            b"OggS\x00\x02\x00\x00\x00\x00\x00\x00\x00\x00test",
        )
        .unwrap();
        let ogg_rep = import_modern_asset(&ogg_src, &out_mod, &ModernAssetImportOptions::default())
            .expect("should import .ogg");
        assert_eq!(ogg_rep.kind, ModernAssetKind::OggAudio);
        assert!(ogg_rep.destination_path.is_file());

        let flac_src = src_dir.join("lossless_theme.flac");
        fs::write(&flac_src, b"fLaC\x00\x00\x00\x22\x10\x00\x10\x00").unwrap();
        let flac_rep =
            import_modern_asset(&flac_src, &out_mod, &ModernAssetImportOptions::default())
                .expect("should import .flac");
        assert_eq!(flac_rep.kind, ModernAssetKind::FlacAudio);
        assert!(flac_rep.destination_path.is_file());

        let wav_src = src_dir.join("cannon_blast.wav");
        let mut wav_bytes = Vec::new();
        wav_bytes.extend_from_slice(b"RIFF");
        wav_bytes.extend_from_slice(&36u32.to_le_bytes());
        wav_bytes.extend_from_slice(b"WAVEfmt ");
        wav_bytes.extend_from_slice(&16u32.to_le_bytes());
        wav_bytes.extend_from_slice(&1u16.to_le_bytes());
        wav_bytes.extend_from_slice(&1u16.to_le_bytes());
        wav_bytes.extend_from_slice(&22050u32.to_le_bytes());
        wav_bytes.extend_from_slice(&44100u32.to_le_bytes());
        wav_bytes.extend_from_slice(&2u16.to_le_bytes());
        wav_bytes.extend_from_slice(&16u16.to_le_bytes());
        wav_bytes.extend_from_slice(b"data");
        wav_bytes.extend_from_slice(&0u32.to_le_bytes());
        fs::write(&wav_src, &wav_bytes).unwrap();
        let wav_rep = import_modern_asset(&wav_src, &out_mod, &ModernAssetImportOptions::default())
            .expect("should import .wav");
        assert_eq!(wav_rep.kind, ModernAssetKind::WavAudio);
        assert!(wav_rep.destination_path.is_file());

        let mp4_src = src_dir.join("briefing.mp4");
        fs::write(&mp4_src, b"\x00\x00\x00\x14ftypisom\x00\x00\x02\x00isom").unwrap();
        let mp4_rep = import_modern_asset(&mp4_src, &out_mod, &ModernAssetImportOptions::default())
            .expect("should import .mp4");
        assert_eq!(mp4_rep.kind, ModernAssetKind::Mp4Video);
        assert!(mp4_rep.destination_path.is_file());

        let _ = fs::remove_dir_all(&tmp);
    }
}
