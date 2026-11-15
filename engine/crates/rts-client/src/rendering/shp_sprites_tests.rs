use super::*;
use crate::rendering::{sync_sim_entities_to_3d, CachedFogVisibility};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

#[test]
fn visual_mode_defaults_by_catalog_presence_and_f8_toggles_live() {
    assert_eq!(
        Ra2EntityVisualMode::default_for_mod_root(Path::new("nonexistent_mod_without_sprites")),
        Ra2EntityVisualMode::Full3dMeshes
    );
    let ra2_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../rust-ra2");
    assert_eq!(
        Ra2EntityVisualMode::default_for_mod_root(&ra2_dir),
        Ra2EntityVisualMode::HybridHdSprites
    );

    let loaded = rts_sdk::loader::LoadedMod::load_from_dir(&ra2_dir).expect("load rust-ra2");
    let world = SimWorld::from_mod_map(&loaded, &loaded.entry_map);
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Shader>()
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_asset::<Scene>()
        .insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(crate::AssetRootResource(
            ra2_dir.parent().unwrap().to_path_buf(),
        ))
        .insert_resource(ModResource(loaded))
        .insert_resource(SimWorldResource(world))
        .init_resource::<CachedFogVisibility>()
        .init_resource::<ClientInteractionState>();
    register_ra2_shp_sprite_rendering(&mut app);
    app.add_systems(Update, sync_sim_entities_to_3d);

    assert_eq!(
        *app.world().resource::<Ra2EntityVisualMode>(),
        Ra2EntityVisualMode::HybridHdSprites
    );
    app.update();

    let mut quad_q = app.world_mut().query::<(&Ra2HdSpriteQuad, &Visibility)>();
    assert!(quad_q
        .iter(app.world())
        .any(|(q, v)| *v == Visibility::Visible && q.min_world_y.abs() < 1e-4));
    let mut child_q = app.world_mut().query::<(&SimGltfSceneChild, &Visibility)>();
    assert!(child_q.iter(app.world()).any(|(_, v)| *v == Visibility::Hidden));

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::F8);
    app.update();
    assert_eq!(
        *app.world().resource::<Ra2EntityVisualMode>(),
        Ra2EntityVisualMode::Full3dMeshes
    );
    assert!(quad_q.iter(app.world()).all(|(_, v)| *v == Visibility::Hidden));
    assert!(child_q.iter(app.world()).all(|(_, v)| *v == Visibility::Inherited));
}

#[test]
fn infantry_8dir_facing_ground_anchor_and_turret_barrel_hierarchy() {
    assert_eq!(infantry_facing_8dir(FRAC_PI_4, FRAC_PI_4), 4);
    assert_eq!(infantry_facing_8dir(-3.0 * FRAC_PI_4, FRAC_PI_4), 0);
    assert_eq!(infantry_facing_8dir(-FRAC_PI_4, FRAC_PI_4), 2);
    assert_eq!(infantry_facing_8dir(3.0 * FRAC_PI_4, FRAC_PI_4), 6);

    let basis = CameraSpriteBasis::from_camera_transform(None);
    let tf = compute_ground_anchored_sprite_transform(
        Vec3::ZERO,
        20,
        10,
        180,
        140,
        [110.0, 118.0],
        0.047,
        &basis,
    );
    for lx in [-0.5_f32, 0.5_f32] {
        assert!(tf.transform_point(Vec3::new(lx, 0.0, 0.0)).y.abs() < 1e-5);
    }

    let child_barrel = CachedArticulatedPart {
        entity: Entity::PLACEHOLDER,
        kind: ArticulatedPartKind::Barrel,
        rest_translation: Vec3::new(0.0, 0.45, 0.80),
        parent_is_turret: true,
    };
    let (rot, _pos) =
        compute_articulated_part_pose(&child_barrel, true, FRAC_PI_2, 0.30, 0.20, 0.0);
    assert!(rot.abs_diff_eq(Quat::from_rotation_x(-0.30), 1e-5));
    assert!((rot * Vec3::Z).x.abs() < 1e-5);

    let ra2_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../rust-ra2");
    let mut cat = Ra2ShpSpriteCatalog::default();
    assert!(cat.load_from_mod_root(&ra2_dir));
    assert!(cat.buildings.len() >= 75 && cat.infantry.len() >= 25);
    let conyard = cat.buildings.get("allied_conyard").expect("allied_conyard");
    assert!(conyard
        .make
        .contains(&select_building_sprite_frame(conyard, 0.45, None, 1.0, true, 0.0)));
    assert_eq!(
        select_building_sprite_frame(conyard, 1.0, None, 0.35, false, 0.0),
        conyard.damaged
    );
}
