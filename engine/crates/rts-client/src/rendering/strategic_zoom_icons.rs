use super::preview_stage::PreviewStageCamera;
use super::{
    resolve_accessible_team_color, resolve_player_team_color_slot, CachedFogVisibility,
    ColorblindPaletteMode, ColorblindSettingsResource,
};
use crate::{ModResource, SimWorldResource};
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{MovementDomain, UnitRole};
use rts_simulation::world::{EntityKind, SimEntity, SimWorld, TileVisibility};

pub const STRATEGIC_ICON_TILE_PX: u32 = 64;
pub const STRATEGIC_ICON_COUNT: u32 = 8;
pub const STRATEGIC_ATLAS_WIDTH: u32 = STRATEGIC_ICON_TILE_PX * STRATEGIC_ICON_COUNT;
pub const STRATEGIC_ATLAS_HEIGHT: u32 = STRATEGIC_ICON_TILE_PX;

/// 8 canonical SupCom/BAR orbital strategic icon classes (`Task 237`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StrategicIconCategory {
    Infantry = 0,
    TankArmor = 1,
    Artillery = 2,
    Air = 3,
    Naval = 4,
    Hero = 5,
    Building = 6,
    HarvesterResource = 7,
}

impl StrategicIconCategory {
    pub const ALL: [Self; 8] = [
        Self::Infantry,
        Self::TankArmor,
        Self::Artillery,
        Self::Air,
        Self::Naval,
        Self::Hero,
        Self::Building,
        Self::HarvesterResource,
    ];

    #[inline]
    pub fn atlas_uv_bounds(self) -> (f32, f32) {
        let idx = (self as u32) as f32;
        let inv = 1.0 / (STRATEGIC_ICON_COUNT as f32);
        let pad = 0.5 / (STRATEGIC_ATLAS_WIDTH as f32);
        (idx * inv + pad, (idx + 1.0) * inv - pad)
    }

    #[inline]
    pub fn size_multiplier(self) -> f32 {
        match self {
            Self::Infantry => 0.80,
            Self::TankArmor => 1.00,
            Self::Artillery => 1.04,
            Self::Air => 1.08,
            Self::Naval => 1.18,
            Self::Hero => 1.32,
            Self::Building => 1.38,
            Self::HarvesterResource => 1.06,
        }
    }
}

/// Classifies any `SimEntity` into one of the 8 orbital strategic icon categories (`Task 237`).
pub fn classify_entity_strategic_icon(
    world: &SimWorld,
    loaded: &LoadedMod,
    ent: &SimEntity,
) -> StrategicIconCategory {
    if world.trait_stores.heroes.contains_key(&ent.id) {
        return StrategicIconCategory::Hero;
    }
    match &ent.kind {
        EntityKind::Building { building_id } => {
            if let Some(b_def) = loaded.buildings.get(building_id) {
                if b_def.role == rts_sdk::schema::BuildingRole::Refinery {
                    return StrategicIconCategory::HarvesterResource;
                }
            }
            StrategicIconCategory::Building
        }
        EntityKind::Unit { unit_id } => {
            let Some(u_def) = loaded.units.get(unit_id) else {
                return StrategicIconCategory::TankArmor;
            };
            if u_def.hero_trait.is_some() || u_def.has_tag("hero") {
                return StrategicIconCategory::Hero;
            }
            if ent.harvester_state.is_some() || u_def.role == UnitRole::Harvester {
                return StrategicIconCategory::HarvesterResource;
            }
            if matches!(
                u_def.movement_domain,
                MovementDomain::Air | MovementDomain::Jumpjet
            ) {
                return StrategicIconCategory::Air;
            }
            if matches!(
                u_def.movement_domain,
                MovementDomain::Naval | MovementDomain::Amphibious
            ) {
                return StrategicIconCategory::Naval;
            }
            match u_def.role {
                UnitRole::Infantry | UnitRole::Engineer => StrategicIconCategory::Infantry,
                _ => {
                    if u_def.has_tag("artillery")
                        || (u_def.immobile_when_deployed && u_def.deploy_weapon.is_some())
                        || unit_id.contains("artillery")
                        || unit_id.contains("v3")
                        || unit_id.contains("howitzer")
                    {
                        StrategicIconCategory::Artillery
                    } else {
                        StrategicIconCategory::TankArmor
                    }
                }
            }
        }
    }
}

#[inline]
fn sd_box(p: Vec2, b: Vec2) -> f32 {
    let d = p.abs() - b;
    d.max(Vec2::ZERO).length() + d.x.max(d.y).min(0.0)
}

#[inline]
fn sd_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = (pa.dot(ba) / ba.length_squared().max(1e-6)).clamp(0.0, 1.0);
    (pa - ba * h).length()
}

/// Evaluates signed distance (`< 0` inside glyph) in normalized `[-1, 1]^2` tile space for each category.
fn icon_signed_distance(cat: StrategicIconCategory, p: Vec2) -> f32 {
    match cat {
        StrategicIconCategory::Infantry => {
            // Crisp rotated tactical diamond + inner dot
            let inv_sqrt2 = std::f32::consts::FRAC_1_SQRT_2;
            let rot = Vec2::new((p.x + p.y) * inv_sqrt2, (p.y - p.x) * inv_sqrt2);
            sd_box(rot, Vec2::splat(0.46))
        }
        StrategicIconCategory::TankArmor => {
            // Armored tracked lozenge + turret chevron bar
            let body = sd_box(Vec2::new(p.x, p.y + 0.04), Vec2::new(0.58, 0.36)) - 0.10;
            let barrel = sd_segment(p, Vec2::new(0.0, -0.05), Vec2::new(0.0, -0.72)) - 0.11;
            body.min(barrel)
        }
        StrategicIconCategory::Artillery => {
            // Indirect-fire outer ring + central shell core + top arc pip
            let r = p.length();
            let ring = (r - 0.56).abs() - 0.11;
            let core = r - 0.22;
            let pip = sd_segment(p, Vec2::new(0.0, -0.52), Vec2::new(0.0, -0.78)) - 0.09;
            ring.min(core).min(pip)
        }
        StrategicIconCategory::Air => {
            // Swept supersonic delta-wing chevron
            let s1 = sd_segment(p, Vec2::new(0.0, -0.72), Vec2::new(-0.66, 0.56)) - 0.14;
            let s2 = sd_segment(p, Vec2::new(0.0, -0.72), Vec2::new(0.66, 0.56)) - 0.14;
            let tail = sd_segment(p, Vec2::new(-0.42, 0.34), Vec2::new(0.42, 0.34)) - 0.11;
            s1.min(s2).min(tail)
        }
        StrategicIconCategory::Naval => {
            // Warship prow shield + superstructure mast
            let hull_top = sd_box(Vec2::new(p.x, p.y - 0.18), Vec2::new(0.62, 0.24)) - 0.08;
            let prow_l = sd_segment(p, Vec2::new(-0.62, 0.12), Vec2::new(0.0, -0.68)) - 0.14;
            let prow_r = sd_segment(p, Vec2::new(0.62, 0.12), Vec2::new(0.0, -0.68)) - 0.14;
            hull_top.min(prow_l).min(prow_r)
        }
        StrategicIconCategory::Hero => {
            // Bold 5-pointed Hero Commander star
            let angle = p.x.atan2(-p.y);
            let r = p.length();
            let star_wave = (angle * 5.0).cos() * 0.5 + 0.5;
            let target_r = 0.32 + star_wave * 0.44;
            r - target_r
        }
        StrategicIconCategory::Building => {
            // Fortified square bastion with corner turrets
            let main_sq = sd_box(p, Vec2::splat(0.56)) - 0.04;
            let c1 = sd_box(p.abs() - Vec2::splat(0.52), Vec2::splat(0.16));
            main_sq.min(c1)
        }
        StrategicIconCategory::HarvesterResource => {
            // Hexagonal crystal / resource collector badge
            let q = p.abs();
            (q.x * 0.866_025 + q.y * 0.5).max(q.y) - 0.62
        }
    }
}

/// Synthesizes the `512x64` (`8 x 64x64`) anti-aliased RGBA8 Strategic Icon Atlas (`Task 237`).
/// White interior (`RGB=1.0`) multiplies cleanly with `ATTRIBUTE_COLOR` accessible team color while
/// the dark border halo (`RGB=0.05`) guarantees crisp readability over bright sand, snow, or water!
pub fn build_strategic_icon_atlas_image() -> Image {
    let w = STRATEGIC_ATLAS_WIDTH;
    let h = STRATEGIC_ATLAS_HEIGHT;
    let mut bytes = vec![0u8; (w * h * 4) as usize];
    let aa = 2.4 / (STRATEGIC_ICON_TILE_PX as f32);

    for cat in StrategicIconCategory::ALL {
        let tile_x0 = (cat as u32) * STRATEGIC_ICON_TILE_PX;
        for py in 0..STRATEGIC_ICON_TILE_PX {
            let ny = ((py as f32 + 0.5) / (STRATEGIC_ICON_TILE_PX as f32)) * 2.0 - 1.0;
            for px in 0..STRATEGIC_ICON_TILE_PX {
                let nx = ((px as f32 + 0.5) / (STRATEGIC_ICON_TILE_PX as f32)) * 2.0 - 1.0;
                let p = Vec2::new(nx, ny);
                let d = icon_signed_distance(cat, p);

                let fill_cov = (1.0 - ((d + aa * 0.5) / aa).clamp(0.0, 1.0)).clamp(0.0, 1.0);
                let outline_cov = (1.0 - ((d - 0.14) / (aa * 1.2)).clamp(0.0, 1.0)).clamp(0.0, 1.0);
                // Inner bevel highlight so icons look crisp and tactile even at 550m
                let inner_core = (1.0 - ((d + 0.22) / 0.12).clamp(0.0, 1.0)) * 0.18;

                let alpha = outline_cov.max(fill_cov);
                let shade = (0.06 * (outline_cov - fill_cov).max(0.0)
                    + (0.90 + inner_core) * fill_cov)
                    / alpha.max(0.001);

                let out = (((py * w) + (tile_x0 + px)) as usize) * 4;
                let c_u8 = (shade.clamp(0.0, 1.0) * 255.0).round() as u8;
                let a_u8 = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
                bytes[out] = c_u8;
                bytes[out + 1] = c_u8;
                bytes[out + 2] = c_u8;
                bytes[out + 3] = a_u8;
            }
        }
    }

    let mut img = Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        bytes,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    img.sampler = ImageSampler::linear();
    img
}

#[derive(Resource, Default, Clone)]
pub struct StrategicIconGpuResources {
    pub initialized: bool,
    pub atlas_image: Handle<Image>,
    pub batch_mesh: Handle<Mesh>,
    pub batch_material: Handle<StandardMaterial>,
    pub ring_mesh: Handle<Mesh>,
    pub ring_material: Handle<StandardMaterial>,
}

#[derive(Component)]
pub struct StrategicIconBatchEntity;

#[derive(Component)]
pub struct StrategicSelectionHighlightEntity;

fn empty_uv_color_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<[f32; 2]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
    mesh.insert_indices(Indices::U32(Vec::new()));
    mesh
}

/// Synchronizes the GPU-batched Orbital Strategic Icon Pass (`165m..550m` crossfade, `Task 237`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_strategic_zoom_icons(
    mut commands: Commands,
    mod_res: Option<Res<ModResource>>,
    sim_res: Option<Res<SimWorldResource>>,
    interaction: Option<Res<crate::input::ClientInteractionState>>,
    cached_fow: Option<Res<CachedFogVisibility>>,
    cb_settings: Option<Res<ColorblindSettingsResource>>,
    mut gpu_res: ResMut<StrategicIconGpuResources>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    images: Option<ResMut<Assets<Image>>>,
    cam_q: Query<(&crate::camera::RtsCameraRig, &Transform), Without<PreviewStageCamera>>,
    mut batch_q: Query<
        &mut Visibility,
        (
            With<StrategicIconBatchEntity>,
            Without<StrategicSelectionHighlightEntity>,
        ),
    >,
    mut sel_q: Query<
        &mut Visibility,
        (
            With<StrategicSelectionHighlightEntity>,
            Without<StrategicIconBatchEntity>,
        ),
    >,
) {
    let (Some(mod_res), Some(sim_res), Some(mut meshes), Some(mut materials), Some(mut images)) =
        (mod_res, sim_res, meshes, materials, images)
    else {
        return;
    };

    if !gpu_res.initialized {
        gpu_res.initialized = true;
        let atlas_h = images.add(build_strategic_icon_atlas_image());
        let icon_mesh_h = meshes.add(empty_uv_color_mesh());
        let icon_mat_h = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(atlas_h.clone()),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        });
        let ring_mesh_h = meshes.add(empty_uv_color_mesh());
        let ring_mat_h = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        });

        commands.spawn((
            StrategicIconBatchEntity,
            Mesh3d(icon_mesh_h.clone()),
            MeshMaterial3d(icon_mat_h.clone()),
            bevy::pbr::NotShadowCaster,
            bevy::pbr::NotShadowReceiver,
            bevy::render::view::NoFrustumCulling,
            Transform::IDENTITY,
            Visibility::Hidden,
        ));
        commands.spawn((
            StrategicSelectionHighlightEntity,
            Mesh3d(ring_mesh_h.clone()),
            MeshMaterial3d(ring_mat_h.clone()),
            bevy::pbr::NotShadowCaster,
            bevy::pbr::NotShadowReceiver,
            bevy::render::view::NoFrustumCulling,
            Transform::IDENTITY,
            Visibility::Hidden,
        ));

        gpu_res.atlas_image = atlas_h;
        gpu_res.batch_mesh = icon_mesh_h;
        gpu_res.batch_material = icon_mat_h;
        gpu_res.ring_mesh = ring_mesh_h;
        gpu_res.ring_material = ring_mat_h;
    }

    let Some((rig, cam_tf)) = cam_q.iter().next() else {
        for mut v in &mut batch_q {
            *v = Visibility::Hidden;
        }
        for mut v in &mut sel_q {
            *v = Visibility::Hidden;
        }
        return;
    };

    let blend = rig.strategic_zoom_blend();
    if blend <= 0.002 {
        for mut v in &mut batch_q {
            if *v != Visibility::Hidden {
                *v = Visibility::Hidden;
            }
        }
        for mut v in &mut sel_q {
            if *v != Visibility::Hidden {
                *v = Visibility::Hidden;
            }
        }
        return;
    }

    let world = &sim_res.0;
    let loaded = &mod_res.0;
    let cb_mode = cb_settings
        .as_deref()
        .map(|s| s.mode)
        .unwrap_or(ColorblindPaletteMode::Off);
    let local_pid = interaction
        .as_deref()
        .map(|i| i.local_player_id)
        .unwrap_or(1);
    let selected_ids = interaction.as_deref().map(|i| &i.selected_ids);

    // Camera-aligned billboard basis so icons stay upright and distortion-free at any yaw/pitch (`60°..88°`)
    let cam_right = cam_tf.right().as_vec3();
    let cam_up = cam_tf.up().as_vec3();
    let base_half_size = (rig.zoom_distance() * 0.0145).clamp(1.65, 6.80);

    let cap = world.entities.len().max(16);
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(cap * 4);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(cap * 4);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(cap * 4);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(cap * 4);
    let mut indices: Vec<u32> = Vec::with_capacity(cap * 6);

    let mut sel_pos: Vec<[f32; 3]> = Vec::new();
    let mut sel_norm: Vec<[f32; 3]> = Vec::new();
    let mut sel_uvs: Vec<[f32; 2]> = Vec::new();
    let mut sel_cols: Vec<[f32; 4]> = Vec::new();
    let mut sel_idx: Vec<u32> = Vec::new();

    for ent in world.entities.values() {
        if ent.transported_by.is_some() {
            continue;
        }
        let mut explored_dim = 1.0_f32;
        if let Some(ref fow) = cached_fow {
            if !fow.is_entity_visible(world, loaded, ent) {
                continue;
            }
            if fow.fog_enabled
                && matches!(ent.kind, EntityKind::Building { .. })
                && fow.world_pos_visibility(world, ent.position) == TileVisibility::Explored
            {
                explored_dim = 0.55;
            }
        }

        let cat = classify_entity_strategic_icon(world, loaded, ent);
        let (u0, u1) = cat.atlas_uv_bounds();
        let half = base_half_size * cat.size_multiplier();
        let slot = resolve_player_team_color_slot(world, loaded, ent.player_id);
        let is_local = ent.player_id == local_pid;
        let is_ally = ent.player_id != 0 && !world.are_hostile(local_pid, ent.player_id);
        let team_rgba = resolve_accessible_team_color(slot, is_local, is_ally, cb_mode);
        let col = [
            team_rgba[0] * explored_dim,
            team_rgba[1] * explored_dim,
            team_rgba[2] * explored_dim,
            blend * explored_dim,
        ];

        // Lift icons cleanly above terrain & building roofs so depth test never clips them
        let lift_y = match cat {
            StrategicIconCategory::Air => 3.8,
            StrategicIconCategory::Hero => 3.4,
            StrategicIconCategory::Building => 3.1,
            _ => 2.5,
        };
        let center = Vec3::new(ent.position[0], ent.position[1] + lift_y, ent.position[2]);
        let r_vec = cam_right * half;
        let u_vec = cam_up * half;

        let v_base = positions.len() as u32;
        positions.extend_from_slice(&[
            (center - r_vec - u_vec).to_array(),
            (center + r_vec - u_vec).to_array(),
            (center + r_vec + u_vec).to_array(),
            (center - r_vec + u_vec).to_array(),
        ]);
        normals.extend_from_slice(&[[0.0, 1.0, 0.0]; 4]);
        uvs.extend_from_slice(&[[u0, 1.0], [u1, 1.0], [u1, 0.0], [u0, 0.0]]);
        colors.extend_from_slice(&[col; 4]);
        indices.extend_from_slice(&[
            v_base,
            v_base + 1,
            v_base + 2,
            v_base,
            v_base + 2,
            v_base + 3,
        ]);

        if selected_ids.is_some_and(|s| s.contains(&ent.id)) {
            let outer = half * 1.30;
            let inner = half * 1.08;
            let hi_col = [0.95, 1.0, 0.88, blend * 0.96];
            let corners = [(-1.0_f32, -1.0_f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
            let sb = sel_pos.len() as u32;
            for &(sx, sy) in &corners {
                let po = center + cam_right * (sx * outer) + cam_up * (sy * outer);
                let pi = center + cam_right * (sx * inner) + cam_up * (sy * inner);
                sel_pos.push(po.to_array());
                sel_pos.push(pi.to_array());
                sel_norm.extend_from_slice(&[[0.0, 1.0, 0.0]; 2]);
                sel_uvs.extend_from_slice(&[[0.5, 0.5]; 2]);
                sel_cols.extend_from_slice(&[hi_col; 2]);
            }
            for side in 0..4u32 {
                let a = sb + side * 2;
                let b = sb + ((side + 1) % 4) * 2;
                sel_idx.extend_from_slice(&[a, b, b + 1, a, b + 1, a + 1]);
            }
        }
    }

    let has_icons = !indices.is_empty();
    let has_sel = !sel_idx.is_empty();

    if let Some(m) = meshes.get_mut(&gpu_res.batch_mesh) {
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        m.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        m.insert_indices(Indices::U32(indices));
    }
    if let Some(m) = meshes.get_mut(&gpu_res.ring_mesh) {
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, sel_pos);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, sel_norm);
        m.insert_attribute(Mesh::ATTRIBUTE_UV_0, sel_uvs);
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, sel_cols);
        m.insert_indices(Indices::U32(sel_idx));
    }

    for mut v in &mut batch_q {
        *v = if has_icons {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for mut v in &mut sel_q {
        *v = if has_sel {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn strategic_icon_atlas_renders_all_eight_crisp_categories() {
        let atlas = build_strategic_icon_atlas_image();
        assert_eq!(atlas.texture_descriptor.size.width, STRATEGIC_ATLAS_WIDTH);
        assert_eq!(atlas.texture_descriptor.size.height, STRATEGIC_ATLAS_HEIGHT);

        for cat in StrategicIconCategory::ALL {
            let x0 = (cat as u32) * STRATEGIC_ICON_TILE_PX;
            let mut opaque_pixels = 0usize;
            for py in 0..STRATEGIC_ICON_TILE_PX {
                for px in 0..STRATEGIC_ICON_TILE_PX {
                    let idx = (((py * STRATEGIC_ATLAS_WIDTH) + (x0 + px)) as usize) * 4;
                    if atlas.data[idx + 3] > 180 {
                        opaque_pixels += 1;
                    }
                }
            }
            assert!(
                opaque_pixels >= 250,
                "Strategic icon {:?} must have crisp non-empty silhouette coverage, got {}",
                cat,
                opaque_pixels
            );
        }
    }

    #[test]
    fn classify_entity_strategic_icon_distinguishes_units_buildings_harvesters_and_heroes() {
        let mod_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../rust-ra2")
            .canonicalize()
            .expect("canonicalize rust-ra2");
        let loaded = LoadedMod::load_from_dir(mod_dir).expect("load rust-ra2");
        let mut world = SimWorld::from_mod_map(&loaded, &loaded.entry_map);

        let tank_id = world
            .spawn_unit(1, "grizzly_tank", [10, 10], &loaded)
            .expect("spawn tank");
        assert_eq!(
            classify_entity_strategic_icon(&world, &loaded, &world.entities[&tank_id]),
            StrategicIconCategory::TankArmor
        );

        let inf_uid = loaded
            .units
            .iter()
            .find(|(_, u)| u.role == UnitRole::Infantry && u.hero_trait.is_none())
            .map(|(id, _)| id.clone())
            .expect("infantry unit in rust-ra2");
        let inf_id = (10..24)
            .find_map(|tx| world.spawn_unit(1, &inf_uid, [tx, 11], &loaded))
            .expect("spawn infantry");
        assert_eq!(
            classify_entity_strategic_icon(&world, &loaded, &world.entities[&inf_id]),
            StrategicIconCategory::Infantry
        );

        let harv_id = (10..24)
            .find_map(|tx| world.spawn_unit(1, "chrono_miner", [tx, 12], &loaded))
            .expect("spawn miner");
        assert_eq!(
            classify_entity_strategic_icon(&world, &loaded, &world.entities[&harv_id]),
            StrategicIconCategory::HarvesterResource
        );

        // Elevating an entity into `trait_stores.heroes` promotes its icon to Hero star/crown
        world.trait_stores.heroes.insert(
            tank_id,
            rts_simulation::world::entity_traits::HeroInstanceState::default(),
        );
        assert_eq!(
            classify_entity_strategic_icon(&world, &loaded, &world.entities[&tank_id]),
            StrategicIconCategory::Hero
        );
    }
}
