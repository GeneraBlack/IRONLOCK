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
        /// `(build_progress, foundation_base_y, active_weld_y, faction_weld_mode)`.
        pub construction_params: Vec4,
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
            construction_params: Vec4::new(1.0, 0.0, 0.0, 0.0),
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
    #[texture(103)]
    #[sampler(104)]
    pub micro_detail_tex: Handle<Image>,
}

pub type TeamColorExtension = TeamDamageMaterialExt;

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

#[inline]
pub fn srgb_channel_to_linear(c: f32) -> f32 {
    let x = c.max(0.0);
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
pub fn linear_channel_to_srgb(c: f32) -> f32 {
    let x = c.max(0.0);
    if x <= 0.0031308 {
        x * 12.92
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

/// Exact CPU mirror of the contrast-preserving S-curve team-paint transfer in perceptual sRGB space.
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

/// Evaluates the Westwood S-curve in perceptual sRGB space from a GPU-sampled Linear `albedo`
/// and returns Linear-RGB `base_color` so Bevy's tonemapper/sRGB output never double-gamma-washes
/// Cobalt Blue or Soviet Crimson into pastel tones.
#[inline]
pub fn evaluate_team_paint_linear_from_linear_albedo(team_srgb: Vec3, linear_albedo: Vec3) -> Vec3 {
    let albedo_srgb = Vec3::new(
        linear_channel_to_srgb(linear_albedo.x),
        linear_channel_to_srgb(linear_albedo.y),
        linear_channel_to_srgb(linear_albedo.z),
    );
    let perceptual_luma = albedo_srgb.dot(Vec3::new(0.299, 0.587, 0.114));
    let shaded_srgb = evaluate_team_paint_s_curve(team_srgb, perceptual_luma);
    Vec3::new(
        srgb_channel_to_linear(shaded_srgb.x),
        srgb_channel_to_linear(shaded_srgb.y),
        srgb_channel_to_linear(shaded_srgb.z),
    )
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
                construction_params: Vec4::new(1.0, 0.0, 0.0, 0.0),
            },
            fow_tex,
            micro_detail_tex: Handle::default(),
        },
    }
}

/// Maps faction identifier to shader arc-weld mode (`+1.0` Allies cyan-white laser,
/// `+2.0` Soviets orange-gold carbon arc, `+3.0` Yuri emerald-psi arc).
#[inline]
pub fn resolve_faction_weld_code(faction_hint: &str) -> f32 {
    let low = faction_hint.to_ascii_lowercase();
    if low.contains("yuri") || low.contains("psi") || low.contains("epsilon") {
        3.0
    } else if low.contains("soviet") || low.contains("nod") || low.contains("red") {
        2.0
    } else {
        1.0
    }
}

#[inline]
pub fn resolve_building_weld_code(
    world: &rts_simulation::world::SimWorld,
    player_id: u8,
    b_def: Option<&rts_sdk::schema::BuildingDef>,
) -> f32 {
    if let Some(b) = b_def {
        let bf = b.faction.trim();
        if !bf.is_empty() && !bf.eq_ignore_ascii_case("any") && !bf.eq_ignore_ascii_case("shared") {
            return resolve_faction_weld_code(bf);
        }
    }
    if let Some(econ) = world.players.get(&player_id) {
        let pf = econ.faction.trim();
        if !pf.is_empty() && !pf.eq_ignore_ascii_case("random") {
            return resolve_faction_weld_code(pf);
        }
    }
    1.0
}

#[inline]
pub fn estimate_building_full_height(
    footprint: Option<[u32; 2]>,
    b_def: Option<&rts_sdk::schema::BuildingDef>,
) -> f32 {
    let scale_y = b_def.map(|b| b.visual.scale[1].max(0.25)).unwrap_or(1.0);
    if let Some(b) = b_def {
        if matches!(
            b.role,
            rts_sdk::schema::BuildingRole::Wall | rts_sdk::schema::BuildingRole::Gate
        ) {
            return 1.85 * scale_y;
        }
        if b.superweapon.is_some() {
            return 5.8 * scale_y;
        }
        if matches!(b.role, rts_sdk::schema::BuildingRole::Defense) {
            return 3.9 * scale_y;
        }
    }
    let max_fp = footprint
        .map(|[fw, fh]| fw.max(fh).max(1) as f32)
        .unwrap_or(2.0);
    (2.1 + max_fp * 0.95).clamp(2.6, 5.8) * scale_y
}

#[inline]
pub fn compute_building_construction_params(
    foundation_y: f32,
    spawn_progress: f32,
    deconstruct_progress: Option<f32>,
    footprint: Option<[u32; 2]>,
    b_def: Option<&rts_sdk::schema::BuildingDef>,
    base_weld_code: f32,
) -> Vec4 {
    let is_deconstruct = deconstruct_progress.is_some();
    let p = deconstruct_progress
        .unwrap_or(spawn_progress)
        .clamp(0.0, 0.9989);
    let est_full_h = estimate_building_full_height(footprint, b_def);
    let active_weld_y = foundation_y + est_full_h * p;
    let weld_mode = if is_deconstruct {
        -base_weld_code.abs()
    } else {
        base_weld_code.abs()
    };
    Vec4::new(p, foundation_y, active_weld_y, weld_mode)
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
    fn linear_pbr_team_paint_converts_srgb_palette_without_double_gamma_washout() {
        let allied_srgb = Vec3::new(0.06, 0.36, 1.00);
        let soviet_srgb = Vec3::new(0.94, 0.05, 0.06);
        // Midtone neutral team gray `sRGB = 0.64` -> `Linear = 0.369`
        let mid_linear_gray = Vec3::splat(srgb_channel_to_linear(0.64));

        let allied_lin =
            evaluate_team_paint_linear_from_linear_albedo(allied_srgb, mid_linear_gray);
        let soviet_lin =
            evaluate_team_paint_linear_from_linear_albedo(soviet_srgb, mid_linear_gray);

        // In Linear PBR reflectance, Cobalt Blue green channel must be ~0.107 (not raw sRGB 0.36!),
        // and Soviet Crimson green/blue channels must stay < 0.01 (not raw sRGB 0.05 -> salmon pink!).
        assert!(
            allied_lin.y > 0.08 && allied_lin.y < 0.14,
            "allied_lin={allied_lin:?}"
        );
        assert!(allied_lin.z > 0.92, "allied_lin={allied_lin:?}");
        assert!(soviet_lin.x > 0.82, "soviet_lin={soviet_lin:?}");
        assert!(
            soviet_lin.y < 0.01 && soviet_lin.z < 0.01,
            "soviet_lin={soviet_lin:?}"
        );

        let wgsl = include_str!("../../../assets/shaders/unit_pbr_team.wgsl");
        assert!(wgsl.contains("fn srgb_to_linear_vec3("));
        assert!(wgsl.contains("fn linear_to_srgb_vec3("));
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

    #[test]
    fn wgsl_enforces_triplanar_micro_detail_glass_attenuation_and_hdr_emissive_punch() {
        let wgsl = include_str!("../../../assets/shaders/unit_pbr_team.wgsl");
        assert!(wgsl.contains("@group(2) @binding(103) var micro_detail_tex: texture_2d<f32>;"));
        assert!(wgsl.contains("@group(2) @binding(104) var micro_detail_sampler: sampler;"));
        assert!(wgsl.contains("world_pos.xz * 2.85"));
        assert!(wgsl.contains("in.uv * 18.0"));
        assert!(wgsl.contains("smoothstep(0.16, 0.28, pbr_input.material.perceptual_roughness)"));
        assert!(wgsl.contains("1.0 - smoothstep(0.06, 0.35, base_emissive_luma)"));
        assert!(wgsl.contains("mix(1.0, 2.4, smoothstep(0.35, 0.58, raw_emissive_luma))"));
    }

    #[test]
    fn construct_shader_params_and_shared_cache_revert_plus_orphaned_deconstruct_work() {
        use crate::rendering::{
            sync_gltf_lod_and_team_damage_materials, SimVisualEntity, SyncedGltfSceneVisuals,
        };
        use crate::{ModResource, SimWorldResource};
        use bevy::ecs::system::RunSystemOnce;
        use rts_simulation::world::SimWorld;

        assert_eq!(
            TeamDamageUniform::default().construction_params,
            Vec4::new(1.0, 0.0, 0.0, 0.0)
        );
        assert_eq!(resolve_faction_weld_code("allies"), 1.0);
        assert_eq!(resolve_faction_weld_code("soviets"), 2.0);
        assert_eq!(resolve_faction_weld_code("yuri"), 3.0);

        let fwd = compute_building_construction_params(1.5, 0.50, None, Some([3, 3]), None, 2.0);
        assert!((fwd.x - 0.50).abs() < 1e-5 && (fwd.y - 1.5).abs() < 1e-5 && fwd.z > 3.5);
        assert!((fwd.w - 2.0).abs() < 1e-5);
        let rev =
            compute_building_construction_params(1.5, 1.0, Some(0.42), Some([3, 3]), None, 3.0);
        assert!((rev.x - 0.42).abs() < 1e-5 && (rev.w - (-3.0)).abs() < 1e-5);

        let wgsl = include_str!("../../../assets/shaders/unit_pbr_team.wgsl");
        assert!(wgsl.contains("team_ext.construction_params.x < 0.999"));
        assert!(wgsl.contains("world_pos.y < team_ext.construction_params.y - 0.02"));
        assert!(wgsl.contains("soot_noise3(world_pos * 5.5)"));
        assert!(wgsl.contains("power_gate"));

        let mod_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../rust-ra2")
            .canonicalize()
            .expect("canonicalize rust-ra2");
        let loaded = LoadedMod::load_from_dir(mod_dir).expect("load rust-ra2");
        let mut world = SimWorld::from_mod_map(&loaded, &loaded.entry_map);
        let b_id = world
            .spawn_building(1, "soviet_tesla_reactor", [10, 10], &loaded, true)
            .expect("spawn building");

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<super::super::TeamDamageMaterialPalette>()
            .init_resource::<UnitPbrTeamMaterialCache>()
            .insert_resource(Assets::<Mesh>::default())
            .insert_resource(Assets::<Image>::default())
            .insert_resource(Assets::<StandardMaterial>::default())
            .insert_resource(Assets::<UnitPbrTeamMaterial>::default())
            .insert_resource(ModResource(loaded));
        let mesh_h = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Cuboid::new(1.0, 1.0, 1.0));
        let mut child_mesh = Entity::PLACEHOLDER;
        let root_ent = app
            .world_mut()
            .spawn((
                SimVisualEntity {
                    sim_id: b_id,
                    entity_type_id: "soviet_tesla_reactor".to_string(),
                    building_footprint: Some([2, 2]),
                    world_pos: Vec3::new(20.0, 0.5, 20.0),
                    spawn_progress: 0.40,
                    deconstruct_progress: None,
                    death_anim_progress: None,
                    is_infantry: false,
                    cached_articulated_parts: None,
                    has_3d_turret_node: false,
                },
                Transform::IDENTITY,
                Visibility::Inherited,
            ))
            .with_children(|p| {
                child_mesh = p
                    .spawn((Name::new("Hull"), Mesh3d(mesh_h), Transform::IDENTITY))
                    .id();
            })
            .id();
        app.insert_resource(SimWorldResource(world.clone()));

        let tick_mat = |app: &mut App| {
            let _ = app
                .world_mut()
                .run_system_once(sync_gltf_lod_and_team_damage_materials);
            app.world_mut().flush();
        };
        let read_cp = |app: &App| -> Vec4 {
            let h = &app
                .world()
                .entity(child_mesh)
                .get::<MeshMaterial3d<UnitPbrTeamMaterial>>()
                .unwrap()
                .0;
            app.world()
                .resource::<Assets<UnitPbrTeamMaterial>>()
                .get(h)
                .unwrap()
                .extension
                .params
                .construction_params
        };

        tick_mat(&mut app);
        tick_mat(&mut app);
        assert_eq!(
            app.world()
                .resource::<UnitPbrTeamMaterialCache>()
                .entries
                .len(),
            0
        );
        let dyn_cp = read_cp(&app);
        assert!((dyn_cp.x - 0.40).abs() < 1e-5 && (dyn_cp.w - 2.0).abs() < 1e-5);

        // Finish construction -> reverts to shared cache (`x = 1.0`, `active_construct_handles` empty)
        app.world_mut()
            .entity_mut(root_ent)
            .get_mut::<SimVisualEntity>()
            .unwrap()
            .spawn_progress = 1.0;
        tick_mat(&mut app);
        assert!(app
            .world()
            .entity(root_ent)
            .get::<SyncedGltfSceneVisuals>()
            .unwrap()
            .active_construct_handles
            .is_empty());
        assert_eq!(
            app.world()
                .resource::<UnitPbrTeamMaterialCache>()
                .entries
                .len(),
            1
        );
        assert!((read_cp(&app).x - 1.0).abs() < 1e-5);

        // Sell / undeploy (`SimEntity` removed, `deconstruct_progress = Some(0.55)`) -> `w = -2.0`
        world.entities.remove(&b_id);
        app.insert_resource(SimWorldResource(world));
        app.world_mut()
            .entity_mut(root_ent)
            .get_mut::<SimVisualEntity>()
            .unwrap()
            .deconstruct_progress = Some(0.55);
        tick_mat(&mut app);
        let dec_cp = read_cp(&app);
        assert!((dec_cp.x - 0.55).abs() < 1e-5 && (dec_cp.w - (-2.0)).abs() < 1e-5);
    }
}
