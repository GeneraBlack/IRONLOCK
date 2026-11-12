use super::*;
use bevy::ecs::system::RunSystemOnce;
use rts_simulation::command::QueuedOrder;
use rts_simulation::world::{CoverLevel, SimWorld};
use std::path::PathBuf;

fn load_sample_mod_and_world() -> (rts_sdk::loader::LoadedMod, SimWorld) {
    let mod_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../rust-ra2")
        .canonicalize()
        .expect("canonicalize rust-ra2");
    let loaded = rts_sdk::loader::LoadedMod::load_from_dir(mod_dir).expect("load rust-ra2");
    let world = SimWorld::from_mod_map(&loaded, &loaded.entry_map);
    (loaded, world)
}

#[test]
fn resource_density_stages_and_gpu_quad_batch_builder_produce_valid_geometry() {
    assert_eq!(resource_density_stage(0, 800), 0);
    assert_eq!(resource_density_stage(100, 800), 1);
    assert_eq!(resource_density_stage(300, 800), 2);
    assert_eq!(resource_density_stage(500, 800), 3);
    assert_eq!(resource_density_stage(750, 800), 4);
    assert_eq!(ore_stage_glb_filename(false, 4), "ore_stage_4.glb");
    assert_eq!(ore_stage_glb_filename(true, 1), "gem_stage_1.glb");

    let mut batch = DynamicQuadBatchBuilder::default();
    batch.push_ground_line(
        Vec3::ZERO,
        Vec3::new(4.0, 0.0, 0.0),
        0.1,
        [1.0, 1.0, 1.0, 1.0],
    );
    batch.push_ground_ring(Vec3::ZERO, 2.0, 0.15, 12, [0.2, 0.9, 0.4, 0.8]);
    batch.push_billboard_rect(Vec3::Y, Vec3::X, Vec3::Y, 0.8, 0.1, [0.2, 1.0, 0.3, 1.0]);
    batch.push_ground_arrow(Vec3::ZERO, 0.5, 1.0, 0.05, [0.2, 1.0, 0.5, 0.9]);
    batch.push_oriented_box(
        Vec3::Y,
        Vec3::splat(0.5),
        Quat::IDENTITY,
        [0.7, 0.7, 0.7, 1.0],
    );

    let mut mesh = batch_builder::new_empty_batch_mesh();
    batch.write_into_mesh(&mut mesh);
    assert_eq!(mesh.count_vertices(), (1 + 12 + 1 + 3 + 6) * 4);
    assert!(batch.positions.is_empty());
    assert!(batch.indices.is_empty());
}

#[test]
fn scaffolding_ping_pillar_bridge_trusses_and_cover_pinned_pips_emit_3d_geometry() {
    let mut scaf_early = DynamicQuadBatchBuilder::default();
    push_building_construction_scaffolding_and_crane(
        &mut scaf_early,
        Vec3::ZERO,
        2.0,
        2.0,
        4.0,
        0.20,
        1.5,
        [0.2, 0.5, 1.0],
    );
    let mut scaf_late = DynamicQuadBatchBuilder::default();
    push_building_construction_scaffolding_and_crane(
        &mut scaf_late,
        Vec3::ZERO,
        2.0,
        2.0,
        4.0,
        0.85,
        1.5,
        [0.2, 0.5, 1.0],
    );
    assert!(scaf_early.positions.len() >= 100);
    assert!(scaf_late.positions.len() > scaf_early.positions.len());

    let mut scaf_done = DynamicQuadBatchBuilder::default();
    push_building_construction_scaffolding_and_crane(
        &mut scaf_done,
        Vec3::ZERO,
        2.0,
        2.0,
        4.0,
        1.0,
        1.5,
        [0.2, 0.5, 1.0],
    );
    assert!(scaf_done.positions.is_empty());

    let mut ping_ground = DynamicQuadBatchBuilder::default();
    let mut ping_billboard = DynamicQuadBatchBuilder::default();
    push_tactical_ping_pillar_and_rings(
        &mut ping_ground,
        &mut ping_billboard,
        Vec3::new(10.0, 0.0, 10.0),
        3.8,
        [0.15, 0.65, 1.0],
        Vec3::X,
        Vec3::Y,
    );
    assert!(ping_ground.positions.len() >= 24 * 3 * 4);
    let max_pillar_y = ping_billboard
        .positions
        .iter()
        .map(|p| p[1])
        .fold(0.0_f32, f32::max);
    assert!(max_pillar_y > 12.0);

    assert!(cover_level_shield_rgba(CoverLevel::None).is_none());
    let (light_col, light_fort) = cover_level_shield_rgba(CoverLevel::Light).unwrap();
    let (heavy_col, heavy_fort) = cover_level_shield_rgba(CoverLevel::Heavy).unwrap();
    let (gar_col, gar_fort) = cover_level_shield_rgba(CoverLevel::Garrison).unwrap();
    assert!(light_col[0] > 0.9 && light_col[1] > 0.7 && light_col[2] < 0.3 && !light_fort);
    assert!(heavy_col[1] > 0.9 && heavy_col[0] < 0.3 && !heavy_fort);
    assert!(gar_col[1] > 0.9 && gar_fort);

    let mut pip_batch = DynamicQuadBatchBuilder::default();
    pip_batch.push_billboard_shield_pip(Vec3::Y, Vec3::X, Vec3::Y, 1.0, gar_col, true);
    pip_batch.push_billboard_pinned_badge(Vec3::Y * 2.0, Vec3::X, Vec3::Y, 1.0, 0.5);
    assert!(pip_batch.positions.len() >= 9 * 4);
}

#[test]
fn static_props_spawn_flat_instanced_mesh3d_without_scene_root_and_global_shift_paths_render() {
    let (loaded, mut world_sim) = load_sample_mod_and_world();
    // Queue a Shift-order on an unselected friendly unit to verify global `[Shift]` path preview
    let friendly_uid = world_sim
        .entities
        .values()
        .find(|e| e.player_id == 1)
        .map(|e| e.id)
        .expect("friendly entity");
    if let Some(ent) = world_sim.entities.get_mut(&friendly_uid) {
        ent.command_queue.push_back(QueuedOrder::Move {
            target_pos: [64.0, 0.0, 64.0],
            facing_radians: Some(0.8),
        });
    }

    let mut app_world = World::new();
    app_world.insert_resource(Time::<()>::default());
    let mut keys = ButtonInput::<KeyCode>::default();
    keys.press(KeyCode::ShiftLeft);
    app_world.insert_resource(keys);
    app_world.insert_resource(Assets::<Mesh>::default());
    app_world.insert_resource(Assets::<StandardMaterial>::default());
    app_world.insert_resource(WorldOverlayGpuAssets::default());
    app_world.insert_resource(PrevSimSnapshot::default());
    app_world.insert_resource(CurrSimSnapshot::default());
    app_world.insert_resource(CachedFogVisibility::default());
    app_world.insert_resource(ClientInteractionState::default());
    app_world.insert_resource(CurveFormationPreviewResource {
        active: true,
        curve_points: vec![[20.0, 0.0, 20.0], [28.0, 0.0, 24.0], [36.0, 0.0, 22.0]],
        sample_targets: vec![([24.0, 0.0, 22.0], 0.4), ([32.0, 0.0, 23.0], 0.6)],
    });
    app_world.insert_resource(ModResource(loaded));
    app_world.insert_resource(SimWorldResource(world_sim));

    app_world
        .run_system_once(sync_gpu_world_overlays_and_props)
        .expect("sync_gpu_world_overlays_and_props should succeed");

    // Verify flat GPU-instanced `(Mesh3d, MeshMaterial3d)` prop entities with ZERO `SceneRoot`!
    let mut prop_q = app_world.query::<(&PropSceneVisualEntity, &Mesh3d, Option<&SceneRoot>)>();
    let prop_count = prop_q.iter(&app_world).count();
    assert!(
        prop_count > 0,
        "map props & ore clusters must spawn flat instanced entities"
    );
    for (_, _, scene_root_opt) in prop_q.iter(&app_world) {
        assert!(
            scene_root_opt.is_none(),
            "static props must NOT spawn SceneRoot hierarchies"
        );
    }

    // Verify constant-screen-pixel-width scaling across tactical -> orbital zoom
    let hw_close = order_ribbon_half_width_for_distance(20.0);
    let hw_orbital = order_ribbon_half_width_for_distance(450.0);
    assert!(hw_orbital > hw_close * 5.0);

    // Verify ground overlay mesh emitted geometry for global `[Shift]` path + curve formation preview
    let ground_h = app_world
        .resource::<WorldOverlayGpuAssets>()
        .ground_mesh
        .clone();
    let meshes = app_world.resource::<Assets<Mesh>>();
    let gm = meshes.get(&ground_h).expect("ground overlay mesh");
    assert!(gm.count_vertices() >= 80);
}
