use super::DamageVisualTier;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};
use rts_sdk::schema::{TeamMaskMode, VisualDef};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

pub const UNIT_PBR_TEAM_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(0x91c4_52a8_7d13_48e2_a3f6_84b0_c217_2360);

pub type UnitPbrTeamMaterial = ExtendedMaterial<StandardMaterial, TeamDamageMaterialExt>;

#[allow(dead_code)]
mod shader_uniforms {
    use bevy::prelude::Vec4;
    use bevy::render::render_resource::ShaderType;

    #[derive(Clone, Copy, Debug, PartialEq, ShaderType)]
    pub struct TeamDamageUniform {
        /// `rgb = accessible team color`, `a = team_tint_strength`.
        pub team_color: Vec4,
        /// `(metallic_override, roughness_override, emissive_boost, team_mask_threshold)`.
        pub pbr_overrides: Vec4,
        /// `(damage_soot, shield_fresnel_strength, mask_mode, strategic_fade_alpha)`.
        pub damage_and_fx: Vec4,
        /// `rgb = shield/chrono/psionic rim color`, `a = pulse speed`.
        pub shield_color: Vec4,
        /// `(map_world_w, map_world_h, fow_strength, elapsed_secs)`.
        pub map_fow_params: Vec4,
    }
}
pub use shader_uniforms::TeamDamageUniform;

impl Default for TeamDamageUniform {
    fn default() -> Self {
        Self {
            team_color: Vec4::new(0.06, 0.36, 1.0, 0.92),
            pbr_overrides: Vec4::new(-1.0, -1.0, 0.0, 0.22),
            damage_and_fx: Vec4::new(0.0, 0.0, 0.0, 1.0),
            shield_color: Vec4::new(0.30, 0.85, 1.0, 1.0),
            map_fow_params: Vec4::new(128.0, 128.0, 0.0, 0.0),
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TeamDamageMaterialExt {
    #[uniform(100)]
    pub params: TeamDamageUniform,
    #[texture(101)]
    #[sampler(102)]
    pub fow_tex: Handle<Image>,
}

impl MaterialExtension for TeamDamageMaterialExt {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(UNIT_PBR_TEAM_SHADER_HANDLE.clone())
    }

    fn deferred_fragment_shader() -> ShaderRef {
        ShaderRef::Handle(UNIT_PBR_TEAM_SHADER_HANDLE.clone())
    }
}

/// Active shield / status Fresnel illumination tier rendered in `unit_pbr_team.wgsl` (`Task 236`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShieldFresnelKind {
    #[default]
    None,
    IronCurtain,
    ChronoTemporal,
    PsionicControl,
    TeslaOvercharge,
}

impl ShieldFresnelKind {
    pub fn strength_and_color(self) -> (f32, Vec4) {
        match self {
            Self::None => (0.0, Vec4::new(0.0, 0.0, 0.0, 0.0)),
            Self::IronCurtain => (0.88, Vec4::new(0.96, 0.14, 0.20, 1.0)),
            Self::ChronoTemporal => (0.84, Vec4::new(0.22, 0.88, 1.00, 1.0)),
            Self::PsionicControl => (0.76, Vec4::new(0.84, 0.24, 1.00, 1.0)),
            Self::TeslaOvercharge => (0.68, Vec4::new(0.36, 0.86, 1.00, 1.0)),
        }
    }
}

#[inline]
pub fn team_mask_mode_to_shader_code(mode: TeamMaskMode) -> f32 {
    match mode {
        TeamMaskMode::AutoNeutralGray => 0.0,
        TeamMaskMode::AlphaMask => 1.0,
        TeamMaskMode::VertexColorMask => 2.0,
        TeamMaskMode::FullTint => 3.0,
        TeamMaskMode::Disabled => 4.0,
    }
}

#[inline]
pub fn damage_tier_soot_strength(tier: DamageVisualTier) -> f32 {
    match tier {
        DamageVisualTier::Intact => 0.0,
        DamageVisualTier::Yellow50 => 0.48,
        DamageVisualTier::Red25 => 0.88,
    }
}

pub fn hash_visual_pbr_spec(visual: Option<&VisualDef>) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    if let Some(v) = visual {
        v.team_color_tint.hash(&mut hasher);
        std::mem::discriminant(&v.team_mask_mode).hash(&mut hasher);
        v.team_mask_threshold.map(f32::to_bits).hash(&mut hasher);
        v.metallic.map(f32::to_bits).hash(&mut hasher);
        v.roughness.map(f32::to_bits).hash(&mut hasher);
        v.emissive_boost.map(f32::to_bits).hash(&mut hasher);
        v.normal_map_path.hash(&mut hasher);
        v.orm_map_path.hash(&mut hasher);
    } else {
        0u8.hash(&mut hasher);
    }
    hasher.finish()
}

/// Exact CPU mirror of the contrast-preserving S-curve team-paint transfer in `unit_pbr_team.wgsl`.
#[inline]
pub fn evaluate_team_paint_s_curve(team_rgb: Vec3, luma: f32) -> Vec3 {
    let smoothstep = |e0: f32, e1: f32, x: f32| -> f32 {
        let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let mid_s = smoothstep(0.25, 0.62, luma);
    let hi_s = smoothstep(0.58, 0.88, luma);
    let shade_gain = (luma * 0.95) * (1.0 - mid_s) + (0.96 + 0.26 * hi_s) * mid_s;
    let spec_edge = smoothstep(0.80, 0.98, luma) * 0.28;
    (team_rgb * shade_gain).lerp(Vec3::ONE, spec_edge)
}

/// Builds a `UnitPbrTeamMaterial` that clones `orig_base` so 100% of glTF textures
/// (`base_color_texture`, `normal_map_texture`, `metallic_roughness_texture`,
/// `occlusion_texture`, `emissive_texture`) and `COLOR_0` vertex colors are preserved untouched
/// whenever `custom_normal_tex` / `custom_orm_tex` are `None` (`Task 236`).
#[allow(clippy::too_many_arguments)]
pub fn build_unit_pbr_team_material(
    orig_base: &StandardMaterial,
    visual: Option<&VisualDef>,
    accessible_team_rgba: [f32; 4],
    slot_is_neutral: bool,
    tier: DamageVisualTier,
    shield: ShieldFresnelKind,
    custom_normal_tex: Option<Handle<Image>>,
    custom_orm_tex: Option<Handle<Image>>,
    fow_tex: Handle<Image>,
    world_size: Vec2,
    fow_strength: f32,
    strategic_fade: f32,
    elapsed_secs: f32,
) -> UnitPbrTeamMaterial {
    let mut base = orig_base.clone();
    if let Some(norm_h) = custom_normal_tex {
        base.normal_map_texture = Some(norm_h);
    }
    if let Some(orm_h) = custom_orm_tex {
        base.metallic_roughness_texture = Some(orm_h.clone());
        base.occlusion_texture = Some(orm_h);
    }

    let (tint_enabled, mask_mode, threshold, met_ovr, rough_ovr, emissive_boost) =
        if let Some(v) = visual {
            (
                v.team_color_tint && v.team_mask_mode != TeamMaskMode::Disabled,
                v.team_mask_mode,
                v.team_mask_threshold.unwrap_or(0.22).clamp(0.05, 0.95),
                v.metallic.unwrap_or(-1.0),
                v.roughness.unwrap_or(-1.0),
                v.emissive_boost.unwrap_or(0.0).max(0.0),
            )
        } else {
            (true, TeamMaskMode::AutoNeutralGray, 0.22, -1.0, -1.0, 0.0)
        };

    if mask_mode == TeamMaskMode::AlphaMask {
        base.alpha_mode = AlphaMode::Opaque;
    }

    let tint_strength = if !tint_enabled {
        0.0
    } else if slot_is_neutral {
        0.22
    } else {
        0.95
    };
    let (shield_strength, shield_col) = shield.strength_and_color();

    UnitPbrTeamMaterial {
        base,
        extension: TeamDamageMaterialExt {
            params: TeamDamageUniform {
                team_color: Vec4::new(
                    accessible_team_rgba[0],
                    accessible_team_rgba[1],
                    accessible_team_rgba[2],
                    tint_strength,
                ),
                pbr_overrides: Vec4::new(met_ovr, rough_ovr, emissive_boost, threshold),
                damage_and_fx: Vec4::new(
                    damage_tier_soot_strength(tier),
                    shield_strength,
                    team_mask_mode_to_shader_code(mask_mode),
                    strategic_fade.clamp(0.0, 1.0),
                ),
                shield_color: shield_col,
                map_fow_params: Vec4::new(
                    world_size.x.max(1.0),
                    world_size.y.max(1.0),
                    fow_strength,
                    elapsed_secs,
                ),
            },
            fow_tex,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnitPbrMaterialCacheKey {
    pub base_mat_id: Option<AssetId<StandardMaterial>>,
    pub visual_spec_hash: u64,
    pub quantized_rgba: [u8; 3],
    pub slot_is_neutral: bool,
    pub tier: DamageVisualTier,
    pub shield: ShieldFresnelKind,
}

/// Shared cache of `Handle<UnitPbrTeamMaterial>` instances keyed by `(glTF StandardMaterial AssetId,
/// VisualDef PBR spec, accessible team color, DamageVisualTier, ShieldFresnelKind)` so hundreds of
/// identical tanks share exact GPU materials while preserving every glTF texture (`Task 236`).
#[derive(Resource, Debug, Default, Clone)]
pub struct UnitPbrTeamMaterialCache {
    pub entries: HashMap<UnitPbrMaterialCacheKey, Handle<UnitPbrTeamMaterial>>,
    pub preview_entries: HashMap<Option<AssetId<StandardMaterial>>, Handle<UnitPbrTeamMaterial>>,
}

impl UnitPbrTeamMaterialCache {
    pub fn clear(&mut self) {
        self.entries.clear();
        self.preview_entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::build_default_fow_overlay_image;
    use rts_sdk::loader::LoadedMod;
    use std::path::PathBuf;

    #[test]
    fn alpha_mask_mode_forces_opaque_alpha_mode_and_shader_enforces_westwood_contract() {
        let blend_base = StandardMaterial {
            alpha_mode: AlphaMode::Blend,
            ..default()
        };
        let alpha_mask_vis = VisualDef {
            team_mask_mode: TeamMaskMode::AlphaMask,
            ..Default::default()
        };
        let built_alpha = build_unit_pbr_team_material(
            &blend_base,
            Some(&alpha_mask_vis),
            [0.94, 0.05, 0.06, 1.0],
            false,
            DamageVisualTier::Intact,
            ShieldFresnelKind::None,
            None,
            None,
            Handle::default(),
            Vec2::splat(128.0),
            1.0,
            1.0,
            0.0,
        );
        assert_eq!(built_alpha.base.alpha_mode, AlphaMode::Opaque);
        assert!((built_alpha.extension.params.damage_and_fx.z - 1.0).abs() < 1e-5);
        assert_eq!(built_alpha.extension.params.pbr_overrides.x, -1.0);
        assert_eq!(built_alpha.extension.params.pbr_overrides.y, -1.0);

        let wgsl = include_str!("../../../assets/shaders/unit_pbr_team.wgsl");
        assert!(wgsl.contains("if base_rgba.a < 0.98"));
        assert!(wgsl.contains("clamp(1.0 - base_rgba.a, 0.0, 1.0) * tint_strength"));
        assert!(wgsl.contains("var extra_emissive = vec3<f32>(0.0);"));
        assert!(!wgsl.contains("mask_w * 0.12"));
        assert!(!wgsl.contains("mask_w * 0.55"));
        assert!(wgsl.contains("0.52"));
    }

    #[test]
    fn team_paint_s_curve_preserves_dark_ao_crevices_vibrant_midtones_and_bevel_highlights() {
        let team = Vec3::new(0.06, 0.36, 1.00);

        // Zero floor lift at black cavity (`luma = 0.0`)
        assert!(evaluate_team_paint_s_curve(team, 0.0).length() < 1e-6);

        // Dark Cycles AO crevices (`luma = 0.15..0.30`) stay genuinely dark (`team * 0.14..0.32`)
        let ao_low = evaluate_team_paint_s_curve(team, 0.15);
        let ao_high = evaluate_team_paint_s_curve(team, 0.30);
        assert!(ao_low.z >= 0.14 && ao_low.z <= 0.16);
        assert!(ao_high.z >= 0.30 && ao_high.z <= 0.32);

        // Mid-tone armor (`luma = 0.60..0.72`) hits true vibrant faction color (`team * 0.95..1.08`)
        let mid_low = evaluate_team_paint_s_curve(team, 0.60);
        let mid_high = evaluate_team_paint_s_curve(team, 0.72);
        assert!(mid_low.z >= 0.95 && mid_low.z <= 0.98);
        assert!(mid_high.z >= 1.06 && mid_high.z <= 1.09);

        // Bright beveled edges (`luma > 0.82`) mix smoothly toward sunlit desaturated highlight
        let bevel = evaluate_team_paint_s_curve(team, 0.95);
        assert!(bevel.x > mid_high.x * 3.0);
        assert!(bevel.y > mid_high.y * 1.25);
    }

    #[test]
    fn build_material_preserves_all_five_gltf_textures_and_ra2_toml_has_zero_flat_pbr_clobbers() {
        let mut images = Assets::<Image>::default();
        let albedo_h = images.add(build_default_fow_overlay_image(4, 4));
        let normal_h = images.add(build_default_fow_overlay_image(4, 4));
        let mr_h = images.add(build_default_fow_overlay_image(4, 4));
        let occ_h = images.add(build_default_fow_overlay_image(4, 4));
        let emissive_h = images.add(build_default_fow_overlay_image(4, 4));
        let fow_h = images.add(build_default_fow_overlay_image(8, 8));

        let orig = StandardMaterial {
            base_color_texture: Some(albedo_h.clone()),
            normal_map_texture: Some(normal_h.clone()),
            metallic_roughness_texture: Some(mr_h.clone()),
            occlusion_texture: Some(occ_h.clone()),
            emissive_texture: Some(emissive_h.clone()),
            metallic: 1.0,
            perceptual_roughness: 1.0,
            ..default()
        };
        let vis = VisualDef {
            team_mask_mode: TeamMaskMode::AlphaMask,
            emissive_boost: Some(1.5),
            ..Default::default()
        };
        let built = build_unit_pbr_team_material(
            &orig,
            Some(&vis),
            [0.06, 0.36, 1.0, 1.0],
            false,
            DamageVisualTier::Intact,
            ShieldFresnelKind::None,
            None,
            None,
            fow_h,
            Vec2::splat(128.0),
            1.0,
            1.0,
            0.0,
        );
        assert_eq!(built.base.base_color_texture, Some(albedo_h));
        assert_eq!(built.base.normal_map_texture, Some(normal_h));
        assert_eq!(built.base.metallic_roughness_texture, Some(mr_h));
        assert_eq!(built.base.occlusion_texture, Some(occ_h));
        assert_eq!(built.base.emissive_texture, Some(emissive_h));
        assert_eq!(built.base.metallic, 1.0);
        assert_eq!(built.base.perceptual_roughness, 1.0);
        assert_eq!(built.extension.params.pbr_overrides.x, -1.0);
        assert_eq!(built.extension.params.pbr_overrides.y, -1.0);
        assert!((built.extension.params.pbr_overrides.z - 1.5).abs() < 1e-5);

        let mod_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../rust-ra2")
            .canonicalize()
            .expect("canonicalize rust-ra2");
        let loaded = LoadedMod::load_from_dir(mod_dir).expect("load rust-ra2");
        assert!(!loaded.units.is_empty() && !loaded.buildings.is_empty());
        for (uid, u) in &loaded.units {
            assert!(
                u.visual.metallic.is_none() && u.visual.roughness.is_none(),
                "unit {uid} still clobbers glTF PBR maps with scalar metallic/roughness"
            );
            assert_eq!(u.visual.team_mask_mode, TeamMaskMode::AlphaMask);
        }
        for (bid, b) in &loaded.buildings {
            assert!(
                b.visual.metallic.is_none() && b.visual.roughness.is_none(),
                "building {bid} still clobbers glTF PBR maps with scalar metallic/roughness"
            );
            assert_eq!(b.visual.team_mask_mode, TeamMaskMode::AlphaMask);
        }
    }
}
