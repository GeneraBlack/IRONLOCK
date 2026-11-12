use crate::input::{ClientInteractionState, UiInputBlocker};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, SystemCursorIcon};
use bevy::winit::cursor::CursorIcon;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::UnitRole;
use rts_simulation::world::{EntityKind, SimWorld};

/// Context-sensitive RTS cursor states driven by active command mode and hovered battlefield target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RtsCursorKind {
    #[default]
    Normal,
    Select,
    Move,
    Attack,
    Deploy,
    Repair,
    Sell,
    EnterGarrison,
    Blocked,
}

impl RtsCursorKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Select => "SELECT",
            Self::Move => "MOVE",
            Self::Attack => "ATTACK",
            Self::Deploy => "DEPLOY / CONSTRUCT",
            Self::Repair => "REPAIR",
            Self::Sell => "SELL",
            Self::EnterGarrison => "ENTER / GARRISON",
            Self::Blocked => "BLOCKED",
        }
    }

    pub fn system_icon(self) -> SystemCursorIcon {
        match self {
            Self::Normal => SystemCursorIcon::Default,
            Self::Select => SystemCursorIcon::Pointer,
            Self::Move => SystemCursorIcon::Crosshair,
            Self::Attack => SystemCursorIcon::Cell,
            Self::Deploy => SystemCursorIcon::Copy,
            Self::Repair => SystemCursorIcon::Alias,
            Self::Sell => SystemCursorIcon::NoDrop,
            Self::EnterGarrison => SystemCursorIcon::Grab,
            Self::Blocked => SystemCursorIcon::NotAllowed,
        }
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct RtsCursorState {
    pub current: RtsCursorKind,
    pub last_applied: Option<RtsCursorKind>,
}

/// Pure, unit-testable resolver that determines the active RTS cursor from current interaction state,
/// hovered entity, and hovered tile.
pub fn resolve_rts_cursor(
    interaction: &ClientInteractionState,
    world: &SimWorld,
    loaded: &LoadedMod,
    pointer_blocked_by_ui: bool,
) -> RtsCursorKind {
    if interaction.menu_open || interaction.studio_open || pointer_blocked_by_ui {
        return RtsCursorKind::Normal;
    }

    let local_pid = interaction.local_player_id;
    let hovered_ent = interaction
        .hovered_entity_id
        .and_then(|id| world.entities.get(&id))
        .filter(|e| e.health > 0.0 && e.transported_by.is_none());

    // 1. Explicit Repair / Sell cursor modes
    if interaction.repair_cursor_mode {
        if let Some(ent) = hovered_ent {
            if ent.player_id == local_pid && matches!(ent.kind, EntityKind::Building { .. }) {
                return if ent.health < ent.max_health {
                    RtsCursorKind::Repair
                } else {
                    RtsCursorKind::Blocked
                };
            }
            return RtsCursorKind::Blocked;
        }
        return RtsCursorKind::Repair;
    }

    if interaction.sell_cursor_mode {
        if let Some(ent) = hovered_ent {
            return if ent.player_id == local_pid && matches!(ent.kind, EntityKind::Building { .. })
            {
                RtsCursorKind::Sell
            } else {
                RtsCursorKind::Blocked
            };
        }
        return RtsCursorKind::Sell;
    }

    // 2. Building / Worker construction placement preview
    let active_place_id = interaction.placing_building_id.as_deref().or_else(|| {
        interaction
            .worker_placing_building
            .as_ref()
            .map(|(_, b)| b.as_str())
    });
    if let Some(building_id) = active_place_id {
        let tile = interaction.hovered_tile;
        if !world.grid.in_bounds(tile) || world.grid.blocked_tiles.contains(&tile) {
            return RtsCursorKind::Blocked;
        }
        if let Some(bdef) = loaded.buildings.get(building_id) {
            let w = bdef.footprint[0].max(1) as i32;
            let h = bdef.footprint[1].max(1) as i32;
            for dx in 0..w {
                for dz in 0..h {
                    let check = [tile[0] + dx, tile[1] + dz];
                    if !world.grid.in_bounds(check) || world.grid.blocked_tiles.contains(&check) {
                        return RtsCursorKind::Blocked;
                    }
                }
            }
        }
        return RtsCursorKind::Deploy;
    }

    // 3. Explicit offensive targeting modes
    if interaction.attack_move_mode
        || interaction.targeted_ability_mode.is_some()
        || interaction.active_support_power_id.is_some()
    {
        return RtsCursorKind::Attack;
    }

    // Inspect current owned selection capabilities
    let mut has_owned_mobile = false;
    let mut has_owned_combat = false;
    let mut has_owned_infantry = false;
    let mut has_owned_engineer = false;
    let mut single_deployable_id: Option<u64> = None;

    for id in &interaction.selected_ids {
        let Some(ent) = world.entities.get(id) else {
            continue;
        };
        if ent.player_id != local_pid || ent.health <= 0.0 || ent.transported_by.is_some() {
            continue;
        }
        match &ent.kind {
            EntityKind::Unit { unit_id } => {
                has_owned_mobile = true;
                if let Some(udef) = loaded.units.get(unit_id) {
                    if udef.primary_weapon.is_some() || udef.secondary_weapon.is_some() {
                        has_owned_combat = true;
                    }
                    if udef.role == UnitRole::Infantry || udef.squad_trait.is_some() {
                        has_owned_infantry = true;
                    }
                    if udef.role == UnitRole::Engineer {
                        has_owned_engineer = true;
                        has_owned_infantry = true;
                    }
                    if interaction.selected_ids.len() == 1
                        && (udef.transforms_into_building.is_some()
                            || udef.builder_trait.is_some()
                                && udef.transforms_into_building.is_some())
                    {
                        single_deployable_id = Some(ent.id);
                    }
                }
            }
            EntityKind::Building { building_id } => {
                if let Some(bdef) = loaded.buildings.get(building_id) {
                    if bdef.primary_weapon.is_some() {
                        has_owned_combat = true;
                    }
                }
            }
        }
    }

    // 4. Hovered battlefield entity context
    if let Some(target) = hovered_ent {
        if Some(target.id) == single_deployable_id {
            return RtsCursorKind::Deploy;
        }

        let target_container_cap = match &target.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| u.container.capacity)
                .unwrap_or(0),
            EntityKind::Building { building_id } => loaded
                .buildings
                .get(building_id)
                .map(|b| b.container.capacity)
                .unwrap_or(0),
        };
        let has_container_room = (target.passenger_ids.len() as u32) < target_container_cap;

        if target.player_id == local_pid {
            if has_owned_infantry
                && has_container_room
                && !interaction.selected_ids.contains(&target.id)
            {
                return RtsCursorKind::EnterGarrison;
            }
            if has_owned_engineer
                && matches!(target.kind, EntityKind::Building { .. })
                && target.health < target.max_health
            {
                return RtsCursorKind::Repair;
            }
            return RtsCursorKind::Select;
        }

        if target.player_id == 0 {
            if has_owned_infantry && has_container_room {
                return RtsCursorKind::EnterGarrison;
            }
            if has_owned_engineer && matches!(target.kind, EntityKind::Building { .. }) {
                return RtsCursorKind::EnterGarrison;
            }
            return if has_owned_combat {
                RtsCursorKind::Attack
            } else {
                RtsCursorKind::Select
            };
        }

        if world.are_hostile(local_pid, target.player_id) {
            return if has_owned_combat {
                RtsCursorKind::Attack
            } else {
                RtsCursorKind::Select
            };
        }

        return RtsCursorKind::Select;
    }

    // 5. Hovered terrain context when mobile units are selected
    if has_owned_mobile {
        let tile = interaction.hovered_tile;
        if !world.grid.in_bounds(tile) {
            return RtsCursorKind::Blocked;
        }
        if world.grid.blocked_tiles.contains(&tile) && !world.grid.ore_tiles.contains_key(&tile) {
            return RtsCursorKind::Blocked;
        }
        return RtsCursorKind::Move;
    }

    RtsCursorKind::Normal
}

pub fn update_rts_cursor(
    mut commands: Commands,
    window_q: Query<(Entity, &Window), With<PrimaryWindow>>,
    interaction: Res<ClientInteractionState>,
    sim_res: Res<SimWorldResource>,
    mod_res: Res<ModResource>,
    ui_blocker: Option<Res<UiInputBlocker>>,
    mut cursor_state: ResMut<RtsCursorState>,
) {
    let Ok((win_entity, window)) = window_q.get_single() else {
        return;
    };
    let cursor_pos = window.cursor_position();
    let pointer_blocked = ui_blocker
        .as_deref()
        .map(|b| b.blocks_world_pointer(interaction.studio_open, interaction.menu_open, cursor_pos))
        .unwrap_or(false);

    let desired = resolve_rts_cursor(&interaction, &sim_res.0, &mod_res.0, pointer_blocked);
    cursor_state.current = desired;

    // Only touch Window's CursorIcon when not in Studio mode (so bevy_egui can manage resize cursors in Studio)
    if interaction.studio_open {
        cursor_state.last_applied = None;
        return;
    }

    if cursor_state.last_applied != Some(desired) {
        commands
            .entity(win_entity)
            .insert(CursorIcon::System(desired.system_icon()));
        cursor_state.last_applied = Some(desired);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_rts_cursor_resolves_all_tactical_states() {
        let loaded = LoadedMod::load_from_dir(LoadedMod::resolve_mod_dir("rust-ra2"))
            .expect("load rust-ra2");
        let mut world = SimWorld::from_mod(&loaded);
        let mut interaction = ClientInteractionState {
            local_player_id: 1,
            menu_open: false,
            studio_open: false,
            hovered_tile: [8, 8],
            ..Default::default()
        };
        world.grid.blocked_tiles.clear();

        // Idle -> Normal
        assert_eq!(
            resolve_rts_cursor(&interaction, &world, &loaded, false),
            RtsCursorKind::Normal
        );

        // Repair & Sell modes
        interaction.repair_cursor_mode = true;
        assert_eq!(
            resolve_rts_cursor(&interaction, &world, &loaded, false),
            RtsCursorKind::Repair
        );
        interaction.repair_cursor_mode = false;
        interaction.sell_cursor_mode = true;
        assert_eq!(
            resolve_rts_cursor(&interaction, &world, &loaded, false),
            RtsCursorKind::Sell
        );
        interaction.sell_cursor_mode = false;

        // Placement valid -> Deploy, blocked -> Blocked
        interaction.placing_building_id = Some("allied_power_plant".to_string());
        assert_eq!(
            resolve_rts_cursor(&interaction, &world, &loaded, false),
            RtsCursorKind::Deploy
        );
        world.grid.blocked_tiles.insert([8, 8]);
        assert_eq!(
            resolve_rts_cursor(&interaction, &world, &loaded, false),
            RtsCursorKind::Blocked
        );
        world.grid.blocked_tiles.remove(&[8, 8]);
        interaction.placing_building_id = None;

        // Spawn owned combat tank, owned infantry, neutral garrison, and enemy tank
        world.grid.water_tiles.clear();
        let inf_uid = loaded
            .units
            .values()
            .find(|u| u.role == UnitRole::Infantry)
            .expect("infantry unit")
            .id
            .clone();
        let tank_id = world
            .spawn_unit(1, "grizzly_tank", [6, 6], &loaded)
            .expect("spawn tank");
        let inf_id = world
            .spawn_unit(1, &inf_uid, [7, 6], &loaded)
            .expect("spawn inf");
        let enemy_id = world
            .spawn_unit(2, "rhino_tank", [10, 10], &loaded)
            .expect("spawn enemy");

        // Select combat tank -> hovering ground = Move, hovering enemy = Attack, hovering ally = Select
        interaction.selected_ids.insert(tank_id);
        assert_eq!(
            resolve_rts_cursor(&interaction, &world, &loaded, false),
            RtsCursorKind::Move
        );
        interaction.hovered_entity_id = Some(enemy_id);
        assert_eq!(
            resolve_rts_cursor(&interaction, &world, &loaded, false),
            RtsCursorKind::Attack
        );
        interaction.hovered_entity_id = Some(inf_id);
        assert_eq!(
            resolve_rts_cursor(&interaction, &world, &loaded, false),
            RtsCursorKind::Select
        );

        // Garrisonable building -> EnterGarrison when infantry selected
        if let Some(bunker_def) = loaded.buildings.values().find(|b| b.container.capacity > 0) {
            let bunker_id = world
                .spawn_building(0, &bunker_def.id, [12, 12], &loaded, false)
                .expect("spawn garrison");
            interaction.selected_ids.clear();
            interaction.selected_ids.insert(inf_id);
            interaction.hovered_entity_id = Some(bunker_id);
            assert_eq!(
                resolve_rts_cursor(&interaction, &world, &loaded, false),
                RtsCursorKind::EnterGarrison
            );
        }
    }
}
