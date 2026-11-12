use crate::input::ClientInteractionState;
use rts_sdk::loader::LoadedMod;

/// Resolves a localized UI string via `LoadedMod::t(lang, key)`, falling back to built-in
/// English (`"en"`) and German (`"de"`) engine translations so switching language in
/// `Options -> Gameplay` immediately translates both mod-defined keys and core client screens.
pub fn ui_t(loaded: &LoadedMod, interaction: &ClientInteractionState, key: &str) -> String {
    let lang = if interaction.language.trim().is_empty() {
        "en"
    } else {
        interaction.language.trim()
    };
    let from_mod = loaded.t(lang, key);
    if from_mod != key {
        return from_mod.to_string();
    }
    builtin_fallback(lang, key).to_string()
}

/// Resolves built-in engine localization keys (`"en"` default, `"de"` German) with 100% key parity.
pub fn tr<'a>(lang: &str, key: &'a str) -> &'a str {
    let effective = if lang.trim().is_empty() {
        "en"
    } else {
        lang.trim()
    };
    builtin_fallback(effective, key)
}

fn builtin_fallback<'a>(lang: &str, key: &'a str) -> &'a str {
    let is_de = lang.eq_ignore_ascii_case("de") || lang.to_ascii_lowercase().starts_with("de-");
    if is_de {
        match key {
            "ui.menu.bootcamp" => "TUTORIAL & GRUNDAUSBILDUNG",
            "ui.menu.campaign" => "STORY-KAMPAGNE",
            "ui.menu.skirmish" => "GEFECHT (SKIRMISH)",
            "ui.menu.multiplayer" => "MEHRSPIELER (ONLINE & LAN)",
            "ui.menu.workshop" => "MOD-WORKSHOP & PAKETE",
            "ui.menu.theater" => "THEATER, SPEICHERSTAENDE & REPLAYS",
            "ui.menu.encyclopedia" => "EINHEITEN- & FRAKTIONSLEXIKON",
            "ui.menu.options" => "OPTIONEN & STEUERUNG",
            "ui.menu.workbench" => "CREATOR WORKBENCH [F12]",
            "ui.menu.exit" => "ZUM DESKTOP BEENDEN",
            "ui.menu.return_main" => "ZURUECK ZUM HAUPTMENUE",
            "ui.workshop.title" => "COMMUNITY MOD-WORKSHOP (.RTSMOD & KARTEN)",
            "ui.pause.title" => "TAKTISCHE GEFECHTSPAUSE",
            "ui.pause.resume" => "GEFECHT FORTSETZEN [ESC]",
            "ui.pause.briefing" => "EINSATZBESPRECHUNG & VIDEO",
            "ui.pause.save_load" => "SPIEL SPEICHERN / LADEN",
            "ui.pause.quicksave" => "SCHNELLSPEICHERN [F5]",
            "ui.pause.quickload" => "SCHNELLLADEN [F8]",
            "ui.pause.options" => "CLIENT-OPTIONEN (SKALIERUNG & AUDIO)",
            "ui.pause.restart" => "SEKTOR NEU STARTEN",
            "ui.nav.home" => "HAUPTMENUE",
            "ui.nav.campaign" => "KAMPAGNE",
            "ui.nav.skirmish" => "GEFECHT",
            "ui.nav.multiplayer" => "MEHRSPIELER",
            "ui.nav.theater" => "REPLAYS",
            "ui.nav.encyclopedia" => "LEXIKON",
            "ui.nav.options" => "OPTIONEN",
            "ui.options.graphics" => "1. GRAFIK",
            "ui.options.audio" => "2. 5-KANAL AUDIO",
            "ui.options.hotkeys" => "3. TASTENBELEGUNG",
            "ui.options.gameplay" => "4. SPIEL & SPRACHE",
            "ui.staging.title" => "MEHRSPIELER-EINSATZRAUM (8 SLOTS)",
            "ui.staging.ready" => "BEREIT MELDEN",
            "ui.staging.launch" => "GEFECHT STARTEN",
            "ui.postgame.victory" => "MISSION ERFOLGREICH — SIEG!",
            "ui.postgame.defeat" => "STREITKRAEFTE VERNICHTET — NIEDERLAGE",
            "ui.postgame.tab_summary" => "1. UEBERSICHT & MVP",
            "ui.postgame.tab_graphs" => "2. WIRTSCHAFT / ARMEE / APM",
            "ui.postgame.tab_ledger" => "3. EINHEITEN-BILANZ",
            "ui.postgame.tab_timeline" => "4. BAU-ZEITLEISTE",
            "ui.replay.all_vision" => "ALLE SPIELER (NEBEL AUS)",
            "hud_objectives_title" => "EINSATZZIELE",
            "hud_obj_focus" => "SEKTOR",
            "cutscene_skipped" => "EVA: Kamerasequenz uebersprungen.",
            "jukebox_playing" => "AKTIV",
            "jukebox_paused" => "PAUSIERT",
            "jukebox_no_track" => "Kein Musikstueck geladen",
            "cmd_tree_rank" => "KOMMANDEUR",
            "cmd_tree_points" => "BEFOERDERUNGSPUNKTE",
            "cmd_tree_none" => "Kein Kommandeur-Doktrinbaum fuer diese Fraktion definiert.",
            "lag_shield_title" => "VERBINDUNGSVERZOEGERUNG — WARTE AUF SPIELER",
            "hud_era" => "ZEITALTER",
            "hud_advance_era" => "FORTSCHRITT",
            "hud_eff" => "EFF",
            "hud_skill_pts" => "FP",
            "hud_revive_hero" => "WIEDERBELEBEN:",
            "desync_alert_title" => "NETZWERK-DESYNC ERKANNT — FORENSIK GESPEICHERT",
            "btn_resync_host" => "VOM HOST NEU SYNCHRONISIEREN [SOFORT]",
            "ui.menu.rejoin_match" => "LAUFENDES GEFECHT WIEDERBETRETEN",
            _ => key,
        }
    } else {
        match key {
            "ui.menu.bootcamp" => "TUTORIAL & BOOTCAMP",
            "ui.menu.campaign" => "STORY CAMPAIGN",
            "ui.menu.skirmish" => "SKIRMISH BATTLE",
            "ui.menu.multiplayer" => "MULTIPLAYER (ONLINE & LAN)",
            "ui.menu.workshop" => "MOD WORKSHOP & PACKAGES",
            "ui.menu.theater" => "THEATER, SAVES & REPLAYS",
            "ui.menu.encyclopedia" => "UNIT & FACTION ENCYCLOPEDIA",
            "ui.menu.options" => "OPTIONS & CONTROLS",
            "ui.menu.workbench" => "CREATOR WORKBENCH [F12]",
            "ui.menu.exit" => "EXIT TO DESKTOP",
            "ui.menu.return_main" => "RETURN TO MAIN MENU",
            "ui.workshop.title" => "COMMUNITY MOD WORKSHOP (.RTSMOD & MAPS)",
            "ui.pause.title" => "TACTICAL BATTLEFIELD PAUSE",
            "ui.pause.resume" => "RESUME BATTLE [ESC]",
            "ui.pause.briefing" => "MISSION BRIEFING & IN-ENGINE FMV",
            "ui.pause.save_load" => "SAVE / LOAD MATCH SLOTS",
            "ui.pause.quicksave" => "QUICK-SAVE MATCH [F5]",
            "ui.pause.quickload" => "QUICK-LOAD MATCH [F8]",
            "ui.pause.options" => "CLIENT OPTIONS (UI SCALE & 5-BUS AUDIO)",
            "ui.pause.restart" => "RESTART SECTOR",
            "ui.nav.home" => "MAIN MENU",
            "ui.nav.campaign" => "CAMPAIGN",
            "ui.nav.skirmish" => "SKIRMISH",
            "ui.nav.multiplayer" => "MULTIPLAYER",
            "ui.nav.theater" => "THEATER & REPLAYS",
            "ui.nav.encyclopedia" => "ENCYCLOPEDIA",
            "ui.nav.options" => "OPTIONS",
            "ui.options.graphics" => "1. GRAPHICS",
            "ui.options.audio" => "2. 5-BUS AUDIO",
            "ui.options.hotkeys" => "3. HOTKEYS (30)",
            "ui.options.gameplay" => "4. GAMEPLAY",
            "ui.staging.title" => "MULTIPLAYER STAGING ROOM (8 SLOTS)",
            "ui.staging.ready" => "TOGGLE READY",
            "ui.staging.launch" => "LAUNCH MATCH",
            "ui.postgame.victory" => "OPERATION COMPLETE — VICTORY!",
            "ui.postgame.defeat" => "COMMAND LOST — DEFEAT",
            "ui.postgame.tab_summary" => "1. SUMMARY & MVP",
            "ui.postgame.tab_graphs" => "2. ECONOMY / ARMY / APM GRAPHS",
            "ui.postgame.tab_ledger" => "3. PER-UNIT COMBAT LEDGER",
            "ui.postgame.tab_timeline" => "4. BUILD-ORDER TIMELINE",
            "ui.replay.all_vision" => "ALL PLAYERS — FOG OFF",
            "hud_objectives_title" => "MISSION OBJECTIVES",
            "hud_obj_focus" => "FOCUS",
            "cutscene_skipped" => "EVA: Camera sequence skipped.",
            "jukebox_playing" => "PLAYING",
            "jukebox_paused" => "PAUSED",
            "jukebox_no_track" => "No soundtrack loaded",
            "cmd_tree_rank" => "COMMANDER",
            "cmd_tree_points" => "PROMOTION POINTS",
            "cmd_tree_none" => "No Commander Power Tree defined for this faction.",
            "lag_shield_title" => "NETWORK LAG SHIELD — WAITING FOR PEER",
            "hud_era" => "ERA",
            "hud_advance_era" => "ADVANCE",
            "hud_eff" => "EFF",
            "hud_skill_pts" => "SP",
            "hud_revive_hero" => "REVIVE:",
            "desync_alert_title" => "MULTIPLAYER DESYNC DETECTED — FORENSICS DUMPED",
            "btn_resync_host" => "RESYNC FROM HOST [INSTANT]",
            "ui.menu.rejoin_match" => "REJOIN ACTIVE MATCH",
            _ => key,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUILTIN_KEYS: &[&str] = &[
        "ui.menu.bootcamp",
        "ui.menu.campaign",
        "ui.menu.skirmish",
        "ui.menu.multiplayer",
        "ui.menu.workshop",
        "ui.menu.theater",
        "ui.menu.encyclopedia",
        "ui.menu.options",
        "ui.menu.workbench",
        "ui.menu.exit",
        "ui.menu.return_main",
        "ui.workshop.title",
        "ui.pause.title",
        "ui.pause.resume",
        "ui.pause.briefing",
        "ui.pause.save_load",
        "ui.pause.quicksave",
        "ui.pause.quickload",
        "ui.pause.options",
        "ui.pause.restart",
        "ui.nav.home",
        "ui.nav.campaign",
        "ui.nav.skirmish",
        "ui.nav.multiplayer",
        "ui.nav.theater",
        "ui.nav.encyclopedia",
        "ui.nav.options",
        "ui.options.graphics",
        "ui.options.audio",
        "ui.options.hotkeys",
        "ui.options.gameplay",
        "ui.staging.title",
        "ui.staging.ready",
        "ui.staging.launch",
        "ui.postgame.victory",
        "ui.postgame.defeat",
        "ui.postgame.tab_summary",
        "ui.postgame.tab_graphs",
        "ui.postgame.tab_ledger",
        "ui.postgame.tab_timeline",
        "ui.replay.all_vision",
        "hud_objectives_title",
        "hud_obj_focus",
        "cutscene_skipped",
        "jukebox_playing",
        "jukebox_paused",
        "jukebox_no_track",
        "cmd_tree_rank",
        "cmd_tree_points",
        "cmd_tree_none",
        "lag_shield_title",
        "hud_era",
        "hud_advance_era",
        "hud_eff",
        "hud_skill_pts",
        "hud_revive_hero",
        "desync_alert_title",
        "btn_resync_host",
        "ui.menu.rejoin_match",
    ];

    #[test]
    fn locale_builtin_keys_have_100_percent_en_de_parity_and_en_default() {
        let default_cfg = crate::settings_store::UnifiedClientSettingsFile::default();
        assert_eq!(default_cfg.language, "en");
        for &k in BUILTIN_KEYS {
            let en = tr("en", k);
            let de = tr("de", k);
            assert_ne!(en, k, "Missing EN translation for {k}");
            assert_ne!(de, k, "Missing DE translation for {k}");
            assert_eq!(tr("", k), en, "Empty lang must default to EN for {k}");
        }
    }
}
