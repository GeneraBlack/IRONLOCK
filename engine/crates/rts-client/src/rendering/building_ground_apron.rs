use super::preview_stage::PreviewStageEntity;
use super::{
    FogOfWarPlaneEntity, PlacementGhostModel, Ra2EntityVisualMode, Ra2ShpSpriteCatalog,
    SimVisualEntity,
};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::HashMap;

/// Standalone soft ground-contact AO + industrial grime/apron quad anchored beneath 3D buildings
/// in `Full3dMeshes` mode so rectangular concrete plinths blend naturally into terrain tiles
/// just like Westwood's pre-rendered `.shp` contact skirts (`Next-Gen 3D Overhaul v2`).
#[derive(Component, Debug, Clone, Copy)]
pub struct BuildingGroundApron {
    pub sim_id: u64,
}

#[derive(Resource, Default, Debug, Clone)]
pub struct BuildingGroundApronAssets {
    pub initialized: bool,
    pub quad_mesh: Handle<Mesh>,
    pub apron_material: Handle<StandardMaterial>,
    pub vehicle_shadow_material: Handle<StandardMaterial>,
}

#[inline]
fn hash21(x: f32, y: f32) -> f32 {
    let mut p3 = (Vec3::new(x, y, x) * 0.1031).fract();
    p3 += Vec3::splat(p3.dot(Vec3::new(p3.y + 33.33, p3.z + 33.33, p3.x + 33.33)));
    ((p3.x + p3.y) * p3.z).fract()
}

fn value_noise2(x: f32, y: f32) -> f32 {
    let ix = x.floor();
    let iy = y.floor();
    let fx = x - ix;
    let fy = y - iy;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let a = hash21(ix, iy);
    let b = hash21(ix + 1.0, iy);
    let c = hash21(ix, iy + 1.0);
    let d = hash21(ix + 1.0, iy + 1.0);
    let ab = a + (b - a) * ux;
    let cd = c + (d - c) * ux;
    ab + (cd - ab) * uy
}

/// Builds a `128x128` `Rgba8UnormSrgb` soft rounded-box ground-contact AO + weathered asphalt/oil
/// skirt texture. Inner core (`|nx|, |nz| <= 0.74`) sits under the 3D plinth; the outer `0.74..1.0`
/// band feathers a rich dark contact shadow + subtle industrial grime ring into the terrain.
pub fn build_building_ground_apron_image() -> Image {
    const SIZE: u32 = 128;
    let mut bytes = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for py in 0..SIZE {
        let nz = ((py as f32 + 0.5) / SIZE as f32) * 2.0 - 1.0;
        for px in 0..SIZE {
            let nx = ((px as f32 + 0.5) / SIZE as f32) * 2.0 - 1.0;
            // Rounded box signed distance (`half_ext = 0.74`, `corner_r = 0.10`)
            let qx = nx.abs() - 0.64;
            let qz = nz.abs() - 0.64;
            let outside = Vec2::new(qx.max(0.0), qz.max(0.0)).length();
            let inside = qx.max(qz).min(0.0);
            let sdf = outside + inside - 0.10; // <= 0 inside plinth, ~0.26 at quad border

            // Slight NW->SE directional shadow bias matching RA2 upper-left sun
            let se_bias = ((nx * 0.25 + nz * 0.35) * 0.045).clamp(-0.03, 0.05);
            let dist_out = (sdf - se_bias).max(0.0);

            let grime = value_noise2((nx + 1.0) * 6.5, (nz + 1.0) * 6.5) * 0.65
                + value_noise2((nx + 2.3) * 14.0, (nz + 1.7) * 14.0) * 0.35;

            // Smooth cubic falloff across outer `0.24` skirt
            let t = (1.0 - (dist_out / 0.245).clamp(0.0, 1.0)).powi(2);
            let contact_ring = (1.0 - (sdf.abs() / 0.095).clamp(0.0, 1.0)) * 0.22;
            let alpha_f = ((t * (0.48 + 0.18 * grime) + contact_ring)
                * (1.0 - (dist_out / 0.255).clamp(0.0, 1.0)))
            .clamp(0.0, 0.70);

            // Warm charcoal asphalt/soot tone (`sRGB ~ 18..34`) so it darkens concrete/grass naturally
            let shade = (16.0 + grime * 16.0).round().clamp(12.0, 36.0) as u8;
            let r = shade;
            let g = shade.saturating_sub(1);
            let b = shade.saturating_sub(3);
            let a = (alpha_f * 255.0).round().clamp(0.0, 255.0) as u8;
            bytes.extend_from_slice(&[r, g, b, a]);
        }
    }
    Image::new(
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        bytes,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// Builds a `64x64` soft oval chassis underbody contact AO shadow for heavy ground vehicles
/// (`grizzly_tank`, `apocalypse_tank`, `rhino_tank`, `prism_tank`, harvesters) so tank tracks
/// sit with grounded 60-ton weight on pavement and grass.
pub fn build_vehicle_underbody_ao_image() -> Image {
    const SIZE: u32 = 64;
    let mut bytes = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for py in 0..SIZE {
        let nz = ((py as f32 + 0.5) / SIZE as f32) * 2.0 - 1.0;
        for px in 0..SIZE {
            let nx = ((px as f32 + 0.5) / SIZE as f32) * 2.0 - 1.0;
            // Rounded super-ellipse matching twin tank treads + hull belly
            let rx = (nx / 0.84).abs();
            let rz = (nz / 0.88).abs();
            let d = (rx.powf(2.4) + rz.powf(2.4)).powf(1.0 / 2.4);
            let falloff = (1.0 - ((d - 0.42) / 0.56).clamp(0.0, 1.0)).powi(2);
            let alpha_f = (falloff * 0.56).clamp(0.0, 0.56);
            let a = (alpha_f * 255.0).round() as u8;
            bytes.extend_from_slice(&[12, 13, 16, a]);
        }
    }
    Image::new(
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        bytes,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

pub fn build_unit_horizontal_apron_quad_mesh() -> Mesh {
    let positions = vec![
        [-0.5, 0.0, -0.5],
        [0.5, 0.0, -0.5],
        [0.5, 0.0, 0.5],
        [-0.5, 0.0, 0.5],
    ];
    let normals = vec![[0.0, 1.0, 0.0]; 4];
    let uvs = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    // Counter-clockwise upward-facing (+Y) quad winding in right-handed Bevy world space
    let indices = vec![0u32, 3, 2, 0, 2, 1];
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_building_ground_aprons(
    mut commands: Commands,
    mut apron_assets: ResMut<BuildingGroundApronAssets>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
    mut images: Option<ResMut<Assets<Image>>>,
    sim_res: Option<Res<SimWorldResource>>,
    mod_res: Option<Res<ModResource>>,
    visual_mode: Option<Res<Ra2EntityVisualMode>>,
    sprite_catalog: Option<Res<Ra2ShpSpriteCatalog>>,
    buildings_q: Query<
        (&SimVisualEntity, &Transform, &Visibility),
        (
            Without<BuildingGroundApron>,
            Without<FogOfWarPlaneEntity>,
            Without<PlacementGhostModel>,
            Without<PreviewStageEntity>,
        ),
    >,
    mut apron_q: Query<
        (
            Entity,
            &BuildingGroundApron,
            &mut Transform,
            &mut Visibility,
        ),
        (
            Without<SimVisualEntity>,
            Without<FogOfWarPlaneEntity>,
            Without<PlacementGhostModel>,
            Without<PreviewStageEntity>,
        ),
    >,
) {
    if !apron_assets.initialized {
        let (Some(ref mut m_assets), Some(ref mut mat_assets), Some(ref mut img_assets)) =
            (&mut meshes, &mut materials, &mut images)
        else {
            return;
        };
        let tex_h = img_assets.add(build_building_ground_apron_image());
        let veh_tex_h = img_assets.add(build_vehicle_underbody_ao_image());
        apron_assets.quad_mesh = m_assets.add(build_unit_horizontal_apron_quad_mesh());
        apron_assets.apron_material = mat_assets.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(tex_h),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.96,
            metallic: 0.0,
            reflectance: 0.04,
            depth_bias: 16.0,
            ..default()
        });
        apron_assets.vehicle_shadow_material = mat_assets.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(veh_tex_h),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.98,
            metallic: 0.0,
            reflectance: 0.02,
            depth_bias: 18.0,
            ..default()
        });
        apron_assets.initialized = true;
    }

    let tile_size = sim_res
        .as_deref()
        .map(|s| s.0.grid.tile_size.max(0.5))
        .unwrap_or(2.0);
    let is_hybrid_sprites =
        visual_mode.as_deref().copied().unwrap_or_default() == Ra2EntityVisualMode::HybridHdSprites;

    // `(sx, sz, world_pos, yaw, root_vis, hide_apron, is_vehicle)`
    let mut active_contacts: HashMap<u64, (f32, f32, Vec3, f32, Visibility, bool, bool)> =
        HashMap::new();
    for (vis, tf, root_vis) in &buildings_q {
        let (_, yaw_rot, _) = tf.rotation.to_euler(EulerRot::YXZ);
        if let Some(fp) = vis.building_footprint {
            let covered_by_hd_sprite = is_hybrid_sprites
                && sprite_catalog
                    .as_deref()
                    .is_some_and(|cat| cat.covers_entity(&vis.entity_type_id, true));
            let prog = vis.spawn_progress.clamp(0.25, 1.0);
            let sx = fp[0].max(1) as f32 * tile_size * 1.26;
            let sz = fp[1].max(1) as f32 * tile_size * 1.26;
            active_contacts.insert(
                vis.sim_id,
                (
                    sx,
                    sz,
                    tf.translation,
                    yaw_rot,
                    *root_vis,
                    covered_by_hd_sprite || prog < 0.15,
                    false,
                ),
            );
        } else if !vis.is_infantry {
            let is_ground_vehicle = mod_res
                .as_deref()
                .and_then(|m| m.0.units.get(&vis.entity_type_id))
                .is_some_and(|u| {
                    matches!(
                        u.movement_domain,
                        rts_sdk::schema::MovementDomain::Ground
                            | rts_sdk::schema::MovementDomain::Chrono
                            | rts_sdk::schema::MovementDomain::Amphibious
                    ) && matches!(
                        u.role,
                        rts_sdk::schema::UnitRole::Vehicle | rts_sdk::schema::UnitRole::Harvester
                    )
                });
            if is_ground_vehicle {
                active_contacts.insert(
                    vis.sim_id,
                    (2.55, 3.45, tf.translation, yaw_rot, *root_vis, false, true),
                );
            }
        }
    }

    for (ent, apron, mut tf, mut vis) in &mut apron_q {
        if let Some((sx, sz, b_pos, yaw, root_vis, hide_for_sprite, _is_veh)) =
            active_contacts.remove(&apron.sim_id)
        {
            let target_vis = if hide_for_sprite || root_vis == Visibility::Hidden {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
            if *vis != target_vis {
                *vis = target_vis;
            }
            tf.translation = Vec3::new(b_pos.x, b_pos.y + 0.020, b_pos.z);
            tf.rotation = Quat::from_rotation_y(yaw);
            tf.scale = Vec3::new(sx, 1.0, sz);
        } else {
            commands.entity(ent).despawn_recursive();
        }
    }

    for (sim_id, (sx, sz, b_pos, yaw, root_vis, hide_for_sprite, is_veh)) in active_contacts {
        let target_vis = if hide_for_sprite || root_vis == Visibility::Hidden {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        let mat_h = if is_veh {
            apron_assets.vehicle_shadow_material.clone()
        } else {
            apron_assets.apron_material.clone()
        };
        commands.spawn((
            BuildingGroundApron { sim_id },
            Mesh3d(apron_assets.quad_mesh.clone()),
            MeshMaterial3d(mat_h),
            bevy::pbr::NotShadowCaster,
            target_vis,
            Transform::from_xyz(b_pos.x, b_pos.y + 0.020, b_pos.z)
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::new(sx, 1.0, sz)),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ground_apron_texture_has_dark_core_and_transparent_feathered_border() {
        let img = build_building_ground_apron_image();
        assert_eq!(img.texture_descriptor.size.width, 128);
        assert_eq!(img.texture_descriptor.size.height, 128);
        assert_eq!(img.data.len(), 128 * 128 * 4);

        let center_alpha = img.data[(64 * 128 + 64) * 4 + 3];
        let border_alpha = img.data[3]; // corner (0, 0)
        assert!(
            center_alpha > 100 && center_alpha < 200,
            "center_alpha={center_alpha}"
        );
        assert_eq!(
            border_alpha, 0,
            "outer corner must feather cleanly to 0 alpha"
        );
    }
}
