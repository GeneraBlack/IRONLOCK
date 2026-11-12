use crate::input::UiAssetCache;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_icon_styled_button};
use crate::ui::widgets::{UiScrollContainer, UiScrollContent};
use crate::ui::SidebarAction;
use crate::AssetRootResource;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::BuildingRole;
use rts_simulation::world::{EntityKind, SimWorld};
use std::collections::BTreeSet;

type TechNodeCard = (String, String, bool, bool, Option<Handle<Image>>);

fn compute_item_tier(loaded: &LoadedMod, prereqs: &[String], role: Option<BuildingRole>) -> usize {
    if role == Some(BuildingRole::ConstructionYard) {
        return 0;
    }
    if prereqs.is_empty() {
        return 1;
    }
    let mut max_depth = 1usize;
    for req in prereqs {
        if let Some(b) = loaded.buildings.get(req) {
            if b.role == BuildingRole::ConstructionYard {
                max_depth = max_depth.max(1);
            } else if matches!(
                b.role,
                BuildingRole::PowerPlant | BuildingRole::Barracks | BuildingRole::Refinery
            ) {
                max_depth = max_depth.max(2);
            } else if b.role == BuildingRole::WarFactory {
                max_depth = max_depth.max(3);
            } else {
                max_depth = max_depth.max(4);
            }
        }
    }
    max_depth.min(4)
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_tech_tree_window(
    parent: &mut ChildBuilder,
    asset_server: Option<&AssetServer>,
    _asset_root: &AssetRootResource,
    ui_cache: &mut UiAssetCache,
    loaded: &LoadedMod,
    world: &SimWorld,
    pid: u8,
    faction_id: &str,
    inspected_id: Option<&str>,
) {
    let owned_buildings: BTreeSet<String> = world
        .entities
        .values()
        .filter(|e| e.player_id == pid && e.health > 0.0 && e.construction_progress >= 1.0)
        .filter_map(|e| match &e.kind {
            EntityKind::Building { building_id } => Some(building_id.clone()),
            _ => None,
        })
        .collect();

    let tier_titles = [
        "TIER 0 — COMMAND & CONSTRUCTION YARD",
        "TIER 1 — POWER & GARRISON INFANTRY",
        "TIER 2 — RESOURCE REFINING & WAR FACTORY",
        "TIER 3 — RADAR, AIRFIELD & MECHANIZED CORPS",
        "TIER 4 — BATTLE LAB, ELITE TECH & SUPERWEAPONS",
    ];

    let mut tiers: [Vec<TechNodeCard>; 5] = std::array::from_fn(|_| Vec::new());

    let has_factions = !loaded.factions.is_empty();
    let any_bldg_prereqs = loaded
        .buildings
        .values()
        .any(|x| !x.prerequisites.is_empty());

    for b in loaded.buildings.values() {
        let b_fac = b.faction.trim();
        let faction_match = if has_factions {
            b_fac.eq_ignore_ascii_case(faction_id)
        } else {
            b.is_available_to_faction(faction_id)
        };
        let in_tech_tree = b.role == BuildingRole::ConstructionYard
            || !b.prerequisites.is_empty()
            || !any_bldg_prereqs;
        if !faction_match
            || !in_tech_tree
            || (b.cost == 0 && b.role != BuildingRole::ConstructionYard)
        {
            continue;
        }
        let tier = compute_item_tier(loaded, &b.prerequisites, Some(b.role));
        let is_owned = owned_buildings.contains(&b.id);
        let reqs_met = b.prerequisites.iter().all(|r| owned_buildings.contains(r));
        let cameo = ui_cache.load_entity_cameo(loaded, asset_server, &b.id);
        tiers[tier].push((
            b.id.clone(),
            format!("[BLD] {}", b.name),
            is_owned,
            reqs_met,
            cameo,
        ));
    }

    let any_unit_prereqs = loaded.units.values().any(|x| !x.prerequisites.is_empty());
    for u in loaded.units.values() {
        let u_fac = u.faction.trim();
        let faction_match = if has_factions {
            u_fac.eq_ignore_ascii_case(faction_id)
        } else {
            u.is_available_to_faction(faction_id)
        };
        let in_tech_tree = !u.prerequisites.is_empty() || !any_unit_prereqs;
        if !faction_match || !in_tech_tree || u.cost == 0 {
            continue;
        }
        let tier = compute_item_tier(loaded, &u.prerequisites, None);
        let reqs_met = u.prerequisites.iter().all(|r| owned_buildings.contains(r));
        let cameo = ui_cache.load_entity_cameo(loaded, asset_server, &u.id);
        tiers[tier].push((u.id.clone(), u.name.clone(), false, reqs_met, cameo));
    }

    // Legend row
    parent.spawn((
        Text::new("Legend: [BUILT] Green  |  [UNLOCKED] Emerald  |  [LOCKED] Grey — Click any node to inspect stats"),
        TextFont {
            font_size: 11.0,
            ..default()
        },
        TextColor(Color::srgb(0.76, 0.86, 0.96)),
    ));

    // Scrollable Tiered Dependency Diagram
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(320.0),
                overflow: Overflow::clip_y(),
                border: UiRect::all(Val::Px(1.0)),
                padding: UiRect::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.05, 0.09, 0.94)),
            BorderColor(Color::srgb(0.24, 0.36, 0.52)),
            UiScrollContainer::new(520.0),
        ))
        .with_children(|viewport| {
            viewport
                .spawn((
                    UiScrollContent,
                    Node {
                        width: Val::Percent(100.0),
                        position_type: PositionType::Relative,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        ..default()
                    },
                ))
                .with_children(|scroll_col| {
                    for (t_idx, items) in tiers.iter().enumerate() {
                        if items.is_empty() {
                            continue;
                        }
                        scroll_col.spawn((
                            Text::new(tier_titles[t_idx]),
                            TextFont {
                                font_size: 11.5,
                                ..default()
                            },
                            TextColor(Color::srgb(0.98, 0.84, 0.42)),
                        ));
                        scroll_col
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Row,
                                flex_wrap: FlexWrap::Wrap,
                                column_gap: Val::Px(5.0),
                                row_gap: Val::Px(5.0),
                                ..default()
                            })
                            .with_children(|row| {
                                for (id, name, is_owned, reqs_met, cameo) in items {
                                    let (badge, col) = if *is_owned {
                                        ("[BUILT]", Color::srgb(0.16, 0.46, 0.24))
                                    } else if *reqs_met {
                                        ("[READY]", Color::srgb(0.18, 0.36, 0.32))
                                    } else {
                                        ("[LOCK]", Color::srgb(0.22, 0.24, 0.28))
                                    };
                                    spawn_icon_styled_button(
                                        row,
                                        &format!("{badge} {name}"),
                                        cameo.clone(),
                                        SidebarAction::InspectTechNode(id.clone()),
                                        col,
                                        Val::Px(228.0),
                                    );
                                }
                            });
                    }
                });
        });

    // Click-to-Inspect Detail Strip
    let detail_text = if let Some(tid) = inspected_id {
        if let Some(u) = loaded.units.get(tid) {
            let costs = u
                .effective_resource_costs()
                .iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect::<Vec<_>>()
                .join(", ");
            let reqs = if u.prerequisites.is_empty() {
                "None".to_string()
            } else {
                u.prerequisites.join(" + ")
            };
            format!(
                "{} [Unit: {}] — HP: {:.0} | Armor: {:?} | Speed: {:.1} ({:?}) | Costs: [{}] | Requires: {}",
                u.name,
                u.id,
                u.health,
                u.armor,
                u.speed,
                u.locomotor_spec().kind,
                costs,
                reqs
            )
        } else if let Some(b) = loaded.buildings.get(tid) {
            let costs = b
                .effective_resource_costs()
                .iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect::<Vec<_>>()
                .join(", ");
            let reqs = if b.prerequisites.is_empty() {
                "Construction Yard".to_string()
            } else {
                b.prerequisites.join(" + ")
            };
            format!(
                "{} [Building: {}] — HP: {:.0} | Power: {:+} | Footprint: {}x{} | Costs: [{}] | Requires: {}",
                b.name, b.id, b.health, b.power_delta, b.footprint[0], b.footprint[1], costs, reqs
            )
        } else {
            "Select a structure or unit node above to inspect prerequisites and combat specs."
                .to_string()
        }
    } else {
        "Click any building or unit card above to inspect prerequisites, multi-resource costs, and combat attributes."
            .to_string()
    };

    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.07, 0.11, 0.18, 0.96)),
            BorderColor(Color::srgb(0.35, 0.52, 0.74)),
        ))
        .with_children(|info| {
            info.spawn((
                Text::new(sanitize_ui_text(&detail_text)),
                TextFont {
                    font_size: 11.5,
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.96, 1.0)),
            ));
        });
}
