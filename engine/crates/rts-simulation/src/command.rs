use serde::{Deserialize, Serialize};

pub use crate::world::EnvironmentPreset;

/// Player slot identifier (`0` = neutral, `1..=8` = active players).
pub type PlayerId = u8;

/// Tactical engagement stance for combat units.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitStance {
    /// Actively engages and pursues enemies within vision/leash range.
    #[default]
    Aggressive,
    /// Engages enemies and pursues only a short distance from guard origin before returning.
    Defensive,
    /// Fires at enemies within weapon range without moving from current position.
    HoldGround,
    /// Never auto-acquires targets; only fires when given an explicit `AttackTarget` order.
    HoldFire,
}

impl UnitStance {
    pub fn next(self) -> Self {
        match self {
            Self::Aggressive => Self::Defensive,
            Self::Defensive => Self::HoldGround,
            Self::HoldGround => Self::HoldFire,
            Self::HoldFire => Self::Aggressive,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Aggressive => "AGGRESSIVE",
            Self::Defensive => "DEFENSIVE",
            Self::HoldGround => "HOLD GROUND",
            Self::HoldFire => "HOLD FIRE",
        }
    }
}

/// Tactical squad formation shape for group movement orders (`[J]`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormationKind {
    #[default]
    Box,
    Line,
    Wedge,
    Column,
}

impl FormationKind {
    pub fn next(self) -> Self {
        match self {
            Self::Box => Self::Line,
            Self::Line => Self::Wedge,
            Self::Wedge => Self::Column,
            Self::Column => Self::Box,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Box => "BOX",
            Self::Line => "LINE",
            Self::Wedge => "WEDGE",
            Self::Column => "COLUMN",
        }
    }
}

/// Area-of-effect tactical smart command kind (`Task 239-Sim`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AreaCommandKind {
    AreaRepair,
    AreaHarvest,
    AreaAttack,
    AreaGuard,
}

/// Individual tactical order stored in `SimEntity::command_queue` for universal `Shift`-queuing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QueuedOrder {
    Move {
        target_pos: [f32; 3],
        #[serde(default)]
        facing_radians: Option<f32>,
    },
    AttackMove {
        target_pos: [f32; 3],
        #[serde(default)]
        facing_radians: Option<f32>,
    },
    AttackTarget {
        target_entity_id: u64,
    },
    AttackGround {
        target_pos: [f32; 3],
    },
    Patrol {
        target_pos: [f32; 3],
    },
    Guard {
        guarded_entity_id: u64,
    },
    Harvest {
        ore_tile: [i32; 2],
    },
    WorkerConstruct {
        building_id: String,
        tile_pos: [i32; 2],
    },
    LoadIntoCarrier {
        carrier_id: u64,
    },
    Unload {
        #[serde(default)]
        drop_pos: Option<[f32; 3]>,
    },
    EngineerRepairOrCapture {
        target_entity_id: u64,
    },
    CastAbility {
        #[serde(default)]
        ability_index: Option<usize>,
        #[serde(default)]
        target_pos: Option<[f32; 3]>,
        #[serde(default)]
        target_entity_id: Option<u64>,
    },
}

impl QueuedOrder {
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Move {
                target_pos,
                facing_radians,
            }
            | Self::AttackMove {
                target_pos,
                facing_radians,
            } => {
                is_valid_vec3(*target_pos)
                    && facing_radians.is_none_or(|f| f.is_finite() && f.abs() <= 1.0e4)
            }
            Self::AttackGround { target_pos } | Self::Patrol { target_pos } => {
                is_valid_vec3(*target_pos)
            }
            Self::Harvest { ore_tile } => is_valid_tile(*ore_tile),
            Self::WorkerConstruct {
                building_id,
                tile_pos,
            } => building_id.len() <= 256 && is_valid_tile(*tile_pos),
            Self::Unload { drop_pos } => drop_pos.is_none_or(is_valid_vec3),
            Self::CastAbility { target_pos, .. } => target_pos.is_none_or(is_valid_vec3),
            Self::AttackTarget { .. }
            | Self::Guard { .. }
            | Self::LoadIntoCarrier { .. }
            | Self::EngineerRepairOrCapture { .. } => true,
        }
    }
}

/// Deterministic player command that can be serialized across the network
/// for lockstep/rollback multiplayer or replay files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GameCommand {
    MoveUnits {
        player_id: u8,
        entity_ids: Vec<u64>,
        target_pos: [f32; 3],
    },
    QueueMoveUnits {
        player_id: u8,
        entity_ids: Vec<u64>,
        target_pos: [f32; 3],
    },
    AttackMove {
        player_id: u8,
        entity_ids: Vec<u64>,
        target_pos: [f32; 3],
    },
    PatrolUnits {
        player_id: u8,
        entity_ids: Vec<u64>,
        target_pos: [f32; 3],
    },
    GuardEntity {
        player_id: u8,
        entity_ids: Vec<u64>,
        guarded_entity_id: u64,
    },
    SetUnitStance {
        player_id: u8,
        entity_ids: Vec<u64>,
        stance: UnitStance,
    },
    StopUnits {
        player_id: u8,
        entity_ids: Vec<u64>,
    },
    ScatterUnits {
        player_id: u8,
        entity_ids: Vec<u64>,
    },
    SetRallyPoint {
        player_id: u8,
        building_ids: Vec<u64>,
        rally_pos: [f32; 3],
    },
    SellBuilding {
        player_id: u8,
        building_id: u64,
    },
    ToggleRepairBuilding {
        player_id: u8,
        building_id: u64,
    },
    SetPrimaryBuilding {
        player_id: u8,
        building_id: u64,
    },
    PingLocation {
        player_id: u8,
        position: [f32; 3],
    },
    AttackTarget {
        player_id: u8,
        attacker_ids: Vec<u64>,
        target_entity_id: u64,
    },
    AttackGround {
        player_id: u8,
        attacker_ids: Vec<u64>,
        target_pos: [f32; 3],
    },
    QueueBuilding {
        player_id: u8,
        building_id: String,
    },
    CancelBuildingQueue {
        player_id: u8,
    },
    PlaceBuilding {
        player_id: u8,
        building_id: String,
        tile_pos: [i32; 2],
    },
    PlaceWallLine {
        player_id: u8,
        building_id: String,
        start_tile: [i32; 2],
        end_tile: [i32; 2],
    },
    ProduceUnit {
        player_id: u8,
        producer_entity_id: Option<u64>,
        unit_id: String,
    },
    CancelProduction {
        player_id: u8,
        unit_id: String,
    },
    ResearchUpgrade {
        player_id: u8,
        upgrade_id: String,
    },
    LoadIntoCarrier {
        player_id: u8,
        passenger_ids: Vec<u64>,
        carrier_id: u64,
    },
    UnloadCarrier {
        player_id: u8,
        carrier_ids: Vec<u64>,
    },
    ActivateAbility {
        player_id: u8,
        entity_ids: Vec<u64>,
    },
    CastTargetedAbility {
        player_id: u8,
        caster_ids: Vec<u64>,
        target_pos: [f32; 3],
        target_entity_id: Option<u64>,
    },
    SetAlliance {
        player_id: u8,
        target_player_id: u8,
        allied: bool,
    },
    TransferCredits {
        from_player_id: u8,
        to_player_id: u8,
        amount: i32,
    },
    TransferUnits {
        from_player_id: u8,
        to_player_id: u8,
        unit_ids: Vec<u64>,
    },
    PlaceMine {
        player_id: u8,
        unit_ids: Vec<u64>,
        target_tile: [i32; 2],
    },
    DockAtServiceDepot {
        player_id: u8,
        unit_ids: Vec<u64>,
        depot_entity_id: u64,
    },
    PlaceBuildingRotated {
        player_id: u8,
        building_id: String,
        tile_pos: [i32; 2],
        rotated: bool,
    },
    CastSupportPower {
        player_id: u8,
        power_id: String,
        source_pos: Option<[f32; 3]>,
        target_pos: [f32; 3],
    },
    ReturnToAirfield {
        player_id: u8,
        unit_ids: Vec<u64>,
        airfield_entity_id: Option<u64>,
    },
    SetFormation {
        player_id: u8,
        formation: FormationKind,
    },
    SetFormationLocked {
        player_id: u8,
        locked: bool,
    },
    InfiltrateBuilding {
        player_id: u8,
        spy_ids: Vec<u64>,
        target_building_id: u64,
    },
    ReverseMove {
        player_id: u8,
        entity_ids: Vec<u64>,
        target_pos: [f32; 3],
    },
    SpawnSandboxUnit {
        player_id: u8,
        unit_id: String,
        tile_pos: [i32; 2],
    },
    UpgradeBuilding {
        player_id: u8,
        building_id: u64,
    },
    ToggleBuildingPower {
        player_id: u8,
        building_ids: Vec<u64>,
    },
    DisguiseUnits {
        player_id: u8,
        spy_ids: Vec<u64>,
        target_entity_id: u64,
    },
    QueueRallyPoint {
        player_id: u8,
        building_ids: Vec<u64>,
        rally_pos: [f32; 3],
    },
    TogglePauseProduction {
        player_id: u8,
        building_id: Option<u64>,
    },
    SetEnvironmentPreset {
        player_id: PlayerId,
        preset: EnvironmentPreset,
    },
    ActivateAbilityIndex {
        player_id: PlayerId,
        entity_ids: Vec<u64>,
        ability_index: usize,
    },
    CastTargetedAbilityIndex {
        player_id: PlayerId,
        caster_ids: Vec<u64>,
        ability_index: usize,
        target_pos: [f32; 3],
        target_entity_id: Option<u64>,
    },
    OrderWorkerConstruct {
        player_id: u8,
        worker_ids: Vec<u64>,
        building_id: String,
        tile_pos: [i32; 2],
    },
    EnqueueOrder {
        player_id: u8,
        entity_ids: Vec<u64>,
        order: QueuedOrder,
    },
    MoveUnitsWithFormation {
        player_id: u8,
        entity_ids: Vec<u64>,
        target_pos: [f32; 3],
        facing_radians: f32,
        line_spacing: f32,
        attack_move: bool,
        append_queue: bool,
    },
    OrderCarryallPickup {
        player_id: u8,
        carryall_id: u64,
        vehicle_id: u64,
    },
    ToggleHoldProduction {
        player_id: u8,
        producer_entity_id: Option<u64>,
        item_id: String,
    },
    EnableAiTakeover {
        player_id: u8,
        difficulty: crate::ai::AiDifficulty,
    },
    UnlockCommanderNode {
        player_id: u8,
        node_id: String,
    },
    PatrolWaypoints {
        player_id: u8,
        entity_ids: Vec<u64>,
        waypoints: Vec<[f32; 3]>,
    },
    ToggleBurrow {
        player_id: u8,
        unit_ids: Vec<u64>,
    },
    UpgradeBuildingTo {
        player_id: u8,
        building_id: u64,
        target_def_id: String,
    },
    RetreatSquad {
        player_id: u8,
        squad_ids: Vec<u64>,
    },
    SetSquadAutoReinforce {
        player_id: u8,
        squad_ids: Vec<u64>,
        enabled: bool,
    },
    ReinforceSquadNow {
        player_id: u8,
        squad_ids: Vec<u64>,
    },
    UpgradeSquadWeapon {
        player_id: u8,
        squad_id: u64,
        upgrade_id: String,
    },
    ReclaimHumanSlot {
        player_id: u8,
    },
    MoveAlongCustomCurve {
        player_id: u8,
        entity_ids: Vec<u64>,
        curve_points: Vec<[f32; 3]>,
        #[serde(default)]
        queued: bool,
    },
    AreaCommand {
        player_id: u8,
        entity_ids: Vec<u64>,
        kind: AreaCommandKind,
        center: [f32; 3],
        radius: f32,
        #[serde(default)]
        queued: bool,
    },
    ToggleAbilityAutoCast {
        player_id: u8,
        entity_ids: Vec<u64>,
        ability_index: usize,
        #[serde(default)]
        enabled: Option<bool>,
    },
    ResearchUpgradeAt {
        player_id: u8,
        #[serde(default)]
        building_id: Option<u64>,
        upgrade_id: String,
    },
    CancelResearch {
        player_id: u8,
        #[serde(default)]
        building_id: Option<u64>,
        upgrade_id: String,
    },
    AdvanceEra {
        player_id: u8,
        era_id: String,
        #[serde(default)]
        building_id: Option<u64>,
    },
    LearnHeroSkill {
        player_id: u8,
        hero_id: u64,
        ability_id: String,
    },
    ReviveHero {
        player_id: u8,
        #[serde(default)]
        altar_entity_id: Option<u64>,
        #[serde(default)]
        hero_def_id: Option<String>,
    },
    PickupItem {
        player_id: u8,
        hero_id: u64,
        ground_item_id: u64,
    },
    UseInventoryItem {
        player_id: u8,
        hero_id: u64,
        slot_index: usize,
        #[serde(default)]
        target_pos: Option<[f32; 3]>,
        #[serde(default)]
        target_entity_id: Option<u64>,
    },
    DropInventoryItem {
        player_id: u8,
        hero_id: u64,
        slot_index: usize,
        #[serde(default)]
        drop_pos: Option<[f32; 3]>,
    },
}

#[inline]
fn is_valid_vec3(v: [f32; 3]) -> bool {
    v[0].is_finite()
        && v[1].is_finite()
        && v[2].is_finite()
        && v[0].abs() <= 1.0e6
        && v[1].abs() <= 1.0e6
        && v[2].abs() <= 1.0e6
}

#[inline]
fn is_valid_tile(t: [i32; 2]) -> bool {
    t[0] >= -16_384 && t[0] <= 16_384 && t[1] >= -16_384 && t[1] <= 16_384
}

impl GameCommand {
    /// Returns the issuing player's ID for network ownership validation.
    pub fn owner_player_id(&self) -> Option<PlayerId> {
        Some(match self {
            Self::MoveUnits { player_id, .. }
            | Self::QueueMoveUnits { player_id, .. }
            | Self::AttackMove { player_id, .. }
            | Self::PatrolUnits { player_id, .. }
            | Self::GuardEntity { player_id, .. }
            | Self::SetUnitStance { player_id, .. }
            | Self::StopUnits { player_id, .. }
            | Self::ScatterUnits { player_id, .. }
            | Self::SetRallyPoint { player_id, .. }
            | Self::SellBuilding { player_id, .. }
            | Self::ToggleRepairBuilding { player_id, .. }
            | Self::SetPrimaryBuilding { player_id, .. }
            | Self::PingLocation { player_id, .. }
            | Self::AttackTarget { player_id, .. }
            | Self::AttackGround { player_id, .. }
            | Self::QueueBuilding { player_id, .. }
            | Self::CancelBuildingQueue { player_id }
            | Self::PlaceBuilding { player_id, .. }
            | Self::PlaceWallLine { player_id, .. }
            | Self::ProduceUnit { player_id, .. }
            | Self::CancelProduction { player_id, .. }
            | Self::ResearchUpgrade { player_id, .. }
            | Self::LoadIntoCarrier { player_id, .. }
            | Self::UnloadCarrier { player_id, .. }
            | Self::ActivateAbility { player_id, .. }
            | Self::CastTargetedAbility { player_id, .. }
            | Self::SetAlliance { player_id, .. }
            | Self::PlaceMine { player_id, .. }
            | Self::DockAtServiceDepot { player_id, .. }
            | Self::PlaceBuildingRotated { player_id, .. }
            | Self::CastSupportPower { player_id, .. }
            | Self::ReturnToAirfield { player_id, .. }
            | Self::SetFormation { player_id, .. }
            | Self::SetFormationLocked { player_id, .. }
            | Self::InfiltrateBuilding { player_id, .. }
            | Self::ReverseMove { player_id, .. }
            | Self::SpawnSandboxUnit { player_id, .. }
            | Self::UpgradeBuilding { player_id, .. }
            | Self::ToggleBuildingPower { player_id, .. }
            | Self::DisguiseUnits { player_id, .. }
            | Self::QueueRallyPoint { player_id, .. }
            | Self::TogglePauseProduction { player_id, .. }
            | Self::SetEnvironmentPreset { player_id, .. }
            | Self::ActivateAbilityIndex { player_id, .. }
            | Self::CastTargetedAbilityIndex { player_id, .. }
            | Self::OrderWorkerConstruct { player_id, .. }
            | Self::EnqueueOrder { player_id, .. }
            | Self::MoveUnitsWithFormation { player_id, .. }
            | Self::OrderCarryallPickup { player_id, .. }
            | Self::ToggleHoldProduction { player_id, .. }
            | Self::EnableAiTakeover { player_id, .. }
            | Self::UnlockCommanderNode { player_id, .. }
            | Self::PatrolWaypoints { player_id, .. }
            | Self::ToggleBurrow { player_id, .. }
            | Self::UpgradeBuildingTo { player_id, .. }
            | Self::RetreatSquad { player_id, .. }
            | Self::SetSquadAutoReinforce { player_id, .. }
            | Self::ReinforceSquadNow { player_id, .. }
            | Self::UpgradeSquadWeapon { player_id, .. }
            | Self::ReclaimHumanSlot { player_id }
            | Self::MoveAlongCustomCurve { player_id, .. }
            | Self::AreaCommand { player_id, .. }
            | Self::ToggleAbilityAutoCast { player_id, .. }
            | Self::ResearchUpgradeAt { player_id, .. }
            | Self::CancelResearch { player_id, .. }
            | Self::AdvanceEra { player_id, .. }
            | Self::LearnHeroSkill { player_id, .. }
            | Self::ReviveHero { player_id, .. }
            | Self::PickupItem { player_id, .. }
            | Self::UseInventoryItem { player_id, .. }
            | Self::DropInventoryItem { player_id, .. } => *player_id,
            Self::TransferCredits { from_player_id, .. }
            | Self::TransferUnits { from_player_id, .. } => *from_player_id,
        })
    }

    /// Validates that all floating-point coordinates are finite (`!is_nan() && !is_infinite()`)
    /// and all tile coordinates/lists are within safe bounds so malformed network payloads can
    /// never corrupt `SimWorld` or break JSON save serialization.
    pub fn is_payload_valid(&self) -> bool {
        match self {
            Self::MoveUnits {
                entity_ids,
                target_pos,
                ..
            }
            | Self::QueueMoveUnits {
                entity_ids,
                target_pos,
                ..
            }
            | Self::AttackMove {
                entity_ids,
                target_pos,
                ..
            }
            | Self::PatrolUnits {
                entity_ids,
                target_pos,
                ..
            }
            | Self::ReverseMove {
                entity_ids,
                target_pos,
                ..
            } => entity_ids.len() <= 4096 && is_valid_vec3(*target_pos),
            Self::AttackGround {
                attacker_ids,
                target_pos,
                ..
            } => attacker_ids.len() <= 4096 && is_valid_vec3(*target_pos),
            Self::SetRallyPoint {
                building_ids,
                rally_pos,
                ..
            }
            | Self::QueueRallyPoint {
                building_ids,
                rally_pos,
                ..
            } => building_ids.len() <= 4096 && is_valid_vec3(*rally_pos),
            Self::PingLocation { position, .. } => is_valid_vec3(*position),
            Self::CastSupportPower {
                power_id,
                source_pos,
                target_pos,
                ..
            } => {
                power_id.len() <= 256
                    && source_pos.is_none_or(is_valid_vec3)
                    && is_valid_vec3(*target_pos)
            }
            Self::CastTargetedAbility {
                caster_ids,
                target_pos,
                ..
            }
            | Self::CastTargetedAbilityIndex {
                caster_ids,
                target_pos,
                ..
            } => caster_ids.len() <= 4096 && is_valid_vec3(*target_pos),
            Self::PlaceBuilding {
                building_id,
                tile_pos,
                ..
            }
            | Self::PlaceBuildingRotated {
                building_id,
                tile_pos,
                ..
            } => building_id.len() <= 256 && is_valid_tile(*tile_pos),
            Self::OrderWorkerConstruct {
                worker_ids,
                building_id,
                tile_pos,
                ..
            } => worker_ids.len() <= 4096 && building_id.len() <= 256 && is_valid_tile(*tile_pos),
            Self::PlaceWallLine {
                building_id,
                start_tile,
                end_tile,
                ..
            } => building_id.len() <= 256 && is_valid_tile(*start_tile) && is_valid_tile(*end_tile),
            Self::PlaceMine {
                unit_ids,
                target_tile,
                ..
            } => unit_ids.len() <= 4096 && is_valid_tile(*target_tile),
            Self::SpawnSandboxUnit {
                unit_id, tile_pos, ..
            } => unit_id.len() <= 256 && is_valid_tile(*tile_pos),
            Self::GuardEntity { entity_ids, .. }
            | Self::SetUnitStance { entity_ids, .. }
            | Self::StopUnits { entity_ids, .. }
            | Self::ScatterUnits { entity_ids, .. }
            | Self::ActivateAbility { entity_ids, .. }
            | Self::ActivateAbilityIndex { entity_ids, .. } => entity_ids.len() <= 4096,
            Self::AttackTarget { attacker_ids, .. } => attacker_ids.len() <= 4096,
            Self::LoadIntoCarrier { passenger_ids, .. } => passenger_ids.len() <= 4096,
            Self::UnloadCarrier { carrier_ids, .. } => carrier_ids.len() <= 4096,
            Self::TransferUnits { unit_ids, .. }
            | Self::DockAtServiceDepot { unit_ids, .. }
            | Self::ReturnToAirfield { unit_ids, .. } => unit_ids.len() <= 4096,
            Self::InfiltrateBuilding { spy_ids, .. } | Self::DisguiseUnits { spy_ids, .. } => {
                spy_ids.len() <= 4096
            }
            Self::ToggleBuildingPower { building_ids, .. } => building_ids.len() <= 4096,
            Self::QueueBuilding { building_id, .. } => building_id.len() <= 256,
            Self::ProduceUnit { unit_id, .. } | Self::CancelProduction { unit_id, .. } => {
                unit_id.len() <= 256
            }
            Self::ResearchUpgrade { upgrade_id, .. } => upgrade_id.len() <= 256,
            Self::CancelBuildingQueue { .. }
            | Self::SellBuilding { .. }
            | Self::ToggleRepairBuilding { .. }
            | Self::SetPrimaryBuilding { .. }
            | Self::SetAlliance { .. }
            | Self::TransferCredits { .. }
            | Self::SetFormation { .. }
            | Self::SetFormationLocked { .. }
            | Self::UpgradeBuilding { .. }
            | Self::TogglePauseProduction { .. }
            | Self::SetEnvironmentPreset { .. }
            | Self::OrderCarryallPickup { .. } => true,
            Self::EnqueueOrder {
                entity_ids, order, ..
            } => entity_ids.len() <= 4096 && order.is_valid(),
            Self::MoveUnitsWithFormation {
                entity_ids,
                target_pos,
                facing_radians,
                line_spacing,
                ..
            } => {
                entity_ids.len() <= 4096
                    && is_valid_vec3(*target_pos)
                    && facing_radians.is_finite()
                    && facing_radians.abs() <= 1.0e4
                    && line_spacing.is_finite()
                    && *line_spacing >= 0.0
                    && *line_spacing <= 256.0
            }
            Self::ToggleHoldProduction { item_id, .. } => item_id.len() <= 256,
            Self::EnableAiTakeover { .. } => true,
            Self::UnlockCommanderNode { node_id, .. } => {
                !node_id.is_empty() && node_id.len() <= 256
            }
            Self::PatrolWaypoints {
                entity_ids,
                waypoints,
                ..
            } => {
                entity_ids.len() <= 4096
                    && !waypoints.is_empty()
                    && waypoints.len() <= 128
                    && waypoints.iter().copied().all(is_valid_vec3)
            }
            Self::ToggleBurrow { unit_ids, .. } => unit_ids.len() <= 4096,
            Self::UpgradeBuildingTo { target_def_id, .. } => {
                !target_def_id.is_empty() && target_def_id.len() <= 256
            }
            Self::RetreatSquad { squad_ids, .. }
            | Self::SetSquadAutoReinforce { squad_ids, .. }
            | Self::ReinforceSquadNow { squad_ids, .. } => squad_ids.len() <= 4096,
            Self::UpgradeSquadWeapon { upgrade_id, .. } => {
                !upgrade_id.is_empty() && upgrade_id.len() <= 256
            }
            Self::ReclaimHumanSlot { player_id } => *player_id > 0,
            Self::MoveAlongCustomCurve {
                entity_ids,
                curve_points,
                ..
            } => {
                entity_ids.len() <= 4096
                    && !curve_points.is_empty()
                    && curve_points.len() <= 512
                    && curve_points.iter().copied().all(is_valid_vec3)
            }
            Self::AreaCommand {
                entity_ids,
                center,
                radius,
                ..
            } => {
                entity_ids.len() <= 4096
                    && is_valid_vec3(*center)
                    && radius.is_finite()
                    && *radius > 0.0
                    && *radius <= 512.0
            }
            Self::ToggleAbilityAutoCast {
                entity_ids,
                ability_index,
                ..
            } => entity_ids.len() <= 4096 && *ability_index < 64,
            Self::ResearchUpgradeAt { upgrade_id, .. }
            | Self::CancelResearch { upgrade_id, .. } => {
                !upgrade_id.is_empty() && upgrade_id.len() <= 256
            }
            Self::AdvanceEra { era_id, .. } => !era_id.is_empty() && era_id.len() <= 256,
            Self::LearnHeroSkill { ability_id, .. } => {
                !ability_id.is_empty() && ability_id.len() <= 256
            }
            Self::ReviveHero { hero_def_id, .. } => hero_def_id
                .as_ref()
                .is_none_or(|s| !s.is_empty() && s.len() <= 256),
            Self::PickupItem { .. } => true,
            Self::UseInventoryItem {
                slot_index,
                target_pos,
                ..
            } => *slot_index < 32 && target_pos.is_none_or(is_valid_vec3),
            Self::DropInventoryItem {
                slot_index,
                drop_pos,
                ..
            } => *slot_index < 32 && drop_pos.is_none_or(is_valid_vec3),
        }
    }
}
