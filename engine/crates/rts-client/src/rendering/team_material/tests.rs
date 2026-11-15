use super::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::render::view::visibility::VisibilityRange;
use rts_sdk::manifest::PlayerColorDef;
use rts_sdk::schema::TeamMaskMode;
use std::collections::BTreeSet;
use std::path::PathBuf;

#[test]
fn palette_strictly_caps_shared_materials_at_51_across_1200_units_and_mirror_matches_differ() {
    let mut materials = Assets::<StandardMaterial>::default();
    let mut palette = TeamDamageMaterialPalette::default();
    for unit_idx in 0..1_200u32 {
        let slot = (unit_idx % u32::from(TEAM_COLOR_PALETTE_SLOTS)) as u8;
        let tier = DamageVisualTier::from_hp_ratio(match unit_idx % 3 {
            0 => 1.0,
            1 => 0.42,
            _ => 0.15,
        });
        let _h = palette.get_or_create(&mut materials, slot, tier);
    }
    assert!(palette.len() <= MAX_SHARED_TEAM_DAMAGE_MATERIALS);
    assert_eq!(palette.len(), 51);

    let mod_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../rust-ra2")
        .canonicalize()
        .expect("canonicalize rust-ra2");
    let loaded = LoadedMod::load_from_dir(mod_dir).expect("load rust-ra2");
    let mut world = SimWorld::from_mod_map(&loaded, &loaded.entry_map);
    if let Some(p1) = world.players.get_mut(&1) {
        p1.faction = "allies".to_string();
        p1.color_index = 0;
    }
    if let Some(p2) = world.players.get_mut(&2) {
        p2.faction = "allies".to_string();
        p2.color_index = 0;
    }
    assert_ne!(
        resolve_player_team_color_slot(&world, &loaded, 1),
        resolve_player_team_color_slot(&world, &loaded, 2)
    );
}

#[test]
fn sixteen_players_resolve_collision_free_and_mod_player_colors_override_slots() {
    let mod_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../rust-ra2")
        .canonicalize()
        .expect("canonicalize rust-ra2");
    let mut loaded = LoadedMod::load_from_dir(mod_dir).expect("load rust-ra2");
    let mut world = SimWorld::from_mod_map(&loaded, &loaded.entry_map);

    let template = world.players.get(&1).expect("p1 exists").clone();
    world.players.clear();
    for pid in 1..=16u8 {
        let mut p = template.clone();
        p.player_id = pid;
        p.faction = match pid % 3 {
            1 => "allies".to_string(),
            2 => "soviets".to_string(),
            _ => "yuri".to_string(),
        };
        // Mix explicit choices (Emerald=5, Teal=14, Cobalt=1) with Auto (0)
        p.color_index = match pid {
            4 => 5,
            9 => 14,
            12 => 1,
            _ => 0,
        };
        world.players.insert(pid, p);
    }

    assert_eq!(resolve_player_team_color_slot(&world, &loaded, 4), 5);
    assert_eq!(resolve_player_team_color_slot(&world, &loaded, 9), 14);
    assert_eq!(resolve_player_team_color_slot(&world, &loaded, 12), 1);

    let mut unique_slots = BTreeSet::new();
    for pid in 1..=16u8 {
        let slot = resolve_player_team_color_slot(&world, &loaded, pid);
        assert!((1..=16).contains(&slot));
        assert!(
            unique_slots.insert(slot),
            "collision at pid={pid}, slot={slot}"
        );
    }
    assert_eq!(unique_slots.len(), 16);

    loaded.manifest.player_colors = vec![PlayerColorDef {
        id: "custom_cobalt".to_string(),
        name: "Custom Cobalt".to_string(),
        rgb: [0.11, 0.22, 0.99],
    }];
    assert_eq!(
        team_color_rgb_for_slot_in_mod(1, &loaded),
        [0.11, 0.22, 0.99]
    );
    assert_eq!(
        team_color_rgb_for_player(&world, &loaded, 12),
        [0.11, 0.22, 0.99]
    );
    assert_eq!(
        resolve_ui_color_swatch(1, None, Some(&loaded)),
        ("Custom Cobalt", [0.11, 0.22, 0.99])
    );
}

#[test]
fn gltf_lod0_and_lod1_mesh_nodes_receive_exact_visibility_ranges_and_team_material() {
    let mod_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../rust-ra2")
        .canonicalize()
        .expect("canonicalize rust-ra2");
    let loaded = LoadedMod::load_from_dir(mod_dir).expect("load rust-ra2");
    let mut world = SimWorld::from_mod_map(&loaded, &loaded.entry_map);
    let first_id = *world.entities.keys().next().expect("entity exists");

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<TeamDamageMaterialPalette>()
        .insert_resource(Assets::<Mesh>::default())
        .insert_resource(Assets::<StandardMaterial>::default())
        .insert_resource(ModResource(loaded));

    let dummy_mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(1.0, 1.0, 1.0));
    let mut lod0_ent = Entity::PLACEHOLDER;
    let mut lod1_ent = Entity::PLACEHOLDER;
    app.world_mut()
        .spawn((
            SimVisualEntity {
                sim_id: first_id,
                entity_type_id: "grizzly_tank".to_string(),
                building_footprint: None,
                world_pos: Vec3::ZERO,
                spawn_progress: 1.0,
                deconstruct_progress: None,
                death_anim_progress: None,
                is_infantry: false,
                cached_articulated_parts: None,
                has_3d_turret_node: true,
            },
            Transform::IDENTITY,
            Visibility::Inherited,
        ))
        .with_children(|parent| {
            lod0_ent = parent
                .spawn((
                    Name::new("Hull"),
                    Mesh3d(dummy_mesh.clone()),
                    Transform::IDENTITY,
                ))
                .with_children(|hull| {
                    lod1_ent = hull
                        .spawn((
                            Name::new("Hull_LOD1"),
                            Mesh3d(dummy_mesh.clone()),
                            Transform::IDENTITY,
                        ))
                        .id();
                })
                .id();
        });
    app.insert_resource(SimWorldResource(world.clone()));

    let _ = app
        .world_mut()
        .run_system_once(sync_gltf_lod_and_team_damage_materials);
    app.world_mut().flush();
    let _ = app
        .world_mut()
        .run_system_once(sync_gltf_lod_and_team_damage_materials);
    app.world_mut().flush();

    assert!(
        *app.world()
            .entity(lod0_ent)
            .get::<VisibilityRange>()
            .unwrap()
            == lod0_visibility_range()
    );
    assert!(
        *app.world()
            .entity(lod1_ent)
            .get::<VisibilityRange>()
            .unwrap()
            == lod1_visibility_range()
    );
    let m_intact = app
        .world()
        .entity(lod0_ent)
        .get::<MeshMaterial3d<StandardMaterial>>()
        .unwrap()
        .0
        .clone();

    world.entities.get_mut(&first_id).unwrap().health = 10.0;
    app.insert_resource(SimWorldResource(world));
    let _ = app
        .world_mut()
        .run_system_once(sync_gltf_lod_and_team_damage_materials);
    app.world_mut().flush();

    let m_red = app
        .world()
        .entity(lod0_ent)
        .get::<MeshMaterial3d<StandardMaterial>>()
        .unwrap()
        .0
        .clone();
    assert_ne!(m_intact, m_red);
}

#[test]
fn unit_pbr_team_material_preserves_gltf_textures_and_applies_colorblind_overrides() {
    let mut images = Assets::<Image>::default();
    let albedo_h = images.add(build_default_fow_overlay_image(4, 4));
    let normal_h = images.add(build_default_fow_overlay_image(4, 4));
    let orm_h = images.add(build_default_fow_overlay_image(4, 4));
    let emissive_h = images.add(build_default_fow_overlay_image(4, 4));
    let fow_h = images.add(build_default_fow_overlay_image(16, 16));

    let orig = StandardMaterial {
        base_color_texture: Some(albedo_h.clone()),
        normal_map_texture: Some(normal_h.clone()),
        metallic_roughness_texture: Some(orm_h.clone()),
        emissive_texture: Some(emissive_h.clone()),
        ..default()
    };
    let vis = VisualDef {
        team_mask_mode: TeamMaskMode::AlphaMask,
        metallic: Some(0.82),
        roughness: Some(0.31),
        emissive_boost: Some(1.4),
        team_mask_threshold: Some(0.35),
        ..Default::default()
    };

    let hc_enemy = resolve_accessible_team_color(
        2,
        false,
        false,
        ColorblindPaletteMode::AllyEnemyHighContrast,
    );
    let hc_self =
        resolve_accessible_team_color(1, true, true, ColorblindPaletteMode::AllyEnemyHighContrast);
    assert_ne!(hc_enemy, hc_self);

    let built = build_unit_pbr_team_material(
        &orig,
        Some(&vis),
        hc_enemy,
        false,
        DamageVisualTier::Red25,
        ShieldFresnelKind::IronCurtain,
        None,
        None,
        fow_h,
        Vec2::splat(128.0),
        1.0,
        0.75,
        3.0,
    );
    assert_eq!(built.base.base_color_texture, Some(albedo_h));
    assert_eq!(built.base.normal_map_texture, Some(normal_h));
    assert_eq!(built.base.metallic_roughness_texture, Some(orm_h));
    assert_eq!(built.base.emissive_texture, Some(emissive_h));
    assert!((built.extension.params.pbr_overrides.x - 0.82).abs() < 1e-5);
    assert!((built.extension.params.pbr_overrides.y - 0.31).abs() < 1e-5);
    assert!((built.extension.params.damage_and_fx.x - 0.88).abs() < 1e-5);
    assert!((built.extension.params.damage_and_fx.y - 0.88).abs() < 1e-5);
    assert!((built.extension.params.damage_and_fx.z - 1.0).abs() < 1e-5);
}
