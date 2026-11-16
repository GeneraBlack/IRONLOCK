use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::ai_behavior_tree::BehaviorTreeDef;
use super::common::default_one;
use super::common_visuals::default_burst;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SubFactionDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub specialty: String,
    #[serde(default)]
    pub loading_screen: Option<String>,
    #[serde(default = "default_one")]
    pub infantry_cost_mult: f32,
    #[serde(default = "default_one")]
    pub vehicle_cost_mult: f32,
    #[serde(default = "default_one")]
    pub aircraft_cost_mult: f32,
    #[serde(default = "default_one")]
    pub infantry_hp_mult: f32,
    #[serde(default = "default_one")]
    pub vehicle_speed_mult: f32,
    #[serde(default = "default_one")]
    pub aircraft_damage_mult: f32,
    #[serde(default = "default_one")]
    pub aircraft_hp_mult: f32,
    #[serde(default = "default_one")]
    pub infantry_damage_mult: f32,
    #[serde(default = "default_one")]
    pub infantry_range_mult: f32,
    #[serde(default = "default_one")]
    pub defense_rof_mult: f32,
    #[serde(default = "default_one")]
    pub anti_armor_damage_mult: f32,
    #[serde(default = "default_one")]
    pub psychic_cost_mult: f32,
    #[serde(default = "default_one")]
    pub mind_control_cooldown_mult: f32,
    #[serde(default)]
    pub radiation_resist: f32,
    #[serde(default)]
    pub free_support_power: Option<String>,
    #[serde(default)]
    pub flag_colors: Option<[[f32; 3]; 3]>,
    #[serde(default)]
    pub signature_unit: Option<String>,
    #[serde(default)]
    pub passive_bonus_text: Option<String>,
}

impl Default for SubFactionDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            specialty: String::new(),
            loading_screen: None,
            infantry_cost_mult: 1.0,
            vehicle_cost_mult: 1.0,
            aircraft_cost_mult: 1.0,
            infantry_hp_mult: 1.0,
            vehicle_speed_mult: 1.0,
            aircraft_damage_mult: 1.0,
            aircraft_hp_mult: 1.0,
            infantry_damage_mult: 1.0,
            infantry_range_mult: 1.0,
            defense_rof_mult: 1.0,
            anti_armor_damage_mult: 1.0,
            psychic_cost_mult: 1.0,
            mind_control_cooldown_mult: 1.0,
            radiation_resist: 0.0,
            free_support_power: None,
            flag_colors: None,
            signature_unit: None,
            passive_bonus_text: None,
        }
    }
}

impl SubFactionDef {
    pub fn resolved_flag_colors(&self, fallback: [f32; 3]) -> [[f32; 3]; 3] {
        self.flag_colors.unwrap_or([
            fallback,
            [0.92, 0.94, 0.96],
            [fallback[0] * 0.65, fallback[1] * 0.65, fallback[2] * 0.65],
        ])
    }

    pub fn bonus_summary(&self) -> String {
        if let Some(ref custom) = self.passive_bonus_text {
            if !custom.trim().is_empty() {
                return custom.clone();
            }
        }
        let mut parts = Vec::new();
        let fmt_pct = |mult: f32, label: &str, out: &mut Vec<String>| {
            let delta = ((mult - 1.0) * 100.0).round() as i32;
            if delta > 0 {
                out.push(format!("+{delta}% {label}"));
            } else if delta < 0 {
                out.push(format!("{delta}% {label}"));
            }
        };
        fmt_pct(self.infantry_hp_mult, "Infantry HP", &mut parts);
        fmt_pct(self.infantry_cost_mult, "Infantry Cost", &mut parts);
        fmt_pct(self.infantry_damage_mult, "Infantry DMG", &mut parts);
        fmt_pct(self.infantry_range_mult, "Infantry Range", &mut parts);
        fmt_pct(self.vehicle_cost_mult, "Vehicle Cost", &mut parts);
        fmt_pct(self.vehicle_speed_mult, "Vehicle Speed", &mut parts);
        fmt_pct(self.anti_armor_damage_mult, "Anti-Armor DMG", &mut parts);
        fmt_pct(self.aircraft_damage_mult, "Aircraft DMG", &mut parts);
        fmt_pct(self.aircraft_hp_mult, "Aircraft HP", &mut parts);
        fmt_pct(self.aircraft_cost_mult, "Aircraft Cost", &mut parts);
        fmt_pct(self.defense_rof_mult, "Defense ROF", &mut parts);
        fmt_pct(self.psychic_cost_mult, "Psychic/Clone Cost", &mut parts);
        if self.mind_control_cooldown_mult < 0.999 {
            let saved = ((1.0 - self.mind_control_cooldown_mult) * 100.0).round() as i32;
            parts.push(format!("-{saved}% Mind-Control CD"));
        }
        if self.radiation_resist > 0.001 {
            let pct = (self.radiation_resist * 100.0).round() as i32;
            parts.push(format!("{pct}% Radiation Resist"));
        }
        if let Some(ref sp) = self.free_support_power {
            if !sp.is_empty() {
                parts.push(format!("Free {}", sp.replace('_', " ")));
            }
        }
        if parts.is_empty() {
            self.specialty.clone()
        } else {
            parts.join(" | ")
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FactionHudSkinDef {
    #[serde(default)]
    pub dialog_background: Option<String>,
    #[serde(default)]
    pub credits_panel: Option<String>,
    #[serde(default)]
    pub top_panel: Option<String>,
    #[serde(default)]
    pub options_button: Option<String>,
    #[serde(default)]
    pub diplomacy_button: Option<String>,
    #[serde(default)]
    pub radar_frame: Option<String>,
    #[serde(default)]
    pub side_header: Option<String>,
    #[serde(default)]
    pub repair_button: Option<String>,
    #[serde(default)]
    pub sell_button: Option<String>,
    #[serde(default)]
    pub tab_icons: Vec<String>,
    #[serde(default)]
    pub cameo_row_frame: Option<String>,
    #[serde(default)]
    pub cameo_footer_frame: Option<String>,
    #[serde(default)]
    pub addon_panel: Option<String>,
    #[serde(default)]
    pub power_bar_icon: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VfxFlipbookDef {
    #[serde(default)]
    pub source_shp: String,
    #[serde(default)]
    pub description: String,
    pub frame_width: u32,
    pub frame_height: u32,
    #[serde(default)]
    pub total_source_frames: u32,
    pub exported_frames: u32,
    pub columns: u32,
    pub rows: u32,
    #[serde(default = "default_burst")]
    pub scale_factor: u32,
    pub sprite_sheet: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FactionDef {
    pub id: String,
    pub name: String,
    pub color: [f32; 3],
    pub starting_credits: u32,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub loading_screen: Option<String>,
    #[serde(default)]
    pub campaign_loading_screens: Vec<String>,
    #[serde(default)]
    pub sub_factions: Vec<SubFactionDef>,
    #[serde(default)]
    pub hud_skin: FactionHudSkinDef,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FactionCatalog {
    pub factions: Vec<FactionDef>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UnitVoiceSpec {
    #[serde(default)]
    pub select: Vec<String>,
    #[serde(default)]
    pub move_order: Vec<String>,
    #[serde(default)]
    pub attack_order: Vec<String>,
    #[serde(default)]
    pub death: Vec<String>,
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
pub enum MusicMood {
    #[default]
    Peace,
    Combat,
    Menu,
    Score,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MusicTrackDef {
    pub id: String,
    pub display_name: String,
    pub path: String,
    #[serde(default)]
    pub mood: MusicMood,
    #[serde(default)]
    pub factions: Vec<String>,
    #[serde(default)]
    pub faction_tags: Vec<String>,
    #[serde(default)]
    pub duration_hint_secs: Option<f32>,
}

impl MusicTrackDef {
    /// Returns faction affinities from `faction_tags` when non-empty, falling back to legacy `factions`.
    pub fn effective_factions(&self) -> &[String] {
        if !self.faction_tags.is_empty() {
            &self.faction_tags
        } else {
            &self.factions
        }
    }
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
pub enum AudioBusKind {
    #[default]
    Master,
    Music,
    CombatSfx,
    UnitVoice,
    Eva,
}

impl AudioBusKind {
    pub const ALL: [Self; 5] = [
        Self::Master,
        Self::Music,
        Self::CombatSfx,
        Self::UnitVoice,
        Self::Eva,
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Master => "Master",
            Self::Music => "Music",
            Self::CombatSfx => "Combat SFX",
            Self::UnitVoice => "Unit Voice",
            Self::Eva => "EVA",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AudioCatalog {
    #[serde(default)]
    pub weapon_sfx: BTreeMap<String, String>,
    #[serde(default)]
    pub unit_voices: BTreeMap<String, UnitVoiceSpec>,
    #[serde(default)]
    pub music_tracks: Vec<MusicTrackDef>,
    #[serde(default)]
    pub eva_cues: BTreeMap<String, String>,
    #[serde(default)]
    pub faction_eva_cues: BTreeMap<String, BTreeMap<String, String>>,
}

impl AudioCatalog {
    /// Resolves a faction-specific EVA cue (`faction_eva_cues[faction_id][cue_key]`),
    /// falling back to `<faction_id>_<cue_key>` in `eva_cues` and finally shared `eva_cues[cue_key]`.
    pub fn resolve_eva_cue(&self, faction_id: &str, cue_key: &str) -> Option<&str> {
        if !faction_id.is_empty() {
            if let Some(f_map) = self.faction_eva_cues.get(faction_id) {
                if let Some(path) = f_map.get(cue_key) {
                    return Some(path.as_str());
                }
            }
            let prefixed = format!("{faction_id}_{cue_key}");
            if let Some(path) = self.eva_cues.get(&prefixed) {
                return Some(path.as_str());
            }
        }
        self.eva_cues.get(cue_key).map(String::as_str)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LocaleCatalog {
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub strings: BTreeMap<String, String>,
}

fn default_ai_personality() -> String {
    "balanced".to_string()
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AiProfileDef {
    pub id: String,
    pub faction: String,
    #[serde(default = "default_ai_personality")]
    pub personality: String,
    #[serde(default)]
    pub personality_id: Option<String>,
    #[serde(default)]
    pub build_order: Vec<String>,
    #[serde(default)]
    pub min_attack_wave_size: Option<usize>,
    #[serde(default)]
    pub expand_at_credits: Option<i64>,
    #[serde(default)]
    pub superweapon_doctrine: Option<String>,
    #[serde(default)]
    pub amphibious_assault_enabled: Option<bool>,
    #[serde(default)]
    pub preferred_superweapons: Vec<String>,
    #[serde(default)]
    pub on_ai_tick: Option<String>,
    #[serde(default)]
    pub unit_weights: BTreeMap<String, u32>,
    #[serde(default)]
    pub unit_role_quotas: BTreeMap<String, f32>,
    #[serde(default)]
    pub target_priority_weights: BTreeMap<String, f32>,
    #[serde(default)]
    pub behavior_tree: Option<BehaviorTreeDef>,
}

impl AiProfileDef {
    /// Returns `personality_id` when explicitly set and non-empty, otherwise `personality`.
    pub fn effective_personality_id(&self) -> &str {
        self.personality_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(self.personality.as_str())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AiProfileCatalog {
    #[serde(default)]
    pub profiles: Vec<AiProfileDef>,
}
