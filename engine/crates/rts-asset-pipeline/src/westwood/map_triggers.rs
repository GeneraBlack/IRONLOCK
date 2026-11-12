use super::map_pack_codec::parse_westwood_waypoint_token;
use super::{normalize_id, WestwoodIni};
use rts_sdk::schema::{FactionCatalog, TriggerAction, TriggerCondition, TriggerZoneDef, UnitDef};
use std::collections::BTreeMap;

/// Parses `[Triggers]`, `[Events]`, `[Actions]`, `[Tags]`, and `[CellTags]` from a Westwood `.map` / `.mpr` INI
/// into engine `TriggerZoneDef` records.
pub fn parse_westwood_map_triggers(
    ini: &WestwoodIni,
    width: u32,
    height: u32,
    waypoints: &BTreeMap<i32, [i32; 2]>,
    known_units: &[UnitDef],
    _factions: &FactionCatalog,
) -> Vec<TriggerZoneDef> {
    let Some(triggers_sec) = ini.sections.get("Triggers").or_else(|| {
        ini.sections
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("Triggers"))
            .map(|(_, v)| v)
    }) else {
        return Vec::new();
    };

    let max_x = (width as i32 - 2).max(1);
    let max_y = (height as i32 - 2).max(1);
    let decode_cell = |cell: i32| -> [i32; 2] {
        let (wx, wy) = if cell >= 1000 {
            ((cell % 1000).min(max_x), (cell / 1000).min(max_y))
        } else {
            ((cell % 128).min(max_x), (cell / 128).min(max_y))
        };
        [wx.max(1), wy.max(1)]
    };

    // Map Tag ID -> Trigger ID from `[Tags]` (`<tag_id> = <repeat>,<name>,<trig_id>`)
    let mut tag_to_trigger: BTreeMap<String, String> = BTreeMap::new();
    if let Some(tags_sec) = ini.sections.get("Tags") {
        for (tag_id, raw) in tags_sec {
            let cols: Vec<&str> = raw.split(',').map(str::trim).collect();
            if cols.len() >= 3 {
                tag_to_trigger.insert(tag_id.trim().to_string(), cols[2].to_string());
            }
        }
    }

    // Map Trigger ID -> Cell coordinates from `[CellTags]` (`<cell_num> = <tag_id>`)
    let mut trigger_cells: BTreeMap<String, Vec<[i32; 2]>> = BTreeMap::new();
    if let Some(cell_tags_sec) = ini.sections.get("CellTags") {
        for (cell_str, tag_id) in cell_tags_sec {
            if let Ok(cell_num) = cell_str.trim().parse::<i32>() {
                let trig_id = tag_to_trigger
                    .get(tag_id.trim())
                    .cloned()
                    .unwrap_or_else(|| tag_id.trim().to_string());
                trigger_cells
                    .entry(trig_id)
                    .or_default()
                    .push(decode_cell(cell_num));
            }
        }
    }

    let default_unit_id = known_units.first().map(|u| u.id.clone());
    let default_center = [
        (width as i32 / 2).clamp(2, max_x),
        (height as i32 / 2).clamp(2, max_y),
    ];

    let map_house_to_player = |house: &str| -> u8 {
        let h = house.trim().to_ascii_lowercase();
        if h.contains("bad")
            || h.contains("soviet")
            || h.contains("ussr")
            || h.contains("nod")
            || h == "1"
            || h == "2"
        {
            2
        } else {
            1
        }
    };

    let mut zones = Vec::new();
    for (trig_id, trig_raw) in triggers_sec {
        let t_cols: Vec<&str> = trig_raw.split(',').map(str::trim).collect();
        if t_cols.is_empty() {
            continue;
        }
        let house_player = map_house_to_player(t_cols[0]);
        let trig_name = t_cols.get(2).copied().unwrap_or(trig_id);
        let disabled = t_cols.get(3).copied().unwrap_or("0") == "1";
        if disabled {
            continue;
        }
        let repeat_code = t_cols.get(7).copied().unwrap_or("0");
        let once = repeat_code == "0";

        let zone_bounds = if let Some(cells) = trigger_cells.get(trig_id) {
            let min_cx = cells
                .iter()
                .map(|c| c[0])
                .min()
                .unwrap_or(default_center[0]);
            let max_cx = cells
                .iter()
                .map(|c| c[0])
                .max()
                .unwrap_or(default_center[0]);
            let min_cy = cells
                .iter()
                .map(|c| c[1])
                .min()
                .unwrap_or(default_center[1]);
            let max_cy = cells
                .iter()
                .map(|c| c[1])
                .max()
                .unwrap_or(default_center[1]);
            (
                [(min_cx - 1).max(1), (min_cy - 1).max(1)],
                [(max_cx + 1).min(max_x), (max_cy + 1).min(max_y)],
            )
        } else {
            (
                [
                    (default_center[0] - 2).max(1),
                    (default_center[1] - 2).max(1),
                ],
                [
                    (default_center[0] + 2).min(max_x),
                    (default_center[1] + 2).min(max_y),
                ],
            )
        };

        // Parse `[Events]` for `trig_id`: `<count>,<ev_id>,<flag>,<p1>,...`
        let mut conditions = Vec::new();
        if let Some(ev_raw) = ini.get("Events", trig_id) {
            let ev_cols: Vec<&str> = ev_raw.split(',').map(str::trim).collect();
            if let Some(ev_count) = ev_cols.first().and_then(|s| s.parse::<usize>().ok()) {
                let mut idx = 1usize;
                for _ in 0..ev_count {
                    if idx >= ev_cols.len() {
                        break;
                    }
                    let ev_type = ev_cols[idx].parse::<i32>().unwrap_or(-1);
                    let ev_flag = ev_cols
                        .get(idx + 1)
                        .and_then(|s| s.parse::<i32>().ok())
                        .unwrap_or(0);
                    let p1 = ev_cols.get(idx + 2).copied().unwrap_or("0");
                    idx += if ev_flag == 2 { 4 } else { 3 };

                    let cond = match ev_type {
                        1 | 24 | 45 => Some(TriggerCondition::EnteredZone {
                            player_id: Some(house_player),
                        }),
                        8 | 13 | 14 => {
                            let secs = p1.parse::<f32>().unwrap_or(30.0).max(1.0);
                            Some(TriggerCondition::TimerElapsed { secs })
                        }
                        9 | 11 | 31 | 48 => Some(TriggerCondition::AllBuildingsDestroyed {
                            player_id: if house_player == 1 { 2 } else { 1 },
                        }),
                        36 => Some(TriggerCondition::VariableAtLeast {
                            name: format!("local_{p1}"),
                            value: 1,
                        }),
                        37 => Some(TriggerCondition::VariableEquals {
                            name: format!("local_{p1}"),
                            value: 0,
                        }),
                        _ => None,
                    };
                    if let Some(c) = cond {
                        conditions.push(c);
                    }
                }
            }
        }

        let condition = match conditions.len() {
            0 => Some(if trigger_cells.contains_key(trig_id) {
                TriggerCondition::EnteredZone { player_id: Some(1) }
            } else {
                TriggerCondition::TimerElapsed { secs: 60.0 }
            }),
            1 => conditions.into_iter().next(),
            _ => Some(TriggerCondition::All { conditions }),
        };

        // Parse `[Actions]` for `trig_id`: `<count>,<act_type>,<a1>,<a2>,<a3>,<a4>,<a5>,<a6>,<a7_wp>`
        let mut actions = Vec::new();
        if let Some(act_raw) = ini.get("Actions", trig_id) {
            let act_cols: Vec<&str> = act_raw.split(',').map(str::trim).collect();
            if let Some(act_count) = act_cols.first().and_then(|s| s.parse::<usize>().ok()) {
                let rem = &act_cols[1..];
                let stride = if act_count > 0 && rem.len() >= act_count * 8 {
                    8usize
                } else if act_count > 0 && rem.len() >= act_count * 4 {
                    rem.len() / act_count
                } else {
                    rem.len().max(1)
                };

                for a_idx in 0..act_count {
                    let base = a_idx * stride;
                    if base >= rem.len() {
                        break;
                    }
                    let act_type = rem[base].parse::<i32>().unwrap_or(-1);
                    let p1 = rem.get(base + 1).copied().unwrap_or("0");
                    let p2 = rem.get(base + 2).copied().unwrap_or("0");
                    let wp_tok = rem
                        .get(base + stride.saturating_sub(1))
                        .copied()
                        .unwrap_or("0");
                    let wp_idx = parse_westwood_waypoint_token(wp_tok)
                        .or_else(|| p2.parse::<i32>().ok())
                        .unwrap_or(0);
                    let target_tile = waypoints.get(&wp_idx).copied().unwrap_or(zone_bounds.0);

                    match act_type {
                        1 => actions.push(TriggerAction::DeclareWinner {
                            player_id: house_player,
                        }),
                        2 => actions.push(TriggerAction::DeclareWinner {
                            player_id: if house_player == 1 { 2 } else { 1 },
                        }),
                        4 | 7 | 80 | 107 => {
                            if let Some(ref uid) = default_unit_id {
                                actions.push(TriggerAction::SpawnReinforcements {
                                    player_id: house_player,
                                    unit_id: uid.clone(),
                                    count: 2,
                                    tile: target_tile,
                                });
                            }
                        }
                        11 | 103 => {
                            let msg = if p2.is_empty() || p2 == "0" {
                                trig_name.to_string()
                            } else {
                                p2.to_string()
                            };
                            actions.push(TriggerAction::EvaTransmission {
                                speaker: "EVA".to_string(),
                                text: msg,
                                portrait: None,
                                audio_cue: None,
                                radar_video: None,
                            });
                        }
                        16 => actions.push(TriggerAction::RevealArea {
                            tile: default_center,
                            radius: (width.max(height) as i32).max(16),
                        }),
                        17 => actions.push(TriggerAction::RevealArea {
                            tile: target_tile,
                            radius: 10,
                        }),
                        41 => actions.push(TriggerAction::FocusCamera { tile: target_tile }),
                        53 | 56 => actions.push(TriggerAction::SetVariable {
                            name: format!("local_{p2}"),
                            value: i64::from(act_type == 53),
                        }),
                        105 | 125 => {
                            let amt = p2
                                .parse::<u32>()
                                .ok()
                                .or_else(|| p1.parse::<u32>().ok())
                                .unwrap_or(1000)
                                .max(100);
                            actions.push(TriggerAction::GrantCredits {
                                player_id: house_player,
                                amount: amt,
                            });
                        }
                        _ => {}
                    }
                }
            }
        }

        if actions.is_empty() {
            actions.push(TriggerAction::EvaTransmission {
                speaker: "EVA".to_string(),
                text: trig_name.to_string(),
                portrait: None,
                audio_cue: None,
                radar_video: None,
            });
        }

        zones.push(TriggerZoneDef {
            id: normalize_id(&format!("trig_{trig_id}_{trig_name}")),
            min: zone_bounds.0,
            max: zone_bounds.1,
            once,
            script_hook: String::new(),
            condition,
            actions,
        });
    }

    zones
}
