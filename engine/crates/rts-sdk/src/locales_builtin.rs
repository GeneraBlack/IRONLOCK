use std::collections::BTreeMap;

/// Built-in engine UI localization pairs `(key, en_text, de_text)`.
/// Every entry has 100% EN/DE parity and serves as the zero-allocation engine fallback table.
pub const BUILTIN_UI_STRINGS: &[(&str, &str, &str)] = &[
    // 1. Main Menu
    (
        "ui.main_menu.title",
        "IRONLOCK COMMAND",
        "IRONLOCK KOMMANDO",
    ),
    ("ui.main_menu.campaign", "CAMPAIGN", "KAMPAGNE"),
    ("ui.main_menu.skirmish", "SKIRMISH", "GEFECHT"),
    (
        "ui.main_menu.multiplayer",
        "MULTIPLAYER (LAN / ONLINE)",
        "MEHRSPIELER (LAN / ONLINE)",
    ),
    (
        "ui.main_menu.load_game",
        "LOAD SAVED GAME",
        "SPIELSTAND LADEN",
    ),
    (
        "ui.main_menu.replays",
        "THEATER & REPLAYS",
        "ARCHIV & REPLAYS",
    ),
    (
        "ui.main_menu.encyclopedia",
        "UNIT ENCYCLOPEDIA",
        "EINHEITEN-LEXIKON",
    ),
    (
        "ui.main_menu.studio",
        "CREATOR WORKBENCH [F12]",
        "CREATOR WORKBENCH [F12]",
    ),
    (
        "ui.main_menu.settings",
        "SETTINGS & CONTROLS",
        "OPTIONEN & STEUERUNG",
    ),
    ("ui.main_menu.quit", "EXIT GAME", "SPIEL BEENDEN"),
    ("ui.common.back", "BACK", "ZURÜCK"),
    ("ui.common.apply", "APPLY", "ÜBERNEHMEN"),
    ("ui.common.cancel", "CANCEL", "ABBRECHEN"),
    ("ui.common.confirm", "CONFIRM", "BESTÄTIGEN"),
    ("ui.common.close", "CLOSE [ESC]", "SCHLIESSEN [ESC]"),
    ("ui.common.delete", "DELETE", "LÖSCHEN"),
    ("ui.common.yes", "YES", "JA"),
    ("ui.common.no", "NO", "NEIN"),
    // 2. Campaign & Briefing
    (
        "ui.campaign.title",
        "CAMPAIGN OPERATIONS",
        "KAMPAGNEN-OPERATIONEN",
    ),
    (
        "ui.campaign.select_campaign",
        "SELECT CAMPAIGN",
        "KAMPAGNE WÄHLEN",
    ),
    (
        "ui.campaign.select_mission",
        "SELECT MISSION",
        "MISSION WÄHLEN",
    ),
    (
        "ui.campaign.launch_mission",
        "LAUNCH OPERATION",
        "MISSION STARTEN",
    ),
    ("ui.campaign.completed", "COMPLETED", "ABGESCHLOSSEN"),
    ("ui.campaign.locked", "LOCKED", "GESPERRT"),
    (
        "ui.campaign.carry_over",
        "VETERAN & CREDIT CARRYOVER",
        "VETERANEN- & CREDIT-ÜBERNAHME",
    ),
    (
        "ui.briefing.title",
        "TACTICAL BRIEFING",
        "EINSATZBESPRECHUNG",
    ),
    (
        "ui.briefing.skip_video",
        "SKIP TRANSMISSION [SPACE]",
        "ÜBERTRAGUNG ÜBERSPRINGEN [LEERTASTE]",
    ),
    (
        "ui.briefing.start_battle",
        "DEPLOY TO BATTLEFIELD",
        "AUF DAS SCHLACHTFELD AUSRÜCKEN",
    ),
    // 3. Skirmish
    (
        "ui.skirmish.title",
        "SKIRMISH ENGAGEMENT",
        "GEFECHTS-KONFIGURATION",
    ),
    ("ui.skirmish.map", "BATTLEFIELD MAP", "SCHLACHTFELD-KARTE"),
    (
        "ui.skirmish.generate_map",
        "GENERATE PROCEDURAL MAP",
        "PROZEDURALE KARTE GENERIEREN",
    ),
    ("ui.skirmish.faction", "PLAYER FACTION", "SPIELER-FRAKTION"),
    (
        "ui.skirmish.sub_faction",
        "DOCTRINE / SUB-FACTION",
        "DOKTRIN / UNTERFRAKTION",
    ),
    (
        "ui.skirmish.ai_difficulty",
        "AI COMMANDER DIFFICULTY",
        "KI-SCHWIERIGKEITSGRAD",
    ),
    ("ui.skirmish.ai_easy", "EASY", "LEICHT"),
    ("ui.skirmish.ai_medium", "MEDIUM", "MITTEL"),
    ("ui.skirmish.ai_hard", "HARD", "SCHWER"),
    ("ui.skirmish.ai_brutal", "BRUTAL", "BRUTAL"),
    (
        "ui.skirmish.starting_credits",
        "STARTING CREDITS",
        "START-CREDITS",
    ),
    (
        "ui.skirmish.game_speed",
        "SIMULATION SPEED",
        "SPIELGESCHWINDIGKEIT",
    ),
    ("ui.skirmish.superweapons", "SUPERWEAPONS", "SUPERWAFFEN"),
    ("ui.skirmish.crates", "SUPPLY CRATES", "VERSORGUNGSKISTEN"),
    (
        "ui.skirmish.fog_of_war",
        "FOG OF WAR",
        "KRIEGSNEBEL (FOG OF WAR)",
    ),
    (
        "ui.skirmish.short_game",
        "SHORT GAME (BUILDINGS ONLY)",
        "KURZSPIEL (NUR GEBÄUDE)",
    ),
    (
        "ui.skirmish.start_match",
        "START SKIRMISH",
        "GEFECHT STARTEN",
    ),
    // 4. Multiplayer & Ranked Automatch
    (
        "ui.multiplayer.title",
        "MULTIPLAYER COMMAND NETWORK",
        "MEHRSPIELER-KOMMANDONETZWERK",
    ),
    (
        "ui.multiplayer.host_lobby",
        "HOST STAGING LOBBY",
        "NEUE LOBBY ERSTELLEN",
    ),
    (
        "ui.multiplayer.join_selected",
        "JOIN SELECTED LOBBY",
        "GEWÄHLTER LOBBY BEITRETEN",
    ),
    (
        "ui.multiplayer.direct_connect",
        "DIRECT IP / RELAY CONNECT",
        "DIREKTVERBINDUNG (IP / RELAY)",
    ),
    (
        "ui.multiplayer.spectate",
        "SPECTATE LIVE MATCH",
        "LIVE ZUSCHAUEN (SPECTATOR)",
    ),
    (
        "ui.multiplayer.refresh",
        "REFRESH LOBBIES",
        "LOBBY-LISTE AKTUALISIEREN",
    ),
    (
        "ui.multiplayer.automatch_1v1",
        "1V1 RANKED AUTOMATCH",
        "1V1 RANGLISTEN-AUTOMATCH",
    ),
    (
        "ui.multiplayer.automatch_2v2",
        "2V2 RANKED AUTOMATCH",
        "2V2 RANGLISTEN-AUTOMATCH",
    ),
    (
        "ui.multiplayer.cancel_queue",
        "CANCEL MATCHMAKING",
        "SUCHE ABBRECHEN",
    ),
    (
        "ui.multiplayer.searching",
        "SEARCHING FOR OPPONENT...",
        "SUCHE NACH GEGNER...",
    ),
    (
        "ui.multiplayer.leaderboard",
        "GLOBAL ELO LEADERBOARD",
        "GLOBALE ELO-RANGLISTE",
    ),
    (
        "ui.multiplayer.password",
        "LOBBY PASSWORD",
        "LOBBY-PASSWORT",
    ),
    (
        "ui.multiplayer.player_name",
        "COMMANDER CALLSIGN",
        "SPIELERNAME",
    ),
    // 5. Pre-Game Multiplayer Staging Room
    (
        "ui.staging.title",
        "MULTIPLAYER STAGING ROOM",
        "MEHRSPIELER-VORBEREITUNGSRAUM",
    ),
    ("ui.staging.slot", "SLOT", "PLATZ"),
    ("ui.staging.commander", "COMMANDER", "KOMMANDANT"),
    ("ui.staging.faction", "FACTION", "FRAKTION"),
    ("ui.staging.color", "COLOR", "FARBE"),
    ("ui.staging.team", "TEAM", "TEAM"),
    ("ui.staging.spawn", "SPAWN", "STARTPLATZ"),
    ("ui.staging.handicap", "HANDICAP", "HANDICAP"),
    ("ui.staging.map_sync", "MAP SYNC", "KARTEN-SYNC"),
    ("ui.staging.ready", "READY", "BEREIT"),
    ("ui.staging.not_ready", "NOT READY", "NICHT BEREIT"),
    ("ui.staging.open_slot", "OPEN SLOT", "OFFEN"),
    ("ui.staging.closed_slot", "CLOSED", "GESCHLOSSEN"),
    ("ui.staging.add_ai", "ADD AI BOT", "KI-BOT HINZUFÜGEN"),
    ("ui.staging.match_rules", "ENGAGEMENT RULES", "SPIELREGELN"),
    (
        "ui.staging.chat_placeholder",
        "Type lobby message...",
        "Lobby-Nachricht eingeben...",
    ),
    ("ui.staging.send_chat", "SEND", "SENDEN"),
    (
        "ui.staging.toggle_ready",
        "TOGGLE READY",
        "BEREITSCHAFT UMSCHALTEN",
    ),
    ("ui.staging.launch_match", "LAUNCH MATCH", "MATCH STARTEN"),
    (
        "ui.staging.leave_room",
        "LEAVE STAGING ROOM",
        "LOBBY VERLASSEN",
    ),
    // 6. Diplomacy & Tribute
    (
        "ui.diplomacy.title",
        "DIPLOMACY & TRIBUTE [D]",
        "DIPLOMATIE & TRIBUT [D]",
    ),
    ("ui.diplomacy.allied", "ALLIED", "VERBÜNDET"),
    ("ui.diplomacy.neutral", "NEUTRAL", "NEUTRAL"),
    ("ui.diplomacy.hostile", "HOSTILE", "FEINDLICH"),
    (
        "ui.diplomacy.shared_vision",
        "SHARED RADAR VISION",
        "GETEILTE RADARSICHT",
    ),
    (
        "ui.diplomacy.send_tribute",
        "SEND CREDIT TRIBUTE",
        "CREDITS ÜBERWEISEN",
    ),
    // 7. Faction Technology Tree
    (
        "ui.tech_tree.title",
        "FACTION TECHNOLOGY TREE",
        "TECHNOLOGIEBAUM DER FRAKTION",
    ),
    (
        "ui.tech_tree.buildings",
        "STRUCTURES & DEFENSES",
        "GEBÄUDE & VERTEIDIGUNG",
    ),
    (
        "ui.tech_tree.units",
        "COMBAT & SUPPORT UNITS",
        "KAMPF- & UNTERSTÜTZUNGSEINHEITEN",
    ),
    (
        "ui.tech_tree.upgrades",
        "DOCTRINES & UPGRADES",
        "DOKTRINEN & UPGRADES",
    ),
    (
        "ui.tech_tree.prerequisites",
        "PREREQUISITES",
        "VORAUSSETZUNGEN",
    ),
    (
        "ui.tech_tree.cost_and_time",
        "COST & BUILD TIME",
        "KOSTEN & BAUZEIT",
    ),
    (
        "ui.tech_tree.armor_counters",
        "ARMOR & WEAPON PROFILE",
        "PANZERUNGS- & WAFFENPROFIL",
    ),
    // 8. Objectives & EVA Logbook
    (
        "ui.logbook.title",
        "MISSION OBJECTIVES & EVA ARCHIVE [TAB]",
        "MISSIONSZIELE & EVA-ARCHIV [TAB]",
    ),
    (
        "ui.logbook.primary_objectives",
        "PRIMARY OBJECTIVES",
        "PRIMÄRZIELE",
    ),
    (
        "ui.logbook.eva_transmissions",
        "EVA TRANSMISSION LOG",
        "EVA-FUNKPROTOKOLL",
    ),
    // 9. Options & Settings
    (
        "ui.options.title",
        "ENGINE OPTIONS & CONTROLS",
        "ENGINE-OPTIONEN & STEUERUNG",
    ),
    ("ui.options.tab_audio", "AUDIO & MIXER", "AUDIO & MIXER"),
    ("ui.options.tab_graphics", "GRAPHICS & PBR", "GRAFIK & PBR"),
    (
        "ui.options.tab_gameplay",
        "GAMEPLAY & LANGUAGE",
        "GAMEPLAY & SPRACHE",
    ),
    (
        "ui.options.tab_hotkeys",
        "CONTROLS & HOTKEYS",
        "STEUERUNG & HOTKEYS",
    ),
    (
        "ui.options.language",
        "INTERFACE LANGUAGE",
        "OBERFLÄCHENSPRACHE",
    ),
    ("ui.options.language_en", "English", "English (Englisch)"),
    ("ui.options.language_de", "Deutsch (German)", "Deutsch"),
    (
        "ui.options.master_volume",
        "MASTER VOLUME",
        "GESAMTLAUTSTÄRKE",
    ),
    ("ui.options.music_volume", "MUSIC VOLUME", "MUSIKLAUTSTÄRKE"),
    ("ui.options.sfx_volume", "COMBAT SFX VOLUME", "KAMPFEFFEKTE"),
    ("ui.options.voice_volume", "UNIT VOICES", "EINHEITENSTIMMEN"),
    ("ui.options.eva_volume", "EVA ANNOUNCER", "EVA-SPRECHER"),
    ("ui.options.bloom", "HDR BLOOM", "HDR-BLOOM"),
    (
        "ui.options.ssao",
        "AMBIENT OCCLUSION (SSAO)",
        "UMGEBUNGSVERDECKUNG (SSAO)",
    ),
    (
        "ui.options.shadows",
        "CASCADE SHADOW QUALITY",
        "SCHATTENQUALITÄT",
    ),
    (
        "ui.options.grid_hotkeys",
        "QWER SIDEBAR GRID HOTKEYS",
        "QWER-RASTER-HOTKEYS (SIDEBAR)",
    ),
    (
        "ui.options.wasd_camera",
        "WASD CAMERA PANNING",
        "WASD-KAMERASTEUERUNG",
    ),
    (
        "ui.options.autosave_interval",
        "AUTOSAVE INTERVAL",
        "AUTOSAVE-INTERVALL",
    ),
    (
        "ui.options.reset_defaults",
        "RESET TO DEFAULTS",
        "AUF STANDARD ZURÜCKSETZEN",
    ),
    (
        "ui.options.rebind_prompt",
        "PRESS ANY KEY...",
        "TASTE DRÜCKEN...",
    ),
    (
        "ui.options.hotkey_conflict",
        "CONFLICT DETECTED",
        "TASTENKONFLIKT ERKANNT",
    ),
    // 10. Pause & Save / Load Game
    ("ui.pause.title", "TACTICAL PAUSE", "TAKTISCHE PAUSE"),
    (
        "ui.pause.resume",
        "RESUME BATTLE [ESC]",
        "WEITERSPIELEN [ESC]",
    ),
    (
        "ui.pause.save_load",
        "SAVE / LOAD GAME",
        "SPEICHERN / LADEN",
    ),
    (
        "ui.pause.restart_mission",
        "RESTART ENGAGEMENT",
        "MISSION NEU STARTEN",
    ),
    (
        "ui.pause.surrender",
        "SURRENDER / ABORT",
        "AUFGEBEN / ABBRECHEN",
    ),
    (
        "ui.pause.exit_to_menu",
        "EXIT TO MAIN MENU",
        "ZUM HAUPTMENÜ",
    ),
    (
        "ui.save_load.title",
        "SAVE & LOAD COMMAND STATE",
        "SPIELSTAND SPEICHERN & LADEN",
    ),
    (
        "ui.save_load.save_name",
        "SAVE TITLE",
        "NAME DES SPIELSTANDS",
    ),
    ("ui.save_load.save_button", "SAVE GAME", "SPEICHERN"),
    ("ui.save_load.load_button", "LOAD SELECTED", "AUSWAHL LADEN"),
    (
        "ui.save_load.delete_button",
        "DELETE SAVE",
        "SPIELSTAND LÖSCHEN",
    ),
    (
        "ui.save_load.quick_save",
        "QUICKSAVE [F5]",
        "SCHNELLSPEICHERN [F5]",
    ),
    (
        "ui.save_load.quick_load",
        "QUICKLOAD [F8]",
        "SCHNELLLADEN [F8]",
    ),
    ("ui.save_load.autosave_badge", "AUTOSAVE", "AUTOSAVE"),
    (
        "ui.save_load.mod_compatible",
        "MOD HASH VERIFIED",
        "MOD-VERSION KOMPATIBEL",
    ),
    (
        "ui.save_load.mod_mismatch",
        "MOD VERSION MISMATCH",
        "ABWEICHENDE MOD-VERSION",
    ),
    ("ui.save_load.play_time", "ELAPSED TIME", "SPIELZEIT"),
    // 11. Post-Game Victory / Defeat & Telemetry Screen
    (
        "ui.post_game.victory",
        "MISSION ACCOMPLISHED — VICTORY",
        "MISSION ERFÜLLT — SIEG",
    ),
    (
        "ui.post_game.defeat",
        "FORCES ANNIHILATED — DEFEAT",
        "STREITKRÄFTE VERNICHTET — NIEDERLAGE",
    ),
    (
        "ui.post_game.tab_overview",
        "OVERVIEW & MVP",
        "ÜBERSICHT & MVP",
    ),
    (
        "ui.post_game.tab_graphs",
        "ECONOMY & ARMY GRAPHS",
        "WIRTSCHAFTS- & ARMEE-GRAPHEN",
    ),
    (
        "ui.post_game.tab_units",
        "UNIT COMBAT LEDGER",
        "EINHEITEN-BILANZ",
    ),
    (
        "ui.post_game.tab_build_order",
        "BUILD ORDER TIMELINE",
        "BUILD-ORDER-ZEITSTRAHL",
    ),
    (
        "ui.post_game.mvp_unit",
        "MOST VALUABLE UNIT",
        "WERTVOLLSTE EINHEIT (MVP)",
    ),
    (
        "ui.post_game.elo_delta",
        "RANKED ELO RATING",
        "RANGLISTEN-ELO",
    ),
    (
        "ui.post_game.income_per_min",
        "INCOME / MIN",
        "EINKOMMEN / MIN",
    ),
    ("ui.post_game.army_value", "ARMY VALUE", "ARMEEWERT"),
    (
        "ui.post_game.apm",
        "ACTIONS PER MINUTE (APM)",
        "AKTIONEN PRO MINUTE (APM)",
    ),
    ("ui.post_game.units_built", "BUILT", "GEBAUT"),
    ("ui.post_game.units_killed", "KILLED", "ZERSTÖRT"),
    ("ui.post_game.units_lost", "LOST", "VERLOREN"),
    (
        "ui.post_game.damage_dealt",
        "DAMAGE DEALT",
        "SCHADEN VERURSACHT",
    ),
    (
        "ui.post_game.inspect_map",
        "CONTINUE VIEWING BATTLEFIELD",
        "SCHLACHTFELD WEITER BETRACHTEN",
    ),
    (
        "ui.post_game.save_replay",
        "SAVE REPLAY",
        "REPLAY SPEICHERN",
    ),
    (
        "ui.post_game.rematch",
        "PLAY AGAIN / REMATCH",
        "REVANCHE / NOCHMAL SPIELEN",
    ),
    (
        "ui.post_game.next_mission",
        "NEXT CAMPAIGN MISSION",
        "NÄCHSTE KAMPAGNENMISSION",
    ),
    // 12. 3D Replay & Spectator HUD
    (
        "ui.replay.observer_dashboard",
        "OBSERVER & REPLAY CONTROLS",
        "BEOBACHTER- & REPLAY-STEUERUNG",
    ),
    ("ui.replay.play", "PLAY", "WIEDERGABE"),
    ("ui.replay.pause", "PAUSE", "PAUSE"),
    ("ui.replay.step_back", "-10s", "-10s"),
    ("ui.replay.step_forward", "+10s", "+10s"),
    (
        "ui.replay.pov_all_fog_off",
        "ALL PLAYERS (FOG OFF)",
        "ALLE SPIELER (NEBEL AUS)",
    ),
    ("ui.replay.pov_player", "PLAYER POV", "SPIELER-SICHT"),
    (
        "ui.replay.active_production",
        "ACTIVE PRODUCTION",
        "LAUFENDE PRODUKTION",
    ),
    // 13. Top HUD & Sidebar Tabs
    ("ui.hud.credits", "CREDITS", "CREDITS"),
    ("ui.hud.power", "POWER", "ENERGIE"),
    ("ui.hud.supply", "SUPPLY", "VERSORGUNG"),
    ("ui.hud.objectives", "OBJECTIVES", "MISSIONSZIELE"),
    (
        "ui.hud.hot_reload",
        "HOT RELOAD MOD [F12]",
        "MOD LIVE NEU LADEN [F12]",
    ),
    ("ui.hud.chat_all", "[ALL]", "[ALLE]"),
    ("ui.hud.chat_team", "[TEAM]", "[TEAM]"),
    ("ui.sidebar.tab_base", "BASE STRUCTURES", "HAUPTGEBÄUDE"),
    (
        "ui.sidebar.tab_defense",
        "DEFENSE & SUPPORT",
        "VERTEIDIGUNG & SUPPORT",
    ),
    ("ui.sidebar.tab_infantry", "INFANTRY", "INFANTERIE"),
    (
        "ui.sidebar.tab_vehicles",
        "ARMOR & AIRCRAFT",
        "FAHRZEUGE & LUFTWAFFE",
    ),
    ("ui.sidebar.repair", "REPAIR", "REPARIEREN"),
    ("ui.sidebar.sell", "SELL", "VERKAUFEN"),
    ("ui.sidebar.ready", "READY", "BEREIT"),
    ("ui.sidebar.on_hold", "ON HOLD", "PAUSIERT"),
    // 14. Tactical Orders & Unit Stances
    ("ui.stance.aggressive", "AGGRESSIVE", "AGGRESSIV"),
    ("ui.stance.defensive", "DEFENSIVE", "DEFENSIV"),
    ("ui.stance.hold_ground", "HOLD POSITION", "POSITION HALTEN"),
    ("ui.stance.hold_fire", "HOLD FIRE", "FEUER EINSTELLEN"),
    (
        "ui.order.attack_move",
        "ATTACK MOVE [A]",
        "ANGRIFFSBEWEGUNG [A]",
    ),
    ("ui.order.stop", "STOP [S]", "STOPP [S]"),
    ("ui.order.scatter", "SCATTER [X]", "VERTEILEN [X]"),
    (
        "ui.order.deploy",
        "DEPLOY / ABILITY [F]",
        "AUFSTELLEN / FÄHIGKEIT [F]",
    ),
    ("ui.order.guard", "GUARD AREA [G]", "BEWACHEN [G]"),
    (
        "ui.order.patrol",
        "PATROL WAYPOINT [P]",
        "PATROUILLIEREN [P]",
    ),
    // 15. EVA Alerts & Network Status
    (
        "eva.building_complete",
        "Construction complete.",
        "Bau abgeschlossen.",
    ),
    ("eva.unit_ready", "Unit ready.", "Einheit bereit."),
    (
        "eva.under_attack",
        "Our base is under attack!",
        "Unsere Basis wird angegriffen!",
    ),
    (
        "eva.insufficient_funds",
        "Insufficient funds.",
        "Unzureichende Mittel.",
    ),
    (
        "eva.low_power",
        "Low power — defenses offline!",
        "Energiemangel — Verteidigung offline!",
    ),
    (
        "eva.superweapon_detected",
        "Warning: Superweapon activated!",
        "Warnung: Superwaffe aktiviert!",
    ),
    (
        "eva.reinforcements_arrived",
        "Reinforcements have arrived.",
        "Verstärkung ist eingetroffen.",
    ),
    (
        "ui.net.reconnecting",
        "CONNECTION INTERRUPTED — AUTO-RECONNECTING...",
        "VERBINDUNG UNTERBROCHEN — AUTOMATISCHER RECONNECT...",
    ),
    (
        "ui.net.downloading_map",
        "TRANSFERRING CUSTOM MAP...",
        "LADE BENUTZERDEFINIERTE KARTE...",
    ),
    (
        "ui.net.desync_detected",
        "SIMULATION DESYNC DETECTED — FORENSICS SAVED",
        "SIMULATIONS-DESYNC ERKANNT — BERICHT GESPEICHERT",
    ),
];

pub fn builtin_engine_locale(lang: &str, key: &str) -> Option<&'static str> {
    let is_de = lang.eq_ignore_ascii_case("de") || lang.to_ascii_lowercase().starts_with("de-");
    let is_en = lang.eq_ignore_ascii_case("en") || lang.to_ascii_lowercase().starts_with("en-");
    if !is_de && !is_en {
        return None;
    }
    for &(k, en, de) in BUILTIN_UI_STRINGS {
        if k == key {
            return Some(if is_de { de } else { en });
        }
    }
    None
}

pub fn builtin_engine_en_map() -> BTreeMap<&'static str, &'static str> {
    BUILTIN_UI_STRINGS
        .iter()
        .map(|&(k, en, _)| (k, en))
        .collect()
}

pub fn builtin_engine_de_map() -> BTreeMap<&'static str, &'static str> {
    BUILTIN_UI_STRINGS
        .iter()
        .map(|&(k, _, de)| (k, de))
        .collect()
}
