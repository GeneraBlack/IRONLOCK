use super::skirmish::{
    resolve_country_def_by_id, resolve_signature_unit_label, spawn_country_flag_badge,
};
use super::{spawn_body_text, spawn_screen_card};
use crate::input::{ClientInteractionState, UiAssetCache};
use crate::ui::studio_layout::{spawn_icon_styled_button, spawn_styled_button};
use crate::ui::widgets::{UiScrollContainer, UiScrollContent};
use crate::ui::SidebarAction;
use crate::AssetRootResource;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;

fn exclusivity_suffix(loaded: &LoadedMod, sub_factions: &[String]) -> String {
    if sub_factions.is_empty() {
        return String::new();
    }
    if sub_factions
        .iter()
        .any(|s| s.eq_ignore_ascii_case("campaign_vip"))
    {
        return " [VIP]".to_string();
    }
    if sub_factions
        .iter()
        .any(|s| s.eq_ignore_ascii_case("unbuildable_minion"))
    {
        return " [CREW]".to_string();
    }
    let names: Vec<String> = sub_factions
        .iter()
        .map(|sid| {
            if let Some((_, sf)) = resolve_country_def_by_id(loaded, sid) {
                sf.name
                    .split('(')
                    .next()
                    .map(str::trim)
                    .unwrap_or(&sf.name)
                    .to_uppercase()
            } else {
                sid.to_uppercase()
            }
        })
        .collect();
    format!(" [{} ONLY]", names.join("/"))
}

pub fn spawn_encyclopedia_screen(
    parent: &mut ChildBuilder,
    asset_server: Option<&AssetServer>,
    _asset_root: &AssetRootResource,
    ui_cache: &mut UiAssetCache,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
) {
    let active_fac = if !interaction.selected_encyclopedia_faction.is_empty()
        && loaded
            .factions
            .contains_key(&interaction.selected_encyclopedia_faction)
    {
        interaction.selected_encyclopedia_faction.clone()
    } else {
        loaded
            .factions
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "allies".to_string())
    };

    // Column 1: Faction & Playable Country Doctrines (with 3-Stripe Flag Badges)
    spawn_screen_card(
        parent,
        28.0,
        "1. FACTION & COUNTRIES",
        Color::srgb(0.98, 0.84, 0.42),
        |col| {
            for (fid, fdef) in &loaded.factions {
                let sel = fid == &active_fac;
                let [r, g, b] = fdef.color;
                spawn_styled_button(
                    col,
                    &fdef.name,
                    SidebarAction::SelectEncyclopediaFaction(fid.clone()),
                    if sel {
                        Color::srgb(
                            (r * 0.65 + 0.14).min(0.85),
                            (g * 0.65 + 0.14).min(0.85),
                            (b * 0.65 + 0.14).min(0.85),
                        )
                    } else {
                        Color::srgb(0.16, 0.22, 0.32)
                    },
                    Val::Percent(100.0),
                );
            }
            spawn_styled_button(
                col,
                if interaction.encyclopedia_show_buildings {
                    "Category: STRUCTURES (Click for Units)"
                } else {
                    "Category: COMBAT UNITS (Click for Structures)"
                },
                SidebarAction::ToggleEncyclopediaCategory,
                Color::srgb(0.22, 0.36, 0.48),
                Val::Percent(100.0),
            );
            if let Some(fdef) = loaded.factions.get(&active_fac) {
                spawn_body_text(col, &fdef.description, Color::srgb(0.80, 0.88, 0.96));
                if !fdef.sub_factions.is_empty() {
                    spawn_body_text(
                        col,
                        "Playable Member Countries & Passive Bonuses:",
                        Color::srgb(0.98, 0.86, 0.44),
                    );
                    for sf in &fdef.sub_factions {
                        col.spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(6.0),
                            ..default()
                        })
                        .with_children(|row| {
                            spawn_country_flag_badge(row, sf.resolved_flag_colors(fdef.color));
                            spawn_body_text(
                                row,
                                format!(
                                    "{} — {}\nSig: {}",
                                    sf.name,
                                    sf.bonus_summary(),
                                    resolve_signature_unit_label(loaded, sf)
                                ),
                                Color::srgb(0.84, 0.93, 0.98),
                            );
                        });
                    }
                }
            }
        },
    );

    // Column 2: Scrollable Field Catalog (labeling country-exclusive units/defenses clearly)
    let entries: Vec<(String, String, Option<Handle<Image>>)> =
        if interaction.encyclopedia_show_buildings {
            loaded
                .buildings
                .values()
                .filter(|b| b.is_available_to_faction(&active_fac))
                .map(|b| {
                    let c = ui_cache.load_entity_cameo(loaded, asset_server, &b.id);
                    let excl = exclusivity_suffix(loaded, &b.sub_factions);
                    (b.id.clone(), format!("{} (${}){excl}", b.name, b.cost), c)
                })
                .collect()
        } else {
            loaded
                .units
                .values()
                .filter(|u| u.is_available_to_faction(&active_fac))
                .map(|u| {
                    let c = ui_cache.load_entity_cameo(loaded, asset_server, &u.id);
                    let excl = exclusivity_suffix(loaded, &u.sub_factions);
                    (u.id.clone(), format!("{} (${}){excl}", u.name, u.cost), c)
                })
                .collect()
        };

    spawn_screen_card(
        parent,
        35.0,
        "2. TACTICAL CATALOG (SCROLL)",
        Color::srgb(0.65, 0.88, 1.0),
        |col| {
            let max_scroll = (entries.len() as f32 * 38.0 - 380.0).max(0.0);
            col.spawn((
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(0.0),
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    overflow: Overflow::clip_y(),
                    ..default()
                },
                UiScrollContainer::new(max_scroll),
            ))
            .with_children(|vp| {
                vp.spawn((
                    UiScrollContent,
                    Node {
                        width: Val::Percent(100.0),
                        position_type: PositionType::Relative,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        ..default()
                    },
                ))
                .with_children(|list| {
                    for (eid, label, cameo) in &entries {
                        let sel = interaction.selected_encyclopedia_entry.as_deref() == Some(eid);
                        spawn_icon_styled_button(
                            list,
                            label,
                            cameo.clone(),
                            SidebarAction::InspectEncyclopediaEntry(eid.clone()),
                            if sel {
                                Color::srgb(0.24, 0.48, 0.32)
                            } else {
                                Color::srgb(0.16, 0.24, 0.36)
                            },
                            Val::Percent(100.0),
                        );
                    }
                });
            });
        },
    );

    // Column 3: Detailed Combat, Country Exclusivity, Locomotor & Trait Dossier
    let selected_id = interaction
        .selected_encyclopedia_entry
        .as_deref()
        .or_else(|| entries.first().map(|(id, _, _)| id.as_str()));

    spawn_screen_card(
        parent,
        37.0,
        "3. TECHNICAL DOSSIER",
        Color::srgb(0.58, 0.95, 0.74),
        |col| {
            if let Some(id) = selected_id {
                if let Some(u) = loaded.units.get(id) {
                    if let Some(sf_id) = u.sub_factions.first() {
                        if let Some((f_def, sf_def)) = resolve_country_def_by_id(loaded, sf_id) {
                            col.spawn(Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(6.0),
                                ..default()
                            })
                            .with_children(|hdr| {
                                spawn_country_flag_badge(
                                    hdr,
                                    sf_def.resolved_flag_colors(f_def.color),
                                );
                                spawn_body_text(
                                    hdr,
                                    format!(
                                        "Country Exclusive: {} ({})",
                                        sf_def.name,
                                        sf_def.bonus_summary()
                                    ),
                                    Color::srgb(0.98, 0.88, 0.46),
                                );
                            });
                        }
                    }
                    let loco = u.locomotor_spec();
                    let costs = u
                        .effective_resource_costs()
                        .iter()
                        .map(|(k, v)| format!("{k}: {v}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let mut traits_vec = Vec::new();
                    if let Some(ref b) = u.builder_trait {
                        traits_vec.push(format!(
                            "Builder ({} structures)",
                            b.buildable_buildings.len()
                        ));
                    }
                    if let Some(ref g) = u.gatherer_trait {
                        traits_vec.push(format!("Gatherer (Cap: {})", g.carry_capacity));
                    }
                    if let Some(ref sq) = u.squad_trait {
                        traits_vec.push(format!("Squad ({} members)", sq.member_count));
                    }
                    if u.plants_c4 {
                        traits_vec.push("Demolition / Timed Explosives".to_string());
                    }
                    if u.mobile_refinery {
                        traits_vec.push("Mobile Slave Refinery (5x Ore Slaves)".to_string());
                    }
                    if traits_vec.is_empty() {
                        traits_vec.push("Standard Combatant".to_string());
                    }
                    let excl = exclusivity_suffix(loaded, &u.sub_factions);
                    spawn_body_text(
                        col,
                        format!(
                            "{}{excl}\nID: {}  |  Role: {:?}\nHealth: {:.0}  |  Armor: {:?}\nLocomotor: {:?} (Speed {:.1}, Turn {:.0} deg/s)\nCosts: [{}]\nPrimary Weapon: {}\nTraits: {}\nPrerequisites: {}",
                            u.name,
                            u.id,
                            u.role,
                            u.health,
                            u.armor,
                            loco.kind,
                            u.speed,
                            loco.turn_rate_deg_per_sec,
                            costs,
                            u.primary_weapon.as_deref().unwrap_or("Unarmed"),
                            traits_vec.join(", "),
                            if u.prerequisites.is_empty() {
                                "Special / Non-Buildable".to_string()
                            } else {
                                u.prerequisites.join(" + ")
                            }
                        ),
                        Color::srgb(0.88, 0.94, 0.98),
                    );
                } else if let Some(b) = loaded.buildings.get(id) {
                    if let Some(sf_id) = b.sub_factions.first() {
                        if let Some((f_def, sf_def)) = resolve_country_def_by_id(loaded, sf_id) {
                            col.spawn(Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(6.0),
                                ..default()
                            })
                            .with_children(|hdr| {
                                spawn_country_flag_badge(
                                    hdr,
                                    sf_def.resolved_flag_colors(f_def.color),
                                );
                                spawn_body_text(
                                    hdr,
                                    format!(
                                        "Country Exclusive Defense: {} ({})",
                                        sf_def.name,
                                        sf_def.bonus_summary()
                                    ),
                                    Color::srgb(0.98, 0.88, 0.46),
                                );
                            });
                        }
                    }
                    let costs = b
                        .effective_resource_costs()
                        .iter()
                        .map(|(k, v)| format!("{k}: {v}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let trickle = b
                        .effective_resource_trickle()
                        .iter()
                        .map(|(k, v)| format!("{k}: +{v:.1}/s"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let excl = exclusivity_suffix(loaded, &b.sub_factions);
                    spawn_body_text(
                        col,
                        format!(
                            "{}{excl}\nID: {}  |  Role: {:?}\nHealth: {:.0}  |  Footprint: {}x{}\nPower Delta: {:+}\nCosts: [{}]\nTrickle: {}\nPrimary Weapon: {}\nPrerequisites: {}",
                            b.name,
                            b.id,
                            b.role,
                            b.health,
                            b.footprint[0],
                            b.footprint[1],
                            b.power_delta,
                            costs,
                            if trickle.is_empty() { "None".to_string() } else { trickle },
                            b.primary_weapon.as_deref().unwrap_or("None"),
                            if b.prerequisites.is_empty() {
                                "Construction Yard".to_string()
                            } else {
                                b.prerequisites.join(" + ")
                            }
                        ),
                        Color::srgb(0.88, 0.94, 0.98),
                    );
                }
            }
        },
    );
}
