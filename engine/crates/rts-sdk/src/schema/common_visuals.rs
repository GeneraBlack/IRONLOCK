use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{ArmorType, MovementDomain};
use super::entities::UnitRole;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SuperweaponType {
    NuclearMissile,
    WeatherStorm,
    IronCurtain,
    Chronosphere,
    Paratroopers,
    GeneticMutator,
    PsychicDominator,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VisualArchetype {
    Infantry,
    Tank,
    Harvester,
    Aircraft,
    Ship,
    ConstructionYard,
    PowerPlant,
    Barracks,
    Refinery,
    WarFactory,
    Turret,
    #[default]
    GenericBox,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TeamMaskMode {
    #[default]
    AutoNeutralGray,
    AlphaMask,
    VertexColorMask,
    FullTint,
    Disabled,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SocketOffset {
    pub name: String,
    #[serde(default)]
    pub offset: [f32; 3],
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EntityAnimState {
    #[default]
    Idle,
    Walk,
    Prone,
    Attack,
    Deploy,
    Die,
    Constructing,
    Producing,
    Damaged,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AnimClipMap {
    #[serde(default)]
    pub idle: Option<String>,
    #[serde(default)]
    pub walk: Option<String>,
    #[serde(default)]
    pub prone: Option<String>,
    #[serde(default)]
    pub attack: Option<String>,
    #[serde(default)]
    pub deploy: Option<String>,
    #[serde(default)]
    pub die: Option<String>,
    #[serde(default)]
    pub constructing: Option<String>,
    #[serde(default)]
    pub producing: Option<String>,
    #[serde(default)]
    pub damaged: Option<String>,
}

impl AnimClipMap {
    pub fn clip_for_state(&self, state: EntityAnimState) -> Option<&str> {
        match state {
            EntityAnimState::Idle => self.idle.as_deref(),
            EntityAnimState::Walk => self.walk.as_deref(),
            EntityAnimState::Prone => self.prone.as_deref(),
            EntityAnimState::Attack => self.attack.as_deref(),
            EntityAnimState::Deploy => self.deploy.as_deref(),
            EntityAnimState::Die => self.die.as_deref(),
            EntityAnimState::Constructing => self.constructing.as_deref(),
            EntityAnimState::Producing => self.producing.as_deref(),
            EntityAnimState::Damaged => self.damaged.as_deref(),
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum LocomotorKind {
    Infantry,
    Wheeled,
    Tracked,
    Hover,
    Naval,
    FixedWingAircraft,
    Vtol,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LocomotorSpec {
    #[serde(default)]
    pub kind: Option<LocomotorKind>,
    #[serde(default)]
    pub turn_rate_deg_per_sec: f32,
    #[serde(default)]
    pub turret_slew_deg_per_sec: f32,
    #[serde(default)]
    pub accel: f32,
    #[serde(default)]
    pub decel: f32,
    #[serde(default)]
    pub attack_run_radius: f32,
}

impl LocomotorSpec {
    /// Resolves `None` / `0.0` fields into sensible physics kinematics defaults based on
    /// `UnitRole`, `MovementDomain`, armor weight, and aircraft rotor/ammo profile.
    #[allow(clippy::too_many_arguments)]
    pub fn resolved_for(
        &self,
        role: UnitRole,
        domain: MovementDomain,
        armor: ArmorType,
        crushes_infantry: bool,
        spin_speed_deg_per_sec: f32,
        lands_when_deployed: bool,
        max_ammo: u32,
    ) -> Self {
        let kind = self.kind.unwrap_or(match domain {
            MovementDomain::Air => {
                if spin_speed_deg_per_sec > 0.0 || lands_when_deployed || max_ammo == 0 {
                    LocomotorKind::Vtol
                } else {
                    LocomotorKind::FixedWingAircraft
                }
            }
            MovementDomain::Jumpjet => LocomotorKind::Vtol,
            MovementDomain::Naval => LocomotorKind::Naval,
            MovementDomain::Amphibious => LocomotorKind::Hover,
            MovementDomain::Ground | MovementDomain::Chrono => match role {
                UnitRole::Infantry | UnitRole::Engineer => LocomotorKind::Infantry,
                UnitRole::Vehicle | UnitRole::Harvester => {
                    if armor == ArmorType::Light && !crushes_infantry {
                        LocomotorKind::Wheeled
                    } else {
                        LocomotorKind::Tracked
                    }
                }
            },
        });

        let (def_turn, def_slew, def_accel, def_decel, def_run) = match kind {
            LocomotorKind::Infantry => (720.0, 720.0, 28.0, 36.0, 0.0),
            LocomotorKind::Wheeled => (240.0, 260.0, 14.0, 20.0, 0.0),
            LocomotorKind::Tracked => (135.0, 180.0, 9.0, 16.0, 0.0),
            LocomotorKind::Hover => (210.0, 220.0, 11.0, 8.5, 0.0),
            LocomotorKind::Naval => (75.0, 120.0, 5.5, 6.5, 0.0),
            LocomotorKind::FixedWingAircraft => (120.0, 360.0, 18.0, 12.0, 8.0),
            LocomotorKind::Vtol => (220.0, 300.0, 14.0, 16.0, 0.0),
        };

        Self {
            kind: Some(kind),
            turn_rate_deg_per_sec: if self.turn_rate_deg_per_sec > 0.0 {
                self.turn_rate_deg_per_sec
            } else {
                def_turn
            },
            turret_slew_deg_per_sec: if self.turret_slew_deg_per_sec > 0.0 {
                self.turret_slew_deg_per_sec
            } else {
                def_slew
            },
            accel: if self.accel > 0.0 {
                self.accel
            } else {
                def_accel
            },
            decel: if self.decel > 0.0 {
                self.decel
            } else {
                def_decel
            },
            attack_run_radius: if self.attack_run_radius > 0.0 {
                self.attack_run_radius
            } else {
                def_run
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VisualDef {
    pub model: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub archetype: VisualArchetype,
    #[serde(default = "default_scale")]
    pub scale: [f32; 3],
    #[serde(default = "default_color")]
    pub primary_color: [f32; 3],
    #[serde(default = "default_true")]
    pub team_color_tint: bool,
    #[serde(default)]
    pub has_turret: bool,
    #[serde(default)]
    pub barrel_recoil_distance: f32,
    #[serde(default)]
    pub spin_speed_deg_per_sec: f32,
    #[serde(default)]
    pub sockets: Vec<SocketOffset>,
    #[serde(default)]
    pub animations: AnimClipMap,
    #[serde(default)]
    pub has_damage_states: bool,
    #[serde(default)]
    pub metallic: Option<f32>,
    #[serde(default)]
    pub roughness: Option<f32>,
    #[serde(default)]
    pub emissive_boost: Option<f32>,
    #[serde(default)]
    pub team_mask_mode: TeamMaskMode,
    #[serde(default)]
    pub team_mask_threshold: Option<f32>,
    #[serde(default)]
    pub normal_map_path: Option<String>,
    #[serde(default)]
    pub orm_map_path: Option<String>,
}

impl Default for VisualDef {
    fn default() -> Self {
        Self {
            model: String::new(),
            icon: None,
            archetype: VisualArchetype::GenericBox,
            scale: default_scale(),
            primary_color: default_color(),
            team_color_tint: true,
            has_turret: false,
            barrel_recoil_distance: 0.0,
            spin_speed_deg_per_sec: 0.0,
            sockets: Vec::new(),
            animations: AnimClipMap::default(),
            has_damage_states: false,
            metallic: None,
            roughness: None,
            emissive_boost: None,
            team_mask_mode: TeamMaskMode::AutoNeutralGray,
            team_mask_threshold: None,
            normal_map_path: None,
            orm_map_path: None,
        }
    }
}

impl VisualDef {
    pub fn find_socket(&self, name: &str) -> Option<[f32; 3]> {
        self.sockets
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
            .map(|s| s.offset)
    }

    pub fn muzzle_offsets(&self) -> Vec<[f32; 3]> {
        let muzzles: Vec<[f32; 3]> = self
            .sockets
            .iter()
            .filter(|s| s.name.to_ascii_lowercase().starts_with("muzzle"))
            .map(|s| s.offset)
            .collect();
        if muzzles.is_empty() {
            vec![[0.0, self.scale[1] * 0.55, self.scale[2] * 0.55]]
        } else {
            muzzles
        }
    }
}

pub(crate) fn default_scale() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

pub(crate) fn default_color() -> [f32; 3] {
    [0.6, 0.6, 0.6]
}

pub(crate) fn default_true() -> bool {
    true
}

pub(crate) fn default_burst() -> u32 {
    1
}

pub(crate) fn sub_faction_matches(
    sub_factions: &[String],
    player_faction: &str,
    player_sub_faction: Option<&str>,
) -> bool {
    if sub_factions.is_empty() {
        return true;
    }
    if sub_factions.iter().any(|sf| {
        sf.eq_ignore_ascii_case("campaign_vip") || sf.eq_ignore_ascii_case("unbuildable_minion")
    }) {
        return player_sub_faction
            .is_some_and(|sub| sub_factions.iter().any(|sf| sf.eq_ignore_ascii_case(sub)));
    }
    let Some(sub) = player_sub_faction else {
        return true;
    };
    sub_factions
        .iter()
        .any(|sf| sf.eq_ignore_ascii_case(sub) || sf.eq_ignore_ascii_case(player_faction))
}
