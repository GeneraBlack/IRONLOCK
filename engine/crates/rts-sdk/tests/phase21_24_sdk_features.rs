use rts_sdk::archive::ModArchive;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::*;
use rts_sdk::validator::{validate_binary_asset_header, validate_mod};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root should exist")
}

fn test_scratch_dir(name: &str) -> PathBuf {
    let dir = workspace_root()
        .join("engine")
        .join("target_sdk3")
        .join("test_tmp")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("should create test_tmp dir on D:");
    dir
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let ty = entry.file_type().unwrap();
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn valid_minimal_glb() -> Vec<u8> {
    let json = b"{\"asset\":{\"version\":\"2.0\"}} ";
    let total_len = (20 + json.len()) as u32;
    let mut out = Vec::with_capacity(total_len as usize);
    out.extend_from_slice(&0x4654_6C67u32.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&total_len.to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
    out.extend_from_slice(json);
    out
}

fn valid_minimal_png(width: u32, height: u32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    out.extend_from_slice(&13u32.to_be_bytes());
    out.extend_from_slice(b"IHDR");
    out.extend_from_slice(&width.to_be_bytes());
    out.extend_from_slice(&height.to_be_bytes());
    out.extend_from_slice(&[8, 6, 0, 0, 0]);
    out.extend_from_slice(&0u32.to_be_bytes());
    out
}

fn valid_minimal_wav() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&36u32.to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&[0u8; 20]);
    out
}

fn valid_minimal_mp4() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&20u32.to_be_bytes());
    out.extend_from_slice(b"ftypisom");
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(b"isom");
    out
}

#[test]
fn deep_binary_header_validator_catches_corrupt_glb_png_audio_video_and_splatmap_in_vfs_and_disk() {
    // 1. Direct header checks across valid & corrupt payloads
    let good_glb = valid_minimal_glb();
    assert!(validate_binary_asset_header("assets/models/tank.glb", &good_glb).is_ok());
    assert_eq!(
        validate_binary_asset_header("assets/models/short.glb", b"glTF")
            .unwrap_err()
            .0,
        "CORRUPT_GLB_ASSET"
    );
    let mut bad_magic_glb = good_glb.clone();
    bad_magic_glb[0..4].copy_from_slice(b"NOPE");
    assert_eq!(
        validate_binary_asset_header("assets/models/bad_magic.glb", &bad_magic_glb)
            .unwrap_err()
            .0,
        "CORRUPT_GLB_ASSET"
    );
    let mut bad_ver_glb = good_glb.clone();
    bad_ver_glb[4..8].copy_from_slice(&1u32.to_le_bytes());
    assert_eq!(
        validate_binary_asset_header("assets/models/v1.glb", &bad_ver_glb)
            .unwrap_err()
            .0,
        "CORRUPT_GLB_ASSET"
    );
    let mut bad_len_glb = good_glb.clone();
    bad_len_glb.push(0);
    assert_eq!(
        validate_binary_asset_header("assets/models/truncated.glb", &bad_len_glb)
            .unwrap_err()
            .0,
        "CORRUPT_GLB_ASSET"
    );
    let mut non_json_glb = good_glb.clone();
    non_json_glb[16..20].copy_from_slice(&0x004E_4942u32.to_le_bytes());
    assert_eq!(
        validate_binary_asset_header("assets/models/bin_first.glb", &non_json_glb)
            .unwrap_err()
            .0,
        "CORRUPT_GLB_ASSET"
    );

    assert!(validate_binary_asset_header("assets/ui/icon.png", &valid_minimal_png(96, 72)).is_ok());
    assert_eq!(
        validate_binary_asset_header("assets/ui/zero.png", &valid_minimal_png(0, 72))
            .unwrap_err()
            .0,
        "CORRUPT_PNG_ASSET"
    );
    assert_eq!(
        validate_binary_asset_header("assets/ui/bad.png", b"NOT_A_PNG_AT_ALL_0123456789")
            .unwrap_err()
            .0,
        "CORRUPT_PNG_ASSET"
    );

    assert!(validate_binary_asset_header("assets/audio/music.ogg", b"OggS\x00\x02").is_ok());
    assert!(validate_binary_asset_header("assets/audio/lossless.flac", b"fLaC\x00\x00").is_ok());
    assert!(validate_binary_asset_header("assets/audio/sfx.wav", &valid_minimal_wav()).is_ok());
    assert_eq!(
        validate_binary_asset_header("assets/audio/corrupt.ogg", b"ID3\x04")
            .unwrap_err()
            .0,
        "CORRUPT_AUDIO_ASSET"
    );
    assert_eq!(
        validate_binary_asset_header("assets/audio/corrupt.flac", b"RIFF")
            .unwrap_err()
            .0,
        "CORRUPT_AUDIO_ASSET"
    );
    assert_eq!(
        validate_binary_asset_header("assets/audio/corrupt.wav", b"RIFF\x10\x00\x00\x00NOPE")
            .unwrap_err()
            .0,
        "CORRUPT_AUDIO_ASSET"
    );

    assert!(validate_binary_asset_header("assets/videos/pip.mp4", &valid_minimal_mp4()).is_ok());
    assert_eq!(
        validate_binary_asset_header("assets/videos/corrupt.mp4", b"\x00\x00\x00\x14moov0000")
            .unwrap_err()
            .0,
        "CORRUPT_VIDEO_ASSET"
    );

    // 2. End-to-end validate_mod on disk AND packed inside an in-memory .rtsmod VFS archive
    let tmp = test_scratch_dir("deep_binary_vfs");
    let mod_dir = tmp.join("mod_copy");
    copy_dir_recursive(
        &workspace_root().join("mods/sdk-showcase-workers-squads"),
        &mod_dir,
    );

    // Corrupt a referenced .glb, inject a 0x0 .png cameo, corrupt .ogg music, corrupt .mp4 radar video, and bad splatmap stride
    fs::write(
        mod_dir.join("assets/models/combat_dozer.glb"),
        b"NOT_GLTF_2",
    )
    .unwrap();
    fs::create_dir_all(mod_dir.join("assets/ui/cameos")).unwrap();
    fs::write(
        mod_dir.join("assets/ui/cameos/bad_dozer.png"),
        valid_minimal_png(0, 72),
    )
    .unwrap();
    fs::create_dir_all(mod_dir.join("assets/audio")).unwrap();
    fs::write(mod_dir.join("assets/audio/bad_theme.ogg"), b"BAD_OGG").unwrap();
    fs::create_dir_all(mod_dir.join("assets/videos")).unwrap();
    fs::write(mod_dir.join("assets/videos/bad_pip.mp4"), b"0000BAD_MP4!").unwrap();

    let mut loaded = LoadedMod::load(&mod_dir).expect("mod should load");
    loaded.units.get_mut("combat_dozer").unwrap().visual.icon =
        Some("assets/ui/cameos/bad_dozer.png".to_string());
    loaded.save_unit_catalog().unwrap();

    let mut audio = loaded.audio.clone();
    audio.music_tracks.push(MusicTrackDef {
        id: "broken_track".to_string(),
        display_name: "Broken".to_string(),
        path: "assets/audio/bad_theme.ogg".to_string(),
        factions: vec![],
        mood: MusicMood::Combat,
        faction_tags: vec!["pioneers".to_string()],
        duration_hint_secs: Some(120.0),
    });
    loaded.save_audio_catalog(audio).unwrap();

    // Pack into .rtsmod VFS and verify in-memory binary header rejection!
    let archive_path = tmp.join("corrupt_test.rtsmod");
    ModArchive::pack_dir_to_file(&mod_dir, &archive_path).unwrap();
    let arch = Arc::new(ModArchive::open(&archive_path).unwrap());
    let mut vfs_loaded = LoadedMod::load_from_archive(arch).unwrap();

    // Also attach PlayRadarVideo + truncated splatmap on entry_map
    let map_id = vfs_loaded.entry_map.id.clone();
    vfs_loaded.entry_map.splatmap = Some(TerrainSplatDef {
        layers: vec!["grass".to_string(), "dirt_ore".to_string()],
        weights_rgba: vec![[255, 0, 0, 0]; 10], // Wrong vertex stride!
    });
    vfs_loaded.entry_map.trigger_zones.push(TriggerZoneDef {
        id: "bad_vid_zone".to_string(),
        min: [1, 1],
        max: [2, 2],
        once: true,
        script_hook: String::new(),
        condition: Some(TriggerCondition::TimerElapsed { secs: 5.0 }),
        actions: vec![TriggerAction::PlayRadarVideo {
            video_path: "assets/videos/bad_pip.mp4".to_string(),
        }],
    });
    vfs_loaded.maps.insert(map_id, vfs_loaded.entry_map.clone());

    let report = validate_mod(&vfs_loaded, true);
    assert!(!report.is_valid());
    for expected in [
        "CORRUPT_GLB_ASSET",
        "CORRUPT_PNG_ASSET",
        "CORRUPT_AUDIO_ASSET",
        "CORRUPT_VIDEO_ASSET",
        "INVALID_SPLATMAP",
    ] {
        assert!(
            report.issues.iter().any(|i| i.code == expected),
            "Expected {expected} in VFS validation report, got: {:#?}",
            report.issues
        );
    }

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn studio_catalog_crud_prefabs_commander_trees_and_reference_checker_roundtrip_100_percent() {
    let tmp = test_scratch_dir("studio_crud_roundtrip");
    let mod_dir = tmp.join("showcase_studio");
    copy_dir_recursive(
        &workspace_root().join("mods/sdk-showcase-workers-squads"),
        &mod_dir,
    );

    // Create valid placeholder assets for newly authored Studio items
    fs::create_dir_all(mod_dir.join("assets/models")).unwrap();
    fs::create_dir_all(mod_dir.join("assets/ui/cameos")).unwrap();
    fs::create_dir_all(mod_dir.join("assets/audio")).unwrap();
    fs::create_dir_all(mod_dir.join("assets/videos")).unwrap();
    fs::write(
        mod_dir.join("assets/models/studio_mech.glb"),
        valid_minimal_glb(),
    )
    .unwrap();
    fs::write(
        mod_dir.join("assets/models/bridge_hut.glb"),
        valid_minimal_glb(),
    )
    .unwrap();
    fs::write(
        mod_dir.join("assets/ui/cameos/studio_icon.png"),
        valid_minimal_png(96, 72),
    )
    .unwrap();
    fs::write(
        mod_dir.join("assets/audio/peace_loop.ogg"),
        b"OggS\x00\x02\x00\x00",
    )
    .unwrap();
    fs::write(
        mod_dir.join("assets/videos/radar_brief.mp4"),
        valid_minimal_mp4(),
    )
    .unwrap();

    let mut loaded = LoadedMod::load(&mod_dir).expect("showcase copy should load");

    // 1. Resource CRUD
    loaded
        .upsert_resource_type_def(ResourceTypeDef {
            id: "plasma".to_string(),
            display_name: "Refined Plasma".to_string(),
            color_rgb: [0.3, 0.8, 1.0],
            starting_amount: 400,
            is_capacity_pool: false,
            icon: Some("assets/ui/cameos/studio_icon.png".to_string()),
        })
        .unwrap();

    // 2. Faction CRUD
    loaded
        .upsert_faction_def(FactionDef {
            id: "directorate".to_string(),
            name: "Orbital Directorate".to_string(),
            color: [0.2, 0.8, 0.9],
            starting_credits: 6000,
            description: "High-tech command faction".to_string(),
            ..Default::default()
        })
        .unwrap();

    // 3. Weapon CRUD (including Squad 2.0 slot upgrade weapon)
    loaded
        .upsert_weapon_def(WeaponDef {
            id: "plasma_lmg".to_string(),
            name: "Plasma LMG".to_string(),
            damage: 22.0,
            range: 9.5,
            cooldown_secs: 0.6,
            projectile_speed: 38.0,
            ..Default::default()
        })
        .unwrap();

    // 4. Building CRUD (BridgeRepairHut + SupportAuraSpec radar/periodic reveal + SupportPower)
    loaded
        .upsert_building_def(BuildingDef {
            id: "tech_comm_center".to_string(),
            name: "Tech Communications Center".to_string(),
            faction: "directorate".to_string(),
            role: BuildingRole::BridgeRepairHut,
            cost: 800,
            build_time_secs: 6.0,
            health: 1500.0,
            armor: ArmorType::Structure,
            footprint: [2, 2],
            vision_range: 18.0,
            support_aura: SupportAuraSpec {
                grants_radar: true,
                periodic_reveal_interval_secs: Some(30),
                periodic_reveal_radius: 24.0,
                ..Default::default()
            },
            support_powers: vec![SupportPowerDef {
                id: "orbital_scan_sweep".to_string(),
                name: "Orbital Scan".to_string(),
                kind: SupportPowerKind::default(),
                cooldown_secs: 60.0,
                radius: 16.0,
                ..Default::default()
            }],
            addon_sockets: vec!["Addon_RadarDish".to_string()],
            visual: VisualDef {
                model: "assets/models/bridge_hut.glb".to_string(),
                icon: Some("assets/ui/cameos/studio_icon.png".to_string()),
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();

    // 5. Unit CRUD (Squads 2.0 cover multipliers, pin threshold, auto_reinforce, weapon upgrade slots)
    loaded
        .upsert_unit_def(UnitDef {
            id: "directorate_shock_squad".to_string(),
            name: "Shock Squad".to_string(),
            faction: "directorate".to_string(),
            role: UnitRole::Infantry,
            cost: 350,
            build_time_secs: 4.0,
            health: 360.0,
            armor: ArmorType::Light,
            speed: 4.2,
            vision_range: 14.0,
            primary_weapon: Some("plasma_lmg".to_string()),
            prerequisites: vec!["tech_comm_center".to_string()],
            resource_costs: BTreeMap::from([("plasma".to_string(), 50)]),
            squad_trait: Some(SquadTraitSpec {
                member_count: 5,
                member_spacing: 0.9,
                reinforce_cost_ratio: 0.5,
                directional_cover_multiplier: 0.65,
                max_suppression: 100.0,
                light_cover_damage_mult: Some(0.75),
                heavy_cover_damage_mult: Some(0.45),
                pin_suppression_threshold: Some(0.85),
                auto_reinforce_default: Some(false),
                weapon_upgrade_slots: 2,
                available_weapon_upgrades: vec!["plasma_lmg".to_string()],
            }),
            visual: VisualDef {
                model: "assets/models/studio_mech.glb".to_string(),
                icon: Some("assets/ui/cameos/studio_icon.png".to_string()),
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();

    // 6. Upgrade + CommanderPowerTree CRUD (co-existing in upgrades/upgrades.toml)
    loaded
        .upsert_upgrade_def(UpgradeDef {
            id: "plasma_capacitors".to_string(),
            name: "Plasma Capacitors".to_string(),
            faction: "directorate".to_string(),
            cost: 600,
            research_time_secs: 12.0,
            prerequisites: vec!["tech_comm_center".to_string()],
            range_bonus: 0.15,
            ..Default::default()
        })
        .unwrap();

    loaded
        .upsert_commander_tree_def(CommanderPowerTreeDef {
            id: "directorate_command_tree".to_string(),
            faction: "directorate".to_string(),
            display_name: "Orbital Doctrine".to_string(),
            nodes: vec![
                CommanderPowerNodeDef {
                    id: "node_scan_1".to_string(),
                    display_name: "Recon Uplink".to_string(),
                    icon: Some("assets/ui/cameos/studio_icon.png".to_string()),
                    required_rank: 1,
                    cost_points: 1,
                    prerequisites: vec![],
                    grants_support_power: Some("orbital_scan_sweep".to_string()),
                    grants_upgrade: None,
                    unlocks_def_id: None,
                    description: "Unlocks Orbital Scan".to_string(),
                },
                CommanderPowerNodeDef {
                    id: "node_overcharge_2".to_string(),
                    display_name: "Plasma Overcharge".to_string(),
                    icon: Some("assets/ui/cameos/studio_icon.png".to_string()),
                    required_rank: 2,
                    cost_points: 1,
                    prerequisites: vec!["node_scan_1".to_string()],
                    grants_support_power: None,
                    grants_upgrade: Some("plasma_capacitors".to_string()),
                    unlocks_def_id: Some("directorate_shock_squad".to_string()),
                    description: "Grants Plasma Capacitors".to_string(),
                },
            ],
        })
        .unwrap();

    // 7. AI Profile 2.0 CRUD
    loaded
        .upsert_ai_profile_def(AiProfileDef {
            id: "directorate_admiral".to_string(),
            faction: "directorate".to_string(),
            personality_id: Some("tech".to_string()),
            build_order: vec!["tech_comm_center".to_string()],
            unit_weights: BTreeMap::from([("directorate_shock_squad".to_string(), 15)]),
            unit_role_quotas: BTreeMap::from([("infantry".to_string(), 0.6)]),
            target_priority_weights: BTreeMap::from([("harvester".to_string(), 2.0)]),
            expand_at_credits: Some(3500),
            superweapon_doctrine: Some("cluster_vehicles".to_string()),
            amphibious_assault_enabled: Some(true),
            preferred_superweapons: vec!["orbital_scan_sweep".to_string()],
            ..Default::default()
        })
        .unwrap();

    // 8. Campaign, Map Objectives/Cutscenes/Triggers, Audio, Locale & Prefab CRUD
    let entry_map_id = loaded.entry_map.id.clone();
    let mut updated_map = loaded.entry_map.clone();
    updated_map.initial_weather = Some("IonStorm".to_string());
    updated_map.playable_bounds = Some(([2, 2], [40, 40]));
    updated_map.objectives = vec![MissionObjectiveDef {
        id: "obj_hold_outpost".to_string(),
        tier: ObjectiveTier::Primary,
        title: "Hold the Outpost".to_string(),
        description: Some("Defend until reinforcements arrive.".to_string()),
        target_count: 1,
        marker_tile: Some([12, 12]),
    }];
    updated_map.cutscenes = vec![CameraCutsceneDef {
        id: "intro_flyover".to_string(),
        skippable: true,
        letterbox: true,
        keyframes: vec![
            CameraKeyframe {
                time_secs: 0.0,
                focus: [16.0, 0.0, 16.0],
                distance: 36.0,
                pitch_deg: 45.0,
                yaw_deg: 0.0,
            },
            CameraKeyframe {
                time_secs: 4.0,
                focus: [24.0, 0.0, 24.0],
                distance: 28.0,
                pitch_deg: 40.0,
                yaw_deg: 35.0,
            },
        ],
    }];
    updated_map.trigger_zones.push(TriggerZoneDef {
        id: "zone_intro_director".to_string(),
        min: [4, 4],
        max: [8, 8],
        once: true,
        script_hook: String::new(),
        condition: Some(TriggerCondition::PeriodicTimer {
            start_secs: 1,
            interval_secs: 30,
            max_repeats: Some(3),
        }),
        actions: vec![
            TriggerAction::PlayCutscene {
                cutscene_id: "intro_flyover".to_string(),
            },
            TriggerAction::StartCountdown {
                timer_id: "hold_timer".to_string(),
                label: "Reinforcements".to_string(),
                duration_secs: 120,
            },
            TriggerAction::EvaTransmission {
                speaker: "EVA".to_string(),
                text: "Incoming transmission.".to_string(),
                portrait: Some("assets/ui/cameos/studio_icon.png".to_string()),
                audio_cue: Some("assets/audio/peace_loop.ogg".to_string()),
                radar_video: Some("assets/videos/radar_brief.mp4".to_string()),
            },
            TriggerAction::SetObjectiveStatus {
                objective_id: "obj_hold_outpost".to_string(),
                status: ObjectiveStatus::Completed,
            },
            TriggerAction::ExpandPlayableBounds {
                min_tile: [0, 0],
                max_tile: [44, 44],
            },
        ],
    });
    let entry_map_path = mod_dir.join(&loaded.manifest.entry_map);
    updated_map.save_to_file(&entry_map_path).unwrap();

    loaded
        .upsert_campaign_def(CampaignDef {
            id: "directorate_campaign".to_string(),
            display_name: "Orbital Dawn".to_string(),
            faction: "directorate".to_string(),
            description: "Directorate Story Campaign".to_string(),
            missions: vec![MissionDef {
                id: "dir_m01".to_string(),
                display_name: "First Contact".to_string(),
                map_id: entry_map_id.clone(),
                briefing: "Secure the valley.".to_string(),
                next_mission_id: None,
                carry_over_credits_ratio: 0.5,
                carry_over_max_veteran_units: 4,
                loading_screen: Some("assets/ui/cameos/studio_icon.png".to_string()),
                intro_video: None,
                victory_video: None,
                radar_video: Some("assets/videos/radar_brief.mp4".to_string()),
                ..Default::default()
            }],
        })
        .unwrap();

    let mut new_audio = loaded.audio.clone();
    new_audio.music_tracks.push(MusicTrackDef {
        id: "orbital_peace".to_string(),
        display_name: "Orbital Calm".to_string(),
        path: "assets/audio/peace_loop.ogg".to_string(),
        factions: vec![],
        mood: MusicMood::Peace,
        faction_tags: vec!["directorate".to_string()],
        duration_hint_secs: Some(95.0),
    });
    loaded.save_audio_catalog(new_audio).unwrap();

    let mut de_locale = loaded.locales.get("de").cloned().unwrap_or(LocaleCatalog {
        language: "de".to_string(),
        strings: BTreeMap::new(),
    });
    de_locale.strings.insert(
        "unit.directorate_shock_squad".to_string(),
        "Schocktrupp".to_string(),
    );
    loaded.save_locale_catalog(de_locale).unwrap();

    let prefab = MapPrefabDef {
        id: "checkpoint_alpha".to_string(),
        display_name: "Checkpoint Alpha".to_string(),
        category: "Military".to_string(),
        footprint_size: [4, 4],
        height_deltas: vec![0.0; 25],
        splat_samples: vec![[0, 0, 255, 0]; 25],
        entities: vec![PreplacedEntityDef {
            player_id: 0,
            building_id: Some("tech_comm_center".to_string()),
            unit_id: None,
            tile_pos: [1, 1],
            tag: Some("vip_comm_hub".to_string()),
        }],
        props: vec![],
    };
    loaded.save_prefab_def(&prefab).unwrap();

    // Reload completely cold from disk via LoadedMod::load(&mod_dir)
    let mut reloaded =
        LoadedMod::load(&mod_dir).expect("reloaded mod after Studio CRUD must parse");
    assert!(reloaded.resources.contains_key("plasma"));
    assert!(reloaded.factions.contains_key("directorate"));
    assert!(reloaded.weapons.contains_key("plasma_lmg"));
    assert_eq!(
        reloaded.buildings["tech_comm_center"].role,
        BuildingRole::BridgeRepairHut
    );
    assert!(
        reloaded.buildings["tech_comm_center"]
            .support_aura
            .grants_radar
    );
    assert_eq!(
        reloaded.units["directorate_shock_squad"]
            .squad_trait
            .as_ref()
            .unwrap()
            .weapon_upgrade_slots,
        2
    );
    assert!(reloaded.upgrades.contains_key("plasma_capacitors"));
    assert!(reloaded
        .commander_trees
        .contains_key("directorate_command_tree"));
    assert_eq!(
        reloaded.ai_profiles["directorate_admiral"].effective_personality_id(),
        "tech"
    );
    assert!(reloaded.campaigns.contains_key("directorate_campaign"));
    assert_eq!(
        reloaded.t("de", "unit.directorate_shock_squad"),
        "Schocktrupp"
    );
    assert_eq!(reloaded.entry_map.objectives.len(), 1);
    assert_eq!(reloaded.entry_map.cutscenes.len(), 1);

    let prefabs = reloaded.load_prefab_defs().unwrap();
    assert_eq!(prefabs.get("checkpoint_alpha"), Some(&prefab));

    // Validate reloaded mod -> 0 errors and 0 warnings!
    let report = validate_mod(&reloaded, true);
    assert!(
        report.issues.is_empty(),
        "Studio CRUD roundtrip mod must validate with 0 errors and 0 warnings, got: {:#?}",
        report.issues
    );

    // Reference checker must find all cross-references to tech_comm_center & directorate
    let comm_refs = reloaded.find_id_references("tech_comm_center");
    assert!(comm_refs
        .iter()
        .any(|r| r.contains("unit:directorate_shock_squad.prerequisites")));
    assert!(comm_refs
        .iter()
        .any(|r| r.contains("upgrade:plasma_capacitors.prerequisites")));
    assert!(comm_refs
        .iter()
        .any(|r| r.contains("ai_profile:directorate_admiral.build_order")));
    assert!(comm_refs
        .iter()
        .any(|r| r.contains("prefab:checkpoint_alpha.entities")));

    let fac_refs = reloaded.find_id_references("directorate");
    assert!(fac_refs
        .iter()
        .any(|r| r.contains("commander_tree:directorate_command_tree.faction")));
    assert!(fac_refs
        .iter()
        .any(|r| r.contains("campaign:directorate_campaign.faction")));
    assert!(fac_refs
        .iter()
        .any(|r| r.contains("audio.music_tracks[orbital_peace].factions")));

    // Delete CRUD roundtrip
    reloaded.delete_prefab_def("checkpoint_alpha").unwrap();
    reloaded
        .delete_campaign_def("directorate_campaign")
        .unwrap();
    reloaded
        .delete_commander_tree_def("directorate_command_tree")
        .unwrap();
    reloaded
        .delete_ai_profile_def("directorate_admiral")
        .unwrap();
    reloaded.delete_unit_def("directorate_shock_squad").unwrap();
    reloaded.delete_upgrade_def("plasma_capacitors").unwrap();
    reloaded.delete_building_def("tech_comm_center").unwrap();
    reloaded.delete_weapon_def("plasma_lmg").unwrap();
    reloaded.delete_resource_type_def("plasma").unwrap();

    let after_del = LoadedMod::load(&mod_dir).unwrap();
    assert!(!after_del.units.contains_key("directorate_shock_squad"));
    assert!(!after_del.buildings.contains_key("tech_comm_center"));
    assert!(!after_del
        .commander_trees
        .contains_key("directorate_command_tree"));
    assert!(!after_del.campaigns.contains_key("directorate_campaign"));
    assert!(after_del.load_prefab_defs().unwrap().is_empty());

    let _ = fs::remove_dir_all(&tmp);
}
