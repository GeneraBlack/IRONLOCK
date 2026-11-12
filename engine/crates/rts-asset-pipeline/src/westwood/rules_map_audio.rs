use super::*;
use rts_sdk::schema::{
    AudioCatalog, BuildingDef, FactionCatalog, MapDef, OrePatchDef, UnitDef, UnitVoiceSpec,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn get_ini_ci<'a>(ini: &'a WestwoodIni, section: &str, key: &str) -> Option<&'a str> {
    let sec_map = ini.sections.get(section).or_else(|| {
        ini.sections
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(section))
            .map(|(_, v)| v)
    })?;
    sec_map
        .get(key)
        .or_else(|| {
            sec_map
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v)
        })
        .map(String::as_str)
}

pub(super) fn list_values_ci(ini: &WestwoodIni, section: &str) -> Vec<String> {
    if let Some(sec_map) = ini.sections.get(section).or_else(|| {
        ini.sections
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(section))
            .map(|(_, v)| v)
    }) {
        let mut numeric_pairs: Vec<(usize, String)> = Vec::new();
        let mut fallback: Vec<String> = Vec::new();
        for (k, v) in sec_map {
            if v.is_empty() {
                continue;
            }
            if let Ok(idx) = k.parse::<usize>() {
                numeric_pairs.push((idx, v.clone()));
            } else {
                fallback.push(v.clone());
            }
        }
        numeric_pairs.sort_by_key(|(idx, _)| *idx);
        let mut out: Vec<String> = numeric_pairs.into_iter().map(|(_, v)| v).collect();
        out.extend(fallback);
        out
    } else {
        Vec::new()
    }
}

pub(super) fn resolve_art_entity_meta(
    sec: &str,
    entity_id: &str,
    rules_ini: &WestwoodIni,
    art_ini: Option<&WestwoodIni>,
) -> (Option<String>, Option<String>, Option<bool>) {
    let rules_image = get_ini_ci(rules_ini, sec, "Image")
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("none"));
    let art_sec_name = rules_image.unwrap_or(sec);

    let lookup_art_field = |key: &str| -> Option<&str> {
        if let Some(art) = art_ini {
            if let Some(v) = get_ini_ci(art, art_sec_name, key) {
                if !v.trim().is_empty() && !v.trim().eq_ignore_ascii_case("none") {
                    return Some(v.trim());
                }
            }
            if !art_sec_name.eq_ignore_ascii_case(sec) {
                if let Some(v) = get_ini_ci(art, sec, key) {
                    if !v.trim().is_empty() && !v.trim().eq_ignore_ascii_case("none") {
                        return Some(v.trim());
                    }
                }
            }
        }
        if let Some(v) = get_ini_ci(rules_ini, art_sec_name, key) {
            if !v.trim().is_empty() && !v.trim().eq_ignore_ascii_case("none") {
                return Some(v.trim());
            }
        }
        if !art_sec_name.eq_ignore_ascii_case(sec) {
            if let Some(v) = get_ini_ci(rules_ini, sec, key) {
                if !v.trim().is_empty() && !v.trim().eq_ignore_ascii_case("none") {
                    return Some(v.trim());
                }
            }
        }
        None
    };

    let cameo_icon = lookup_art_field("Cameo")
        .or_else(|| lookup_art_field("AltCameo"))
        .map(|raw_cameo| {
            let clean = raw_cameo.trim_end_matches(".shp").trim_end_matches(".SHP");
            format!("assets/icons/{}.png", normalize_id(clean))
        });

    let art_image = lookup_art_field("Image").or(rules_image);
    let model_override = art_image.and_then(|raw_img| {
        let clean = raw_img
            .trim_end_matches(".vxl")
            .trim_end_matches(".VXL")
            .trim_end_matches(".shp")
            .trim_end_matches(".SHP");
        let norm = normalize_id(clean);
        if !norm.is_empty() && norm != entity_id {
            Some(norm)
        } else {
            None
        }
    });

    let voxel_flag = lookup_art_field("Voxel").map(|v| parse_bool(Some(v)));

    (cameo_icon, model_override, voxel_flag)
}

fn clean_westwood_sound_stem(raw_token: &str) -> Option<String> {
    let trimmed = raw_token
        .trim()
        .trim_start_matches('$')
        .trim_start_matches('#')
        .trim_end_matches(".wav")
        .trim_end_matches(".WAV")
        .trim_end_matches(".aud")
        .trim_end_matches(".AUD")
        .trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("none")
        || trimmed.eq_ignore_ascii_case("null")
        || trimmed.eq_ignore_ascii_case("<none>")
    {
        return None;
    }
    let id = normalize_id(trimmed);
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

pub(super) fn resolve_sound_cue_paths(
    raw_cues: Option<&str>,
    sound_ini: Option<&WestwoodIni>,
    rules_ini: &WestwoodIni,
) -> Vec<String> {
    let Some(raw) = raw_cues else {
        return Vec::new();
    };
    let mut resolved = Vec::new();
    for cue in raw.split(',').map(str::trim) {
        if cue.is_empty() || cue.eq_ignore_ascii_case("none") || cue.eq_ignore_ascii_case("null") {
            continue;
        }
        let sounds_field = sound_ini
            .and_then(|s| get_ini_ci(s, cue, "Sounds"))
            .or_else(|| get_ini_ci(rules_ini, cue, "Sounds"));
        if let Some(sounds_str) = sounds_field {
            let mut matched_any = false;
            for tok in sounds_str
                .split(|c: char| c.is_ascii_whitespace() || c == ',')
                .filter(|t| !t.is_empty())
                .take(4)
            {
                if let Some(stem) = clean_westwood_sound_stem(tok) {
                    let rel = format!("assets/audio/{stem}.wav");
                    if !resolved.contains(&rel) {
                        resolved.push(rel);
                    }
                    matched_any = true;
                }
            }
            if matched_any {
                continue;
            }
        }
        if let Some(stem) = clean_westwood_sound_stem(cue) {
            let rel = format!("assets/audio/{stem}.wav");
            if !resolved.contains(&rel) {
                resolved.push(rel);
            }
        }
    }
    resolved
}

fn normalize_eva_cue_key(raw_sec: &str) -> String {
    let stripped = raw_sec
        .trim()
        .strip_prefix("EVA_")
        .or_else(|| raw_sec.trim().strip_prefix("eva_"))
        .unwrap_or_else(|| raw_sec.trim());
    let mut snake = String::with_capacity(stripped.len() + 4);
    let mut prev_lower = false;
    for ch in stripped.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && prev_lower && !snake.ends_with('_') {
                snake.push('_');
            }
            snake.push(ch.to_ascii_lowercase());
            prev_lower = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else if !snake.is_empty() && !snake.ends_with('_') {
            snake.push('_');
            prev_lower = false;
        }
    }
    let clean = snake.trim_matches('_').to_string();
    if clean.is_empty() {
        normalize_id(raw_sec)
    } else {
        clean
    }
}

pub(super) fn assemble_westwood_audio_catalog(
    ini: &WestwoodIni,
    sound_ini: Option<&WestwoodIni>,
    eva_ini: Option<&WestwoodIni>,
    weapon_raw_sections: &BTreeMap<String, String>,
    unit_voices_map: BTreeMap<String, UnitVoiceSpec>,
    defined_factions: &BTreeSet<String>,
) -> Option<AudioCatalog> {
    // Link weapon `Report=` SFX cues via `sound.ini` / `rules.ini`
    let mut weapon_sfx_map: BTreeMap<String, String> = BTreeMap::new();
    for (w_id, raw_w_sec) in weapon_raw_sections {
        let report_raw = get_ini_ci(ini, raw_w_sec, "Report");
        if let Some(first_sfx) = resolve_sound_cue_paths(report_raw, sound_ini, ini)
            .into_iter()
            .next()
        {
            weapon_sfx_map.insert(w_id.clone(), first_sfx);
        }
    }

    // Link EVA cues from `eva.ini` (or `[DialogList]` embedded in `rules.ini` / `sound.ini`)
    let mut eva_cues_map: BTreeMap<String, String> = BTreeMap::new();
    let mut faction_eva_map: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let eva_source = eva_ini.or_else(|| {
        if !list_values_ci(ini, "DialogList").is_empty() {
            Some(ini)
        } else {
            sound_ini.filter(|s| !list_values_ci(s, "DialogList").is_empty())
        }
    });
    if let Some(eva_doc) = eva_source {
        let dialog_sections = list_values_ci(eva_doc, "DialogList");
        for dlg_sec in dialog_sections {
            let cue_key = normalize_eva_cue_key(&dlg_sec);
            let allied_stem =
                get_ini_ci(eva_doc, &dlg_sec, "Allied").and_then(clean_westwood_sound_stem);
            let soviet_stem = get_ini_ci(eva_doc, &dlg_sec, "Russian")
                .or_else(|| get_ini_ci(eva_doc, &dlg_sec, "Soviet"))
                .and_then(clean_westwood_sound_stem);
            let yuri_stem =
                get_ini_ci(eva_doc, &dlg_sec, "Yuri").and_then(clean_westwood_sound_stem);
            let default_stem = get_ini_ci(eva_doc, &dlg_sec, "Normal")
                .or_else(|| get_ini_ci(eva_doc, &dlg_sec, "Default"))
                .and_then(clean_westwood_sound_stem)
                .or_else(|| allied_stem.clone())
                .or_else(|| soviet_stem.clone())
                .or_else(|| yuri_stem.clone());

            if let Some(def_s) = default_stem {
                eva_cues_map.insert(cue_key.clone(), format!("assets/audio/{def_s}.wav"));
            }
            for (fac_id, opt_stem) in [
                ("allies", allied_stem),
                ("soviets", soviet_stem),
                ("yuri", yuri_stem),
            ] {
                if defined_factions.contains(fac_id) {
                    if let Some(stem) = opt_stem {
                        faction_eva_map
                            .entry(fac_id.to_string())
                            .or_default()
                            .insert(cue_key.clone(), format!("assets/audio/{stem}.wav"));
                    }
                }
            }
        }
    }

    if !weapon_sfx_map.is_empty()
        || !unit_voices_map.is_empty()
        || !eva_cues_map.is_empty()
        || !faction_eva_map.is_empty()
    {
        Some(AudioCatalog {
            weapon_sfx: weapon_sfx_map,
            unit_voices: unit_voices_map,
            music_tracks: Vec::new(),
            eva_cues: eva_cues_map,
            faction_eva_cues: faction_eva_map,
        })
    } else {
        None
    }
}

/// Parses a Westwood `.map` / `.mpr` INI (`[Map]`, `[Waypoints]`, `[Structures]`, `[Units]`, `[Infantry]`,
/// `[IsoMapPack5]`, `[OverlayPack]`, `[OverlayDataPack]`, and `[Triggers]`/`[Events]`/`[Actions]`) into a `MapDef` if present.
pub fn parse_westwood_map_ini(
    ini: &WestwoodIni,
    buildings: &[BuildingDef],
    units: &[UnitDef],
    factions: &FactionCatalog,
) -> Option<MapDef> {
    let size_str = ini
        .get("Map", "Size")
        .or_else(|| ini.get("Map", "LocalSize"))?;
    let parts: Vec<u32> = size_str
        .split(',')
        .filter_map(|s| s.trim().parse::<u32>().ok())
        .collect();
    if parts.len() < 4 {
        return None;
    }
    let width = parts[2].clamp(24, 128);
    let height = parts[3].clamp(24, 128);

    let map_name = ini
        .get("Basic", "Name")
        .unwrap_or("Converted Westwood Map")
        .to_string();

    let mut base_map = build_default_converted_map(buildings, units);
    base_map.name = map_name.clone();
    base_map.id = normalize_id(&map_name);
    base_map.width = width;
    base_map.height = height;

    // Extract multiplayer start waypoints (`0..8`) and full trigger waypoint table (`0..128`)
    let mut wp_coords: Vec<[i32; 2]> = Vec::new();
    let mut all_waypoints: BTreeMap<i32, [i32; 2]> = BTreeMap::new();
    let max_x = (width as i32 - 5).max(4);
    let max_y = (height as i32 - 5).max(4);
    let decode_cell = |cell: i32| -> [i32; 2] {
        let (wx, wy) = if cell >= 1000 {
            ((cell % 1000).min(max_x), (cell / 1000).min(max_y))
        } else {
            ((cell % 128).min(max_x), (cell / 128).min(max_y))
        };
        [wx.max(4), wy.max(4)]
    };

    if let Some(wp_sec) = ini.sections.get("Waypoints") {
        for (k, val) in wp_sec {
            if let (Ok(wp_idx), Ok(cell)) = (k.trim().parse::<i32>(), val.trim().parse::<i32>()) {
                all_waypoints.insert(wp_idx, decode_cell(cell));
            }
        }
    }
    for wp_idx in 0..8 {
        if let Some(&coord) = all_waypoints.get(&wp_idx) {
            wp_coords.push(coord);
        }
    }

    if wp_coords.len() >= 2 {
        let templates = base_map.spawns.clone();
        let mut new_spawns = Vec::with_capacity(wp_coords.len());
        for (idx, &coord) in wp_coords.iter().enumerate() {
            let mut spawn = templates
                .get(idx % templates.len().max(1))
                .cloned()
                .unwrap_or_else(|| rts_sdk::schema::PlayerSpawnDef {
                    player_id: (idx + 1) as u8,
                    faction: "allies".to_string(),
                    sub_faction: None,
                    base_tile: coord,
                    team_id: 0,
                    initial_buildings: Vec::new(),
                    initial_units: Vec::new(),
                });
            spawn.player_id = (idx + 1) as u8;
            spawn.base_tile = coord;
            let sign_x = if coord[0] <= (width as i32) / 2 {
                1
            } else {
                -1
            };
            let sign_y = if coord[1] <= (height as i32) / 2 {
                1
            } else {
                -1
            };
            for b in &mut spawn.initial_buildings {
                b.tile_pos = coord;
            }
            for (u_idx, u) in spawn.initial_units.iter_mut().enumerate() {
                let dx = 3 + (u_idx as i32 % 3);
                let dy = 1 + (u_idx as i32 / 3);
                u.tile_pos = [
                    (coord[0] + sign_x * dx).clamp(2, width as i32 - 2),
                    (coord[1] + sign_y * dy).clamp(2, height as i32 - 2),
                ];
            }
            if let Some(f) = factions.factions.get(idx % factions.factions.len().max(1)) {
                spawn.faction = f.id.clone();
            }
            new_spawns.push(spawn);
        }
        base_map.spawns = new_spawns;
        base_map.ore_patches = wp_coords
            .iter()
            .map(|c| {
                let sign_x = if c[0] <= (width as i32) / 2 { 1 } else { -1 };
                let sign_y = if c[1] <= (height as i32) / 2 { 1 } else { -1 };
                OrePatchDef {
                    position: [
                        (c[0] + sign_x * 5).clamp(4, width as i32 - 4),
                        (c[1] + sign_y * 4).clamp(4, height as i32 - 4),
                    ],
                    radius: 3,
                    amount_per_tile: 650,
                }
            })
            .collect();
    }

    // Parse preplaced [Structures], [Units], and [Infantry] when present in .map/.mpr INI
    let known_bld: BTreeSet<String> = buildings.iter().map(|b| b.id.clone()).collect();
    let known_unt: BTreeSet<String> = units.iter().map(|u| u.id.clone()).collect();
    let map_house_owner = |house: &str| -> u8 {
        let h = house.trim().to_ascii_lowercase();
        if h.contains("neutral") || h.contains("civilian") || h.contains("special") {
            0
        } else if h.contains("bad")
            || h.contains("ussr")
            || h.contains("soviet")
            || h.contains("nod")
        {
            2
        } else {
            1
        }
    };

    if let Some(sec_map) = ini.sections.get("Structures") {
        for raw in sec_map.values() {
            let cols: Vec<&str> = raw.split(',').map(str::trim).collect();
            if cols.len() >= 5 {
                let b_id = normalize_id(cols[1]);
                if known_bld.contains(&b_id) {
                    let pos =
                        if let (Ok(x), Ok(y)) = (cols[3].parse::<i32>(), cols[4].parse::<i32>()) {
                            [x.clamp(2, max_x), y.clamp(2, max_y)]
                        } else if let Ok(cell) = cols[3].parse::<i32>() {
                            decode_cell(cell)
                        } else {
                            continue;
                        };
                    base_map
                        .preplaced_entities
                        .push(rts_sdk::schema::PreplacedEntityDef {
                            player_id: map_house_owner(cols[0]),
                            unit_id: None,
                            building_id: Some(b_id),
                            tile_pos: pos,
                            ..Default::default()
                        });
                }
            }
        }
    }

    for unit_sec in ["Units", "Infantry"] {
        if let Some(sec_map) = ini.sections.get(unit_sec) {
            for raw in sec_map.values() {
                let cols: Vec<&str> = raw.split(',').map(str::trim).collect();
                if cols.len() >= 5 {
                    let u_id = normalize_id(cols[1]);
                    if known_unt.contains(&u_id) {
                        let pos = if let (Ok(x), Ok(y)) =
                            (cols[3].parse::<i32>(), cols[4].parse::<i32>())
                        {
                            [x.clamp(2, max_x), y.clamp(2, max_y)]
                        } else if let Ok(cell) = cols[3].parse::<i32>() {
                            decode_cell(cell)
                        } else {
                            continue;
                        };
                        base_map
                            .preplaced_entities
                            .push(rts_sdk::schema::PreplacedEntityDef {
                                player_id: map_house_owner(cols[0]),
                                unit_id: Some(u_id),
                                building_id: None,
                                tile_pos: pos,
                                ..Default::default()
                            });
                    }
                }
            }
        }
    }

    // Remove any default obstacles that overlap spawns or preplaced entities
    base_map.obstacles.retain(|obs| {
        let hits_spawn = base_map.spawns.iter().any(|sp| {
            let b_hit = sp.base_tile[0] + 3 >= obs.min[0]
                && sp.base_tile[0] - 1 <= obs.max[0]
                && sp.base_tile[1] + 3 >= obs.min[1]
                && sp.base_tile[1] - 1 <= obs.max[1];
            let u_hit = sp.initial_units.iter().any(|u| {
                u.tile_pos[0] >= obs.min[0]
                    && u.tile_pos[0] <= obs.max[0]
                    && u.tile_pos[1] >= obs.min[1]
                    && u.tile_pos[1] <= obs.max[1]
            });
            b_hit || u_hit
        });
        let hits_preplaced = base_map.preplaced_entities.iter().any(|pe| {
            pe.tile_pos[0] + 2 >= obs.min[0]
                && pe.tile_pos[0] - 1 <= obs.max[0]
                && pe.tile_pos[1] + 2 >= obs.min[1]
                && pe.tile_pos[1] - 1 <= obs.max[1]
        });
        !hits_spawn && !hits_preplaced
    });

    // Decode `[IsoMapPack5]`, `[OverlayPack]`, `[OverlayDataPack]`, and `[Triggers]`/`[Events]`/`[Actions]`
    if let Ok(Some(decoded)) =
        decode_westwood_map_packs(ini, width, height, &all_waypoints, units, factions)
    {
        apply_isomappack_to_map_def(&mut base_map, &decoded);
    }

    Some(base_map)
}
