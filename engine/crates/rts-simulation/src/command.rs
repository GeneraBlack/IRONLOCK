use serde::{Deserialize, Serialize};

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
}
