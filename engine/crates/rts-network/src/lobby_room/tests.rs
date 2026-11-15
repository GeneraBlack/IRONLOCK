use super::*;
use std::path::PathBuf;

#[test]
fn staging_room_slot_lifecycle_spawn_conflict_and_map_ready_invariants() {
    let mut room = LobbyRoomState::new(
        4,
        LobbyMatchRules {
            map_id: "custom_valley".to_string(),
            map_name: "Custom Valley".to_string(),
            custom_map_hash: Some(0xDEAD_BEEF_CAFE_0001),
            starting_credits: 12_500,
            game_speed_hz: 30,
            superweapons_enabled: false,
            crates_enabled: true,
            fog_of_war: true,
            game_mode: "Short Game".to_string(),
            ..Default::default()
        },
    );
    assert_eq!(room.slots.len(), 8);
    assert_eq!(room.open_or_human_capacity(), 4);

    assert_eq!(room.assign_human(1, "HostCmdr".to_string()), Some(0));
    assert_eq!(room.assign_human(2, "PeerTwo".to_string()), Some(1));
    assert!(!room.can_launch(), "PeerTwo lacks custom map and ready");

    room.update_own_slot(2, "allies".to_string(), None, 3, 2, Some(1), true, false)
        .unwrap();
    assert!(!room.find_human_slot(2).unwrap().ready);

    assert!(room
        .update_own_slot(1, "soviet".to_string(), None, 0, 1, Some(1), true, true)
        .is_err());

    room.update_own_slot(1, "soviet".to_string(), None, 0, 1, Some(0), true, true)
        .unwrap();
    room.host_configure_slot(
        2,
        LobbySlotOccupant::AiBot {
            difficulty: "Hard".to_string(),
            personality: "Rusher".to_string(),
        },
        "random".to_string(),
        1,
        2,
        120,
    )
    .unwrap();
    room.host_configure_slot(
        3,
        LobbySlotOccupant::AiBot {
            difficulty: "Brutal".to_string(),
            personality: "Turtle".to_string(),
        },
        "random".to_string(),
        2,
        4,
        150,
    )
    .unwrap();

    room.update_own_slot(2, "allies".to_string(), None, 3, 2, Some(1), true, true)
        .unwrap();
    assert!(room.can_launch());

    let mut updated_rules = room.rules.clone();
    updated_rules.custom_map_hash = Some(0x9999_8888_7777_6666);
    room.host_update_rules(updated_rules);
    assert!(!room.can_launch());
    assert_eq!(room.humans_missing_map(), vec![2]);

    let mut resume_rules = room.rules.clone();
    resume_rules.custom_map_hash = None;
    resume_rules.resume_save_hash = Some("ab".repeat(32));
    resume_rules.resume_save_name = Some("midgame_tick60.rtssav".to_string());
    resume_rules.resume_tick = Some(60);
    resume_rules.allow_spectators = true;
    resume_rules.spectator_delay_secs = 30;
    room.host_update_rules(resume_rules);
    assert_eq!(room.humans_missing_save(), vec![2]);
    assert!(!room.can_launch());

    room.update_own_slot(2, "allies".to_string(), None, 3, 2, Some(1), true, true)
        .unwrap();
    assert!(!room.find_human_slot(2).unwrap().ready);

    assert!(room.set_human_save_ready(2, true));
    room.update_own_slot(2, "allies".to_string(), None, 3, 2, Some(1), true, true)
        .unwrap();
    assert!(room.can_launch());
}

#[test]
fn sixteen_slot_8v8_resize_and_sim_world_launch_preserves_colors_teams_and_spawns() {
    let mod_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../rust-ra2")
        .canonicalize()
        .expect("canonicalize rust-ra2");
    let loaded = rts_sdk::loader::LoadedMod::load_from_dir(mod_dir).expect("load rust-ra2");

    let mut room = LobbyRoomState::new(8, LobbyMatchRules::default());
    assert_eq!(room.slots.len(), 8);
    room.assign_human(1, "Commander1".to_string());
    room.resize_slots(16);
    assert_eq!(room.slots.len(), 16);
    assert_eq!(room.rules.target_slot_count, Some(16));

    // Configure Slot 0 (Human P1) + Slots 1..16 (15 AI bots) across Team A (1) vs Team B (2) and Team H (8) check
    room.update_own_slot(1, "allies".to_string(), None, 1, 1, Some(0), true, true)
        .expect("host slot 0 update");

    for slot_idx in 1..16u8 {
        let fac = match slot_idx % 3 {
            0 => "allies",
            1 => "soviets",
            _ => "yuri",
        };
        let team_id = if slot_idx < 8 { 1 } else { 2 };
        let color_idx = slot_idx + 1; // 2..=16
        room.host_configure_slot(
            slot_idx,
            LobbySlotOccupant::AiBot {
                difficulty: "Hard".to_string(),
                personality: "Balanced".to_string(),
            },
            fac.to_string(),
            team_id,
            color_idx,
            100,
        )
        .expect("configure AI slot");
    }
    assert!(room.can_launch());

    let world = room.build_sim_world(&loaded, 0x2026_1616_8888);
    assert_eq!(world.players.len(), 16);
    for pid in 1..=16u8 {
        let econ = world.players.get(&pid).expect("player spawned");
        assert_eq!(econ.color_index, pid);
        let expected_team = if pid <= 8 { 1 } else { 2 };
        assert_eq!(econ.team_id, expected_team);
        assert_eq!(econ.allied_players.len(), 7);
    }
    assert!(!world.are_hostile(1, 8));
    assert!(world.are_hostile(1, 9));
    assert!(!world.are_hostile(9, 16));

    // Verify Team H (`team_id == 8`) clamping and shrink safety preserving Host
    room.update_own_slot(1, "yuri".to_string(), None, 16, 8, Some(15), true, true)
        .expect("team H and spawn 15 valid");
    assert_eq!(room.find_human_slot(1).unwrap().team_id, 8);
    assert_eq!(room.find_human_slot(1).unwrap().color_index, 16);
    room.resize_slots(8);
    assert_eq!(room.slots.len(), 8);
    assert!(room.find_human_slot(1).is_some());
}
