use super::{spawn_body_text, spawn_screen_card};
use crate::input::{ClientInteractionState, UiAssetCache};
use crate::ui::studio_layout::{spawn_icon_styled_button, spawn_styled_button};
use crate::ui::widgets::{UiScrollContainer, UiScrollContent};
use crate::ui::SidebarAction;
use crate::AssetRootResource;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;

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

    // Column 1: Faction & Category Selector
    spawn_screen_card(
        parent,
        26.0,
        "1. FACTION & CATEGORY",
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
            }
        },
    );

    // Column 2: Scrollable Field Catalog
    let entries: Vec<(String, String, Option<Handle<Image>>)> =
        if interaction.encyclopedia_show_buildings {
            loaded
                .buildings
                .values()
                .filter(|b| b.is_available_to_faction(&active_fac))
                .map(|b| {
                    let c = ui_cache.load_entity_cameo(loaded, asset_server, &b.id);
                    (b.id.clone(), format!("{} (${})", b.name, b.cost), c)
                })
                .collect()
        } else {
            loaded
                .units
                .values()
                .filter(|u| u.is_available_to_faction(&active_fac))
                .map(|u| {
                    let c = ui_cache.load_entity_cameo(loaded, asset_server, &u.id);
                    (u.id.clone(), format!("{} (${})", u.name, u.cost), c)
                })
                .collect()
        };

    spawn_screen_card(
        parent,
        36.0,
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

    // Column 3: Detailed Combat, Locomotor & Trait Dossier
    let selected_id = interaction
        .selected_encyclopedia_entry
        .as_deref()
        .or_else(|| entries.first().map(|(id, _, _)| id.as_str()));

    spawn_screen_card(
        parent,
        38.0,
        "3. TECHNICAL DOSSIER",
        Color::srgb(0.58, 0.95, 0.74),
        |col| {
            if let Some(id) = selected_id {
                if let Some(u) = loaded.units.get(id) {
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
                    if traits_vec.is_empty() {
                        traits_vec.push("Standard Combatant".to_string());
                    }
                    spawn_body_text(
                        col,
                        format!(
                            "{}\nID: {}  |  Role: {:?}\nHealth: {:.0}  |  Armor: {:?}\nLocomotor: {:?} (Speed {:.1}, Turn {:.0} deg/s)\nCosts: [{}]\nPrimary Weapon: {}\nTraits: {}\nPrerequisites: {}",
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
                                "None".to_string()
                            } else {
                                u.prerequisites.join(" + ")
                            }
                        ),
                        Color::srgb(0.88, 0.94, 0.98),
                    );
                } else if let Some(b) = loaded.buildings.get(id) {
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
                    spawn_body_text(
                        col,
                        format!(
                            "{}\nID: {}  |  Role: {:?}\nHealth: {:.0}  |  Footprint: {}x{}\nPower Delta: {:+}\nCosts: [{}]\nTrickle: {}\nPrimary Weapon: {}\nPrerequisites: {}",
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
