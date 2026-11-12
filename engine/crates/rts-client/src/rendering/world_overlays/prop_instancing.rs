use super::batch_builder::DynamicQuadBatchBuilder;
use crate::rendering::{resolve_mod_asset_path, CachedFogVisibility, MapEnvironmentEntity};
use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::math::Affine3A;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::render::render_asset::RenderAssetUsages;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{CrateKind, PropKind};
use rts_simulation::world::{SimWorld, TileVisibility};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Component)]
pub struct PropSceneVisualEntity {
    pub prop_key: PropInstanceKey,
    pub model_file: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropInstanceKey {
    MapProp(usize),
    Crate(u64),
    Mine(usize),
    CivilianStructure(usize),
    GridOreTile([i32; 2]),
    ResourceNode([i32; 2]),
}

pub const ALL_PROP_GLB_FILES: [&str; 16] = [
    "tree_temperate.glb",
    "tree_pine.glb",
    "streetlight.glb",
    "explosive_barrel.glb",
    "supply_crate.glb",
    "landmine.glb",
    "civilian_building_intact.glb",
    "civilian_building_ruin.glb",
    "ore_stage_1.glb",
    "ore_stage_2.glb",
    "ore_stage_3.glb",
    "ore_stage_4.glb",
    "gem_stage_1.glb",
    "gem_stage_2.glb",
    "gem_stage_3.glb",
    "gem_stage_4.glb",
];

/// Computes the 4-stage visual crystal cluster density (`1..=4`) from remaining tile/node amount.
#[inline]
pub fn resource_density_stage(remaining: u32, max_reference: u32) -> u8 {
    if remaining == 0 {
        return 0;
    }
    let denom = max_reference.max(1) as f32;
    let ratio = (remaining as f32 / denom).clamp(0.01, 1.0);
    if ratio > 0.75 {
        4
    } else if ratio > 0.50 {
        3
    } else if ratio > 0.25 {
        2
    } else {
        1
    }
}

pub fn ore_stage_glb_filename(is_gem: bool, stage: u8) -> &'static str {
    match (is_gem, stage.clamp(1, 4)) {
        (false, 1) => "ore_stage_1.glb",
        (false, 2) => "ore_stage_2.glb",
        (false, 3) => "ore_stage_3.glb",
        (false, _) => "ore_stage_4.glb",
        (true, 1) => "gem_stage_1.glb",
        (true, 2) => "gem_stage_2.glb",
        (true, 3) => "gem_stage_3.glb",
        (true, _) => "gem_stage_4.glb",
    }
}

pub(super) fn resolve_prop_gltf_asset_path(
    asset_root: &Path,
    mod_root: &Path,
    file_name: &str,
) -> Option<String> {
    let rel = format!("assets/models/props/{file_name}");
    let scene_str = if mod_root.join(&rel).is_file() {
        Some(resolve_mod_asset_path(asset_root, mod_root, &rel))
    } else if Path::new("rust-ra2").join(&rel).is_file() {
        Some(resolve_mod_asset_path(
            asset_root,
            Path::new("rust-ra2"),
            &rel,
        ))
    } else {
        let ws_ra2 = asset_root.join("rust-ra2");
        ws_ra2
            .join(&rel)
            .is_file()
            .then(|| resolve_mod_asset_path(asset_root, &ws_ra2, &rel))
    }?;
    Some(scene_str.trim_end_matches("#Scene0").to_string())
}

/// Synthesizes a single combined vertex-colored `Mesh` for `fname` so static props render as flat
/// GPU-instanced `(Mesh3d, MeshMaterial3d)` entities immediately with zero `SceneRoot` hierarchies (`Task 236`).
pub fn build_procedural_instanced_prop_mesh(fname: &'static str) -> Mesh {
    let mut b = DynamicQuadBatchBuilder::default();
    match fname {
        "tree_temperate.glb" => {
            b.push_oriented_box(
                Vec3::new(0.0, 0.65, 0.0),
                Vec3::new(0.14, 0.65, 0.14),
                Quat::IDENTITY,
                [0.38, 0.25, 0.14, 1.0],
            );
            b.push_oriented_box(
                Vec3::new(0.0, 1.65, 0.0),
                Vec3::new(0.72, 0.58, 0.72),
                Quat::from_rotation_y(0.4),
                [0.18, 0.52, 0.20, 1.0],
            );
            b.push_oriented_box(
                Vec3::new(0.0, 2.35, 0.0),
                Vec3::new(0.48, 0.42, 0.48),
                Quat::from_rotation_y(1.1),
                [0.24, 0.62, 0.24, 1.0],
            );
        }
        "tree_pine.glb" => {
            b.push_oriented_box(
                Vec3::new(0.0, 0.60, 0.0),
                Vec3::new(0.12, 0.60, 0.12),
                Quat::IDENTITY,
                [0.34, 0.22, 0.13, 1.0],
            );
            for (tier, (y, r)) in [(1.25, 0.68), (1.90, 0.50), (2.48, 0.30)]
                .into_iter()
                .enumerate()
            {
                b.push_oriented_box(
                    Vec3::new(0.0, y, 0.0),
                    Vec3::new(r, 0.36, r),
                    Quat::from_rotation_y(tier as f32 * 0.55),
                    [0.14, 0.44 + tier as f32 * 0.05, 0.22, 1.0],
                );
            }
        }
        "streetlight.glb" => {
            b.push_oriented_box(
                Vec3::new(0.0, 1.35, 0.0),
                Vec3::new(0.06, 1.35, 0.06),
                Quat::IDENTITY,
                [0.45, 0.48, 0.52, 1.0],
            );
            b.push_oriented_box(
                Vec3::new(0.28, 2.68, 0.0),
                Vec3::new(0.32, 0.05, 0.08),
                Quat::IDENTITY,
                [0.52, 0.55, 0.60, 1.0],
            );
            b.push_oriented_box(
                Vec3::new(0.52, 2.60, 0.0),
                Vec3::new(0.10, 0.06, 0.10),
                Quat::IDENTITY,
                [1.0, 0.92, 0.65, 1.0],
            );
        }
        "explosive_barrel.glb" => {
            b.push_oriented_box(
                Vec3::new(0.0, 0.42, 0.0),
                Vec3::new(0.26, 0.42, 0.26),
                Quat::IDENTITY,
                [0.86, 0.18, 0.14, 1.0],
            );
            b.push_oriented_box(
                Vec3::new(0.0, 0.42, 0.0),
                Vec3::new(0.28, 0.08, 0.28),
                Quat::IDENTITY,
                [0.95, 0.78, 0.16, 1.0],
            );
        }
        "supply_crate.glb" => {
            b.push_oriented_box(
                Vec3::new(0.0, 0.34, 0.0),
                Vec3::new(0.34, 0.34, 0.34),
                Quat::IDENTITY,
                [0.64, 0.48, 0.26, 1.0],
            );
            b.push_oriented_box(
                Vec3::new(0.0, 0.34, 0.0),
                Vec3::new(0.36, 0.07, 0.36),
                Quat::IDENTITY,
                [0.24, 0.72, 0.96, 1.0],
            );
        }
        "landmine.glb" => {
            b.push_oriented_box(
                Vec3::new(0.0, 0.06, 0.0),
                Vec3::new(0.22, 0.06, 0.22),
                Quat::IDENTITY,
                [0.28, 0.32, 0.26, 1.0],
            );
            b.push_oriented_box(
                Vec3::new(0.0, 0.13, 0.0),
                Vec3::new(0.06, 0.02, 0.06),
                Quat::IDENTITY,
                [0.98, 0.22, 0.16, 1.0],
            );
        }
        "civilian_building_intact.glb" => {
            b.push_oriented_box(
                Vec3::new(0.0, 1.45, 0.0),
                Vec3::new(0.95, 1.45, 0.95),
                Quat::IDENTITY,
                [0.72, 0.70, 0.66, 1.0],
            );
            b.push_oriented_box(
                Vec3::new(0.0, 2.98, 0.0),
                Vec3::new(1.02, 0.10, 1.02),
                Quat::IDENTITY,
                [0.42, 0.44, 0.48, 1.0],
            );
        }
        "civilian_building_ruin.glb" => {
            b.push_oriented_box(
                Vec3::new(-0.22, 0.55, 0.14),
                Vec3::new(0.86, 0.55, 0.82),
                Quat::from_rotation_z(0.12),
                [0.38, 0.36, 0.34, 1.0],
            );
            b.push_oriented_box(
                Vec3::new(0.30, 0.28, -0.25),
                Vec3::new(0.55, 0.28, 0.52),
                Quat::from_rotation_y(0.45),
                [0.28, 0.26, 0.25, 1.0],
            );
        }
        _ => {
            let is_gem = fname.starts_with("gem_");
            let stage: usize = fname
                .chars()
                .find(|c| c.is_ascii_digit())
                .and_then(|c| c.to_digit(10))
                .unwrap_or(2) as usize;
            let count = (stage * 2).clamp(2, 8);
            let col = if is_gem {
                [0.94, 0.24, 0.98, 1.0]
            } else {
                [0.98, 0.80, 0.16, 1.0]
            };
            for i in 0..count {
                let ang = (i as f32 / count as f32) * std::f32::consts::TAU + 0.35;
                let r = if i == 0 { 0.0 } else { 0.34 };
                let h = 0.18 + (stage as f32) * 0.06 + (i % 2) as f32 * 0.05;
                b.push_oriented_box(
                    Vec3::new(ang.cos() * r, h * 0.5, ang.sin() * r),
                    Vec3::new(0.14, h * 0.5, 0.14),
                    Quat::from_rotation_y(ang) * Quat::from_rotation_z(0.18),
                    col,
                );
            }
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    b.write_into_mesh(&mut mesh);
    mesh
}

/// Flattens all primitives across all nodes in a loaded `Gltf` prop asset into ONE single vertex-colored
/// `Mesh` so Bevy's GPU mesh instancer renders all copies in 1 instanced draw call with 0 `SceneRoot` entities!
pub(super) fn merge_gltf_prop_into_single_mesh(
    gltf: &Gltf,
    gltf_nodes: &Assets<GltfNode>,
    gltf_meshes: &Assets<GltfMesh>,
    meshes: &Assets<Mesh>,
    materials: &Assets<StandardMaterial>,
) -> Option<Mesh> {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let mut append_gmesh = |gmesh: &GltfMesh, xf: Affine3A| {
        for prim in &gmesh.primitives {
            let Some(src_mesh) = meshes.get(&prim.mesh) else {
                continue;
            };
            let Some(VertexAttributeValues::Float32x3(src_pos)) =
                src_mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                continue;
            };
            let src_nrm = match src_mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
                Some(VertexAttributeValues::Float32x3(n)) if n.len() == src_pos.len() => {
                    Some(n.as_slice())
                }
                _ => None,
            };
            let src_col = match src_mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
                Some(VertexAttributeValues::Float32x4(c)) if c.len() == src_pos.len() => {
                    Some(c.as_slice())
                }
                _ => None,
            };
            let mat_tint = prim
                .material
                .as_ref()
                .and_then(|mh| materials.get(mh))
                .map(|m| m.base_color.to_linear().to_f32_array())
                .unwrap_or([1.0, 1.0, 1.0, 1.0]);

            let base_v = positions.len() as u32;
            for (i, &p) in src_pos.iter().enumerate() {
                let wp = xf.transform_point3(Vec3::from_array(p));
                positions.push(wp.to_array());
                let n = src_nrm
                    .map(|ns| {
                        xf.transform_vector3(Vec3::from_array(ns[i]))
                            .normalize_or_zero()
                    })
                    .unwrap_or(Vec3::Y);
                normals.push(n.to_array());
                let vc = src_col.map(|cs| cs[i]).unwrap_or([1.0, 1.0, 1.0, 1.0]);
                colors.push([
                    vc[0] * mat_tint[0],
                    vc[1] * mat_tint[1],
                    vc[2] * mat_tint[2],
                    vc[3] * mat_tint[3],
                ]);
            }
            match src_mesh.indices() {
                Some(Indices::U32(ix)) => {
                    indices.extend(ix.iter().map(|&idx| base_v + idx));
                }
                Some(Indices::U16(ix)) => {
                    indices.extend(ix.iter().map(|&idx| base_v + idx as u32));
                }
                None => {
                    indices.extend((0..src_pos.len() as u32).map(|idx| base_v + idx));
                }
            }
        }
    };

    let mut visited_any_node = false;
    for node_h in &gltf.nodes {
        if let Some(node) = gltf_nodes.get(node_h) {
            if let Some(ref gm_h) = node.mesh {
                if let Some(gmesh) = gltf_meshes.get(gm_h) {
                    visited_any_node = true;
                    append_gmesh(gmesh, Affine3A::from_mat4(node.transform.compute_matrix()));
                }
            }
        }
    }
    if !visited_any_node {
        for gm_h in &gltf.meshes {
            if let Some(gmesh) = gltf_meshes.get(gm_h) {
                append_gmesh(gmesh, Affine3A::IDENTITY);
            }
        }
    }

    if positions.is_empty() || indices.is_empty() {
        return None;
    }

    let mut combined = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    combined.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    combined.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    combined.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    combined.insert_indices(Indices::U32(indices));
    Some(combined)
}

/// Synchronizes all static & semi-static map objects (trees, rocks/streetlights, explosive barrels,
/// bonus crates, landmines, civilian ruins, and 4-stage ore/gem clusters) as flat GPU-instanced
/// `(Mesh3d, MeshMaterial3d)` entities without `SceneRoot` hierarchies (`Task 236`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn sync_instanced_static_props(
    commands: &mut Commands,
    world: &SimWorld,
    loaded: &LoadedMod,
    cached_fow: &CachedFogVisibility,
    local_pid: u8,
    elapsed: f32,
    prop_instanced_meshes: &HashMap<&'static str, (Handle<Mesh>, Handle<StandardMaterial>)>,
    prop_vis_q: &mut Query<
        (
            Entity,
            &mut PropSceneVisualEntity,
            &mut Mesh3d,
            &mut MeshMaterial3d<StandardMaterial>,
            &mut Transform,
            &mut Visibility,
        ),
        (
            Without<super::SimProjectileVisualEntity>,
            Without<crate::rendering::preview_stage::PreviewStageEntity>,
        ),
    >,
) {
    if prop_instanced_meshes.is_empty() {
        return;
    }

    let mut desired_props: HashMap<PropInstanceKey, (&'static str, Transform, Visibility)> =
        HashMap::new();

    // 1. Destructible & Explosive Map Props (`world.props`)
    for (idx, prop) in world.props.iter().enumerate() {
        let fname: &'static str = match prop.kind {
            PropKind::Tree => {
                let h = (idx as u32).wrapping_mul(2654435761);
                if h & 1 == 0 {
                    "tree_temperate.glb"
                } else {
                    "tree_pine.glb"
                }
            }
            PropKind::Streetlight => "streetlight.glb",
            PropKind::ExplosiveBarrel => "explosive_barrel.glb",
            #[allow(unreachable_patterns)]
            _ => "tree_temperate.glb",
        };
        let visible =
            cached_fow.world_pos_visibility(world, prop.position) != TileVisibility::Unexplored;
        let yaw = ((idx as f32) * 1.618) % std::f32::consts::TAU;
        desired_props.insert(
            PropInstanceKey::MapProp(idx),
            (
                fname,
                Transform::from_xyz(prop.position[0], prop.position[1], prop.position[2])
                    .with_rotation(Quat::from_rotation_y(yaw)),
                if visible {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
            ),
        );
    }

    // 2. Collectible Bonus Crates (`world.crates`)
    for crate_item in &world.crates {
        let visible = cached_fow.is_world_pos_visible(world, crate_item.position);
        let _ = matches!(crate_item.kind, CrateKind::BonusCredits);
        let bob = (elapsed * 2.4 + crate_item.id as f32).sin() * 0.08;
        desired_props.insert(
            PropInstanceKey::Crate(crate_item.id),
            (
                "supply_crate.glb",
                Transform::from_xyz(
                    crate_item.position[0],
                    crate_item.position[1] + bob,
                    crate_item.position[2],
                )
                .with_rotation(Quat::from_rotation_y(elapsed * 1.1 + crate_item.id as f32)),
                if visible {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
            ),
        );
    }

    // 3. Landmines (`world.mines`)
    for (idx, mine) in world.mines.iter().enumerate() {
        let friendly = mine.owner_player_id == local_pid
            || !world.are_hostile(local_pid, mine.owner_player_id);
        let visible = friendly
            || (mine.revealed_to_enemies && cached_fow.is_world_pos_visible(world, mine.position));
        desired_props.insert(
            PropInstanceKey::Mine(idx),
            (
                "landmine.glb",
                Transform::from_xyz(mine.position[0], mine.position[1], mine.position[2]),
                if visible {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
            ),
        );
    }

    // 4. Destructible Civilian Structures (`civilian_building_intact.glb` -> `civilian_building_ruin.glb`)
    for (idx, civ) in world.civilian_structures.iter().enumerate() {
        if !civ.is_ruin {
            let synced_as_catalog_entity = world
                .civilian_structure_entity_id(civ.id)
                .and_then(|eid| world.entities.get(&eid))
                .is_some_and(|se| loaded.buildings.contains_key(se.def_id()));
            if synced_as_catalog_entity {
                continue;
            }
        }
        let fname: &'static str = if civ.is_ruin {
            "civilian_building_ruin.glb"
        } else {
            "civilian_building_intact.glb"
        };
        let center = world
            .grid
            .footprint_center_world(civ.tile_pos, civ.footprint);
        let ground_y = world.grid.sample_world_height(center[0], center[2]);
        let visible = cached_fow.world_pos_visibility(world, center) != TileVisibility::Unexplored;
        let scale_mult = (civ.footprint[0].max(civ.footprint[1]).max(1) as f32) * 0.48;
        desired_props.insert(
            PropInstanceKey::CivilianStructure(idx),
            (
                fname,
                Transform::from_xyz(center[0], ground_y, center[2])
                    .with_scale(Vec3::splat(scale_mult)),
                if visible {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
            ),
        );
    }

    // 5. 4-Stage Resource Clusters (`ore_stage_1..4.glb` / `gem_stage_1..4.glb`)
    for (&tile, &amt) in &world.grid.ore_tiles {
        if amt == 0 {
            continue;
        }
        let is_gem = world.grid.gem_tiles.contains(&tile);
        let max_ref = if is_gem { 1_200 } else { 800 };
        let stage = resource_density_stage(amt, max_ref);
        let fname = ore_stage_glb_filename(is_gem, stage);
        let pos = world.grid.tile_to_world(tile);
        let visible = cached_fow.tile_visibility(tile) != TileVisibility::Unexplored;
        let seed = (tile[0] as u32)
            .wrapping_mul(2654435761)
            .wrapping_add((tile[1] as u32).wrapping_mul(2246822519));
        let yaw = ((seed & 0xff) as f32 / 255.0) * std::f32::consts::TAU;
        desired_props.insert(
            PropInstanceKey::GridOreTile(tile),
            (
                fname,
                Transform::from_xyz(pos[0], pos[1], pos[2])
                    .with_rotation(Quat::from_rotation_y(yaw)),
                if visible {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
            ),
        );
    }

    for (res_id, tile_map) in &world.resource_nodes {
        let is_high_tier = res_id.contains("gem") || res_id.contains("alloy");
        for (&ntile, &remaining) in tile_map {
            if remaining == 0 || world.grid.ore_tiles.contains_key(&ntile) {
                continue;
            }
            let stage = resource_density_stage(remaining, 1_000);
            let fname = ore_stage_glb_filename(is_high_tier, stage);
            let rpos = world.grid.tile_to_world(ntile);
            let visible = cached_fow.tile_visibility(ntile) != TileVisibility::Unexplored;
            let seed = (ntile[0] as u32)
                .wrapping_mul(2654435761)
                .wrapping_add((ntile[1] as u32).wrapping_mul(2246822519));
            let yaw = ((seed & 0xff) as f32 / 255.0) * std::f32::consts::TAU;
            desired_props.insert(
                PropInstanceKey::ResourceNode(ntile),
                (
                    fname,
                    Transform::from_xyz(rpos[0], rpos[1], rpos[2])
                        .with_rotation(Quat::from_rotation_y(yaw)),
                    if visible {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    },
                ),
            );
        }
    }

    let mut matched_keys = HashSet::with_capacity(desired_props.len());
    for (ent, mut p_vis, mut mesh_3d, mut mat_3d, mut tf, mut vis) in prop_vis_q.iter_mut() {
        if let Some(&(fname, target_tf, target_vis)) = desired_props.get(&p_vis.prop_key) {
            matched_keys.insert(p_vis.prop_key);
            if let Some((want_mesh, want_mat)) = prop_instanced_meshes.get(fname) {
                if p_vis.model_file != fname || mesh_3d.0 != *want_mesh {
                    p_vis.model_file = fname;
                    mesh_3d.0 = want_mesh.clone();
                    mat_3d.0 = want_mat.clone();
                }
            }
            if *tf != target_tf {
                *tf = target_tf;
            }
            if *vis != target_vis {
                *vis = target_vis;
            }
        } else {
            commands.entity(ent).despawn_recursive();
        }
    }

    for (key, (fname, target_tf, target_vis)) in desired_props {
        if matched_keys.contains(&key) {
            continue;
        }
        if let Some((mesh_h, mat_h)) = prop_instanced_meshes.get(fname) {
            commands.spawn((
                MapEnvironmentEntity,
                PropSceneVisualEntity {
                    prop_key: key,
                    model_file: fname,
                },
                Mesh3d(mesh_h.clone()),
                MeshMaterial3d(mat_h.clone()),
                target_tf,
                target_vis,
            ));
        }
    }
}
