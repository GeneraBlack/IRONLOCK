use super::*;
use bevy::render::mesh::Indices;
use std::path::PathBuf;

fn sample_mod_and_world() -> (rts_sdk::loader::LoadedMod, SimWorld) {
    let mod_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../rust-ra2")
        .canonicalize()
        .expect("canonicalize rust-ra2 mod path");
    let loaded = rts_sdk::loader::LoadedMod::load_from_dir(mod_dir).expect("load rust-ra2");
    let world = SimWorld::from_mod_map(&loaded, &loaded.entry_map);
    (loaded, world)
}

#[test]
fn build_continuous_terrain_mesh_produces_valid_grid() {
    let (loaded, world) = sample_mod_and_world();
    let w = world.grid.width as usize;
    let h = world.grid.height as usize;
    let mesh = build_continuous_terrain_mesh(&world);
    let expected_verts = (w + 1) * (h + 1);
    assert_eq!(mesh.count_vertices(), expected_verts);
    assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
    assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
    assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_some());
    match mesh.indices().expect("terrain indices") {
        Indices::U32(indices) => assert_eq!(indices.len(), w * h * 6),
        Indices::U16(indices) => assert_eq!(indices.len(), w * h * 6),
    }

    let splat_mesh = build_splatted_terrain_mesh(&loaded.entry_map, &world);
    assert_eq!(splat_mesh.count_vertices(), expected_verts);
    assert!(splat_mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_some());
}

#[test]
fn client_rendering_av_plugin_initializes_and_ticks_with_zero_ecs_conflicts() {
    let (loaded, world) = sample_mod_and_world();
    let asset_root = loaded
        .root_dir
        .parent()
        .unwrap_or(loaded.root_dir.as_path())
        .to_path_buf();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Shader>()
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_asset::<StandardMaterial>()
        .init_asset::<TerrainSplatMaterial>()
        .init_asset::<WaterWaveMaterial>()
        .init_asset::<UnitPbrTeamMaterial>()
        .init_asset::<crate::vfx::GpuParticleShaderMaterial>()
        .init_asset::<bevy::gltf::Gltf>()
        .init_asset::<bevy::gltf::GltfNode>()
        .init_asset::<bevy::gltf::GltfMesh>()
        .init_asset::<Scene>()
        .init_resource::<CachedFogVisibility>()
        .init_resource::<crate::input::ClientInteractionState>()
        .init_resource::<crate::audio::SoundSettingsResource>()
        .insert_resource(crate::AssetRootResource(asset_root))
        .insert_resource(ModResource(loaded))
        .insert_resource(SimWorldResource(world))
        .add_plugins(ClientRenderingAvPlugin)
        .add_systems(
            Startup,
            (setup_3d_map_environment, crate::vfx::setup_vfx_assets),
        )
        .add_systems(
            Update,
            (
                refresh_cached_fog_visibility,
                sync_sim_entities_to_3d,
                crate::vfx::spawn_combat_and_destruction_vfx,
                crate::vfx::update_and_draw_vfx,
                crate::audio::step_procedural_combat_audio,
            ),
        );

    app.update();
    app.update();

    assert!(app.world().contains_resource::<VideoPlaybackResource>());
    assert!(app.world().contains_resource::<StudioDebugOverlayConfig>());
    assert!(app.world().contains_resource::<MapTerrainDirty>());
    assert!(app.world().contains_resource::<SocketRegistry>());
    let mut fow_plane_q = app.world_mut().query::<&FogOfWarPlaneEntity>();
    assert_eq!(
        fow_plane_q.iter(app.world()).count(),
        0,
        "Floating +0.52m FogOfWarPlaneEntity mesh must not be spawned"
    );
}
