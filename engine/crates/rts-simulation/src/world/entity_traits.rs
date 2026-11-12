//! Sparse `EntityTraitStores` & `DefId`/`TagId` hot-path interning (`Tasks 230, 231, 232`).
//!
//! Extracts rare/genre-specific per-entity states (`PsionicControlState`, `InfiltrationState`,
//! `TemporalState`, `CarrierState`, `ProductionFacilityState`, `PlantedDemolitionState`,
//! `HeroInstanceState`, `EnergyPoolState`) out of `SimEntity` into compact `BTreeMap<u64, T>`
//! tables guarded by a 1-byte `trait_flags` bitmask on `SimEntity` for 1-cycle fast-path skips.

use crate::flow_field::FlowFieldHandle;
use crate::world::{EntityKind, SimEntity, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::manifest::DamageMatrixDef;
use rts_sdk::schema::{ArmorType, DefId, StringInterner, TagId, UnitRole, UpgradeDef, WeaponDef};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

pub const TRAIT_PSIONIC: u8 = 1 << 0;
pub const TRAIT_INFILTRATION: u8 = 1 << 1;
pub const TRAIT_TEMPORAL: u8 = 1 << 2;
pub const TRAIT_CARRIER: u8 = 1 << 3;
pub const TRAIT_PRODUCTION: u8 = 1 << 4;
pub const TRAIT_DEMOLITION: u8 = 1 << 5;
pub const TRAIT_HERO: u8 = 1 << 6;
pub const TRAIT_ENERGY: u8 = 1 << 7;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PsionicControlState {
    pub mind_controlled_by: Option<(u64, u8)>,
}

impl PsionicControlState {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.mind_controlled_by.is_none()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InfiltrationState {
    pub disguised_as: Option<(String, u8)>,
    #[serde(default)]
    pub capture_bonus_claimed: bool,
    #[serde(default)]
    pub infiltrate_target_id: Option<u64>,
}

impl InfiltrationState {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.disguised_as.is_none()
            && !self.capture_bonus_claimed
            && self.infiltrate_target_id.is_none()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TemporalState {
    pub temporal_progress: f32,
}

impl TemporalState {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.temporal_progress <= 0.0
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CarrierState {
    pub boarding_carrier_id: Option<u64>,
    pub parent_carrier_id: Option<u64>,
    pub carryall_pickup_target_id: Option<u64>,
    pub home_airfield_id: Option<u64>,
    pub service_depot_target_id: Option<u64>,
    #[serde(default)]
    pub salvage_upgraded: bool,
    #[serde(default)]
    pub point_defense_timer: f32,
}

impl CarrierState {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.boarding_carrier_id.is_none()
            && self.parent_carrier_id.is_none()
            && self.carryall_pickup_target_id.is_none()
            && self.home_airfield_id.is_none()
            && self.service_depot_target_id.is_none()
            && !self.salvage_upgraded
            && self.point_defense_timer <= 0.0
    }
}

/// Per-building local research queue item (`Task 231`), supporting both `LumpSum` and `Streaming` economy drain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalResearchItem {
    pub upgrade_id: String,
    pub remaining_secs: f32,
    pub total_secs: f32,
    #[serde(default)]
    pub paid_fraction: f32,
    #[serde(default)]
    pub is_era_advance: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProductionFacilityState {
    #[serde(default)]
    pub local_research_queue: VecDeque<LocalResearchItem>,
    #[serde(default)]
    pub rally_flow_handle: Option<FlowFieldHandle>,
}

impl ProductionFacilityState {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.local_research_queue.is_empty() && self.rally_flow_handle.is_none()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PlantedDemolitionState {
    pub planted_c4_owner: Option<u8>,
    #[serde(default)]
    pub pending_airstrike_timer: f32,
    #[serde(default)]
    pub pending_airstrike_damage: f32,
    #[serde(default)]
    pub pending_airstrike_attacker_player: u8,
}

impl PlantedDemolitionState {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.planted_c4_owner.is_none()
            && self.pending_airstrike_timer <= 0.0
            && self.pending_airstrike_damage <= 0.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventorySlotItem {
    pub item_id: String,
    #[serde(default = "default_one_charge")]
    pub charges: u8,
}

fn default_one_charge() -> u8 {
    1
}

/// Per-hero progression, skill ranks (`1..=4`), and `N`-slot relic/item inventory (`Task 232`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeroInstanceState {
    pub level: u8,
    pub skill_points: u8,
    #[serde(default)]
    pub learned_skills: BTreeMap<String, u8>,
    #[serde(default)]
    pub inventory: Vec<Option<InventorySlotItem>>,
}

impl Default for HeroInstanceState {
    fn default() -> Self {
        Self {
            level: 1,
            skill_points: 1,
            learned_skills: BTreeMap::new(),
            inventory: vec![None; 6],
        }
    }
}

/// Snapshot of a fallen hero eligible for revival at a matching altar/HQ building (`Task 232`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FallenHeroRecord {
    pub original_entity_id: u64,
    pub unit_id: String,
    pub experience: f32,
    pub hero_state: HeroInstanceState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeroRevivalJob {
    pub record: FallenHeroRecord,
    pub remaining_secs: f32,
    pub total_secs: f32,
    #[serde(default)]
    pub paid_fraction: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EnergyPoolState {
    pub energy: f32,
    pub max_energy: f32,
    pub regen_per_sec: f32,
}

/// Sparse trait composition store attached to `SimWorld` (`Task 230`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EntityTraitStores {
    #[serde(default)]
    pub psionic: BTreeMap<u64, PsionicControlState>,
    #[serde(default)]
    pub infiltration: BTreeMap<u64, InfiltrationState>,
    #[serde(default)]
    pub temporal: BTreeMap<u64, TemporalState>,
    #[serde(default)]
    pub carrier: BTreeMap<u64, CarrierState>,
    #[serde(default)]
    pub production: BTreeMap<u64, ProductionFacilityState>,
    #[serde(default)]
    pub demolition: BTreeMap<u64, PlantedDemolitionState>,
    #[serde(default)]
    pub heroes: BTreeMap<u64, HeroInstanceState>,
    #[serde(default)]
    pub energy_pools: BTreeMap<u64, EnergyPoolState>,
    #[serde(default)]
    pub fallen_heroes: BTreeMap<u8, Vec<FallenHeroRecord>>,
    #[serde(default)]
    pub revival_queues: BTreeMap<u64, VecDeque<HeroRevivalJob>>,
}

impl EntityTraitStores {
    pub fn remove_entity(&mut self, id: u64) {
        self.psionic.remove(&id);
        self.infiltration.remove(&id);
        self.temporal.remove(&id);
        self.carrier.remove(&id);
        self.production.remove(&id);
        self.demolition.remove(&id);
        self.heroes.remove(&id);
        self.energy_pools.remove(&id);
        self.revival_queues.remove(&id);
    }

    #[inline]
    pub fn mind_controlled_by(&self, ent: &SimEntity) -> Option<(u64, u8)> {
        if (ent.trait_flags & TRAIT_PSIONIC) == 0 {
            return None;
        }
        self.psionic.get(&ent.id).and_then(|s| s.mind_controlled_by)
    }

    #[inline]
    pub fn mind_controlled_by_id(&self, id: u64) -> Option<(u64, u8)> {
        self.psionic.get(&id).and_then(|s| s.mind_controlled_by)
    }

    pub fn set_mind_controlled_by_on(&mut self, ent: &mut SimEntity, val: Option<(u64, u8)>) {
        if let Some(mc) = val {
            ent.trait_flags |= TRAIT_PSIONIC;
            self.psionic.entry(ent.id).or_default().mind_controlled_by = Some(mc);
        } else {
            self.psionic.remove(&ent.id);
            ent.trait_flags &= !TRAIT_PSIONIC;
        }
    }

    #[inline]
    pub fn disguised_as<'a>(&'a self, ent: &SimEntity) -> Option<&'a (String, u8)> {
        if (ent.trait_flags & TRAIT_INFILTRATION) == 0 {
            return None;
        }
        self.infiltration
            .get(&ent.id)
            .and_then(|s| s.disguised_as.as_ref())
    }

    #[inline]
    pub fn disguised_as_id(&self, id: u64) -> Option<&(String, u8)> {
        self.infiltration
            .get(&id)
            .and_then(|s| s.disguised_as.as_ref())
    }

    pub fn set_disguised_as_on(&mut self, ent: &mut SimEntity, val: Option<(String, u8)>) {
        if val.is_some() {
            ent.trait_flags |= TRAIT_INFILTRATION;
            self.infiltration.entry(ent.id).or_default().disguised_as = val;
        } else if let Some(st) = self.infiltration.get_mut(&ent.id) {
            st.disguised_as = None;
            if st.is_empty() {
                self.infiltration.remove(&ent.id);
                ent.trait_flags &= !TRAIT_INFILTRATION;
            }
        }
    }

    #[inline]
    pub fn capture_bonus_claimed(&self, ent: &SimEntity) -> bool {
        (ent.trait_flags & TRAIT_INFILTRATION) != 0
            && self
                .infiltration
                .get(&ent.id)
                .is_some_and(|s| s.capture_bonus_claimed)
    }

    pub fn set_capture_bonus_claimed_on(&mut self, ent: &mut SimEntity, claimed: bool) {
        if claimed {
            ent.trait_flags |= TRAIT_INFILTRATION;
            self.infiltration
                .entry(ent.id)
                .or_default()
                .capture_bonus_claimed = true;
        } else if let Some(st) = self.infiltration.get_mut(&ent.id) {
            st.capture_bonus_claimed = false;
            if st.is_empty() {
                self.infiltration.remove(&ent.id);
                ent.trait_flags &= !TRAIT_INFILTRATION;
            }
        }
    }

    pub fn set_temporal_progress_on(&mut self, ent: &mut SimEntity, progress: f32) {
        let p = progress.max(0.0);
        ent.temporal_progress = p;
        if p > 0.0 {
            ent.trait_flags |= TRAIT_TEMPORAL;
            self.temporal.entry(ent.id).or_default().temporal_progress = p;
        } else {
            self.temporal.remove(&ent.id);
            ent.trait_flags &= !TRAIT_TEMPORAL;
        }
    }

    #[inline]
    pub fn boarding_carrier_id(&self, ent: &SimEntity) -> Option<u64> {
        if (ent.trait_flags & TRAIT_CARRIER) == 0 {
            return None;
        }
        self.carrier
            .get(&ent.id)
            .and_then(|s| s.boarding_carrier_id)
    }

    #[inline]
    pub fn boarding_carrier_id_by_id(&self, id: u64) -> Option<u64> {
        self.carrier.get(&id).and_then(|s| s.boarding_carrier_id)
    }

    pub fn set_boarding_carrier_id_on(&mut self, ent: &mut SimEntity, val: Option<u64>) {
        self.mutate_carrier_on(ent, |c| c.boarding_carrier_id = val);
    }

    #[inline]
    pub fn parent_carrier_id(&self, ent: &SimEntity) -> Option<u64> {
        if (ent.trait_flags & TRAIT_CARRIER) == 0 {
            return None;
        }
        self.carrier.get(&ent.id).and_then(|s| s.parent_carrier_id)
    }

    #[inline]
    pub fn parent_carrier_id_by_id(&self, id: u64) -> Option<u64> {
        self.carrier.get(&id).and_then(|s| s.parent_carrier_id)
    }

    pub fn set_parent_carrier_id_on(&mut self, ent: &mut SimEntity, val: Option<u64>) {
        self.mutate_carrier_on(ent, |c| c.parent_carrier_id = val);
    }

    #[inline]
    pub fn carryall_pickup_target_id(&self, ent: &SimEntity) -> Option<u64> {
        if (ent.trait_flags & TRAIT_CARRIER) == 0 {
            return None;
        }
        self.carrier
            .get(&ent.id)
            .and_then(|s| s.carryall_pickup_target_id)
    }

    pub fn set_carryall_pickup_target_id_on(&mut self, ent: &mut SimEntity, val: Option<u64>) {
        self.mutate_carrier_on(ent, |c| c.carryall_pickup_target_id = val);
    }

    #[inline]
    pub fn home_airfield_id(&self, ent: &SimEntity) -> Option<u64> {
        if (ent.trait_flags & TRAIT_CARRIER) == 0 {
            return None;
        }
        self.carrier.get(&ent.id).and_then(|s| s.home_airfield_id)
    }

    pub fn set_home_airfield_id_on(&mut self, ent: &mut SimEntity, val: Option<u64>) {
        self.mutate_carrier_on(ent, |c| c.home_airfield_id = val);
    }

    #[inline]
    pub fn service_depot_target_id(&self, ent: &SimEntity) -> Option<u64> {
        if (ent.trait_flags & TRAIT_CARRIER) == 0 {
            return None;
        }
        self.carrier
            .get(&ent.id)
            .and_then(|s| s.service_depot_target_id)
    }

    pub fn set_service_depot_target_id_on(&mut self, ent: &mut SimEntity, val: Option<u64>) {
        self.mutate_carrier_on(ent, |c| c.service_depot_target_id = val);
    }

    #[inline]
    pub fn salvage_upgraded(&self, ent: &SimEntity) -> bool {
        (ent.trait_flags & TRAIT_CARRIER) != 0
            && self
                .carrier
                .get(&ent.id)
                .is_some_and(|s| s.salvage_upgraded)
    }

    pub fn set_salvage_upgraded_on(&mut self, ent: &mut SimEntity, val: bool) {
        self.mutate_carrier_on(ent, |c| c.salvage_upgraded = val);
    }

    #[inline]
    pub fn point_defense_timer(&self, ent: &SimEntity) -> f32 {
        if (ent.trait_flags & TRAIT_CARRIER) == 0 {
            return 0.0;
        }
        self.carrier
            .get(&ent.id)
            .map(|s| s.point_defense_timer)
            .unwrap_or(0.0)
    }

    pub fn set_point_defense_timer_on(&mut self, ent: &mut SimEntity, val: f32) {
        self.mutate_carrier_on(ent, |c| c.point_defense_timer = val.max(0.0));
    }

    /// Clears transient order-specific carrier targets (`boarding_carrier_id`,
    /// `carryall_pickup_target_id`, `service_depot_target_id`) in 1 cycle when `TRAIT_CARRIER == 0`.
    #[inline]
    pub fn clear_order_carrier_targets_on(&mut self, ent: &mut SimEntity) {
        if (ent.trait_flags & TRAIT_CARRIER) == 0 {
            return;
        }
        self.mutate_carrier_on(ent, |c| {
            c.boarding_carrier_id = None;
            c.carryall_pickup_target_id = None;
            c.service_depot_target_id = None;
        });
    }

    fn mutate_carrier_on<F: FnOnce(&mut CarrierState)>(&mut self, ent: &mut SimEntity, f: F) {
        let entry = self.carrier.entry(ent.id).or_default();
        f(entry);
        if entry.is_empty() {
            self.carrier.remove(&ent.id);
            ent.trait_flags &= !TRAIT_CARRIER;
        } else {
            ent.trait_flags |= TRAIT_CARRIER;
        }
    }

    #[inline]
    pub fn planted_c4_owner(&self, ent: &SimEntity) -> Option<u8> {
        if (ent.trait_flags & TRAIT_DEMOLITION) != 0 {
            if let Some(p) = self
                .demolition
                .get(&ent.id)
                .and_then(|s| s.planted_c4_owner)
            {
                return Some(p);
            }
        }
        ent.c4_bomb_info()
            .and_then(|(_, _, _, p)| (p != 0).then_some(p))
    }

    pub fn set_planted_c4_owner_on(&mut self, ent: &mut SimEntity, owner: Option<u8>) {
        self.mutate_demolition_on(ent, |d| d.planted_c4_owner = owner);
    }

    #[inline]
    pub fn pending_airstrike(&self, ent: &SimEntity) -> Option<(f32, f32, u8)> {
        if (ent.trait_flags & TRAIT_DEMOLITION) == 0 {
            return None;
        }
        let d = self.demolition.get(&ent.id)?;
        (d.pending_airstrike_timer > 0.0 || d.pending_airstrike_damage > 0.0).then_some((
            d.pending_airstrike_timer,
            d.pending_airstrike_damage,
            d.pending_airstrike_attacker_player,
        ))
    }

    pub fn set_pending_airstrike_on(
        &mut self,
        ent: &mut SimEntity,
        timer: f32,
        damage: f32,
        attacker_player: u8,
    ) {
        self.mutate_demolition_on(ent, |d| {
            d.pending_airstrike_timer = timer.max(0.0);
            d.pending_airstrike_damage = damage.max(0.0);
            d.pending_airstrike_attacker_player = attacker_player;
        });
    }

    fn mutate_demolition_on<F: FnOnce(&mut PlantedDemolitionState)>(
        &mut self,
        ent: &mut SimEntity,
        f: F,
    ) {
        let entry = self.demolition.entry(ent.id).or_default();
        f(entry);
        if entry.is_empty() {
            self.demolition.remove(&ent.id);
            ent.trait_flags &= !TRAIT_DEMOLITION;
        } else {
            ent.trait_flags |= TRAIT_DEMOLITION;
        }
    }
}

/// Precomputed `DefId(u16)` and `TagId(u16)` hot-path lookup tables rebuilt from `LoadedMod` (`Task 230`).
#[derive(Debug, Clone, Default)]
pub struct SimInternTable {
    pub defs: StringInterner,
    pub tags: StringInterner,
    pub def_tags: Vec<Vec<TagId>>,
    pub def_armor_tag: Vec<TagId>,
    pub damage_matrix_lut: BTreeMap<(TagId, TagId), f32>,
    pub upgrade_target_defs: BTreeMap<DefId, Vec<DefId>>,
    pub upgrade_target_tags: BTreeMap<DefId, Vec<TagId>>,
    pub synced_mod_id: String,
    pub synced_counts: (usize, usize, usize, usize),
}

impl SimInternTable {
    pub fn ensure_synced(&mut self, loaded: &LoadedMod) {
        let counts = (
            loaded.units.len(),
            loaded.buildings.len(),
            loaded.weapons.len(),
            loaded.upgrades.len(),
        );
        if self.synced_mod_id == loaded.manifest.id
            && self.synced_counts == counts
            && !self.defs.is_empty()
        {
            return;
        }
        *self = Self::from_loaded_mod(loaded);
    }

    pub fn from_loaded_mod(loaded: &LoadedMod) -> Self {
        let mut defs = StringInterner::new();
        let mut tags = StringInterner::new();

        for a in [
            ArmorType::None,
            ArmorType::Light,
            ArmorType::Medium,
            ArmorType::Heavy,
            ArmorType::Structure,
        ] {
            tags.intern_tag(a.as_tag_str());
        }

        for u in loaded.units.values() {
            defs.intern_def(&u.id);
            tags.intern_tag(u.effective_armor_tag());
            for t in &u.tags {
                tags.intern_tag(t);
            }
        }
        for b in loaded.buildings.values() {
            defs.intern_def(&b.id);
            tags.intern_tag(b.effective_armor_tag());
            for t in &b.tags {
                tags.intern_tag(t);
            }
        }
        for w in loaded.weapons.values() {
            defs.intern_def(&w.id);
            if let Some(wt) = &w.warhead_tag {
                tags.intern_tag(wt);
            }
        }
        for upg in loaded.upgrades.values() {
            defs.intern_def(&upg.id);
            for t in &upg.target_tags {
                tags.intern_tag(t);
            }
            for d in &upg.target_def_ids {
                defs.intern_def(d);
            }
        }
        for (wh_tag, row) in &loaded.manifest.rules.damage_matrix.warhead_vs_armor {
            let wt = tags.intern_tag(wh_tag);
            for arm_tag in row.keys() {
                let _ = (wt, tags.intern_tag(arm_tag));
            }
        }

        let mut def_tags = vec![Vec::new(); defs.len()];
        let mut def_armor_tag = vec![TagId::NONE; defs.len()];

        for u in loaded.units.values() {
            if let Some(did) = defs.lookup_def(&u.id) {
                let idx = did.0 as usize;
                def_armor_tag[idx] = tags
                    .lookup_tag(u.effective_armor_tag())
                    .unwrap_or(TagId::NONE);
                def_tags[idx] = u.tags.iter().filter_map(|t| tags.lookup_tag(t)).collect();
            }
        }
        for b in loaded.buildings.values() {
            if let Some(did) = defs.lookup_def(&b.id) {
                let idx = did.0 as usize;
                def_armor_tag[idx] = tags
                    .lookup_tag(b.effective_armor_tag())
                    .unwrap_or(TagId::NONE);
                def_tags[idx] = b.tags.iter().filter_map(|t| tags.lookup_tag(t)).collect();
            }
        }

        let mut damage_matrix_lut = BTreeMap::new();
        for (wh_str, row) in &loaded.manifest.rules.damage_matrix.warhead_vs_armor {
            if let Some(wt) = tags.lookup_tag(wh_str) {
                for (arm_str, &mult) in row {
                    if let Some(at) = tags.lookup_tag(arm_str) {
                        damage_matrix_lut.insert((wt, at), mult);
                    }
                }
            }
        }

        let mut upgrade_target_defs = BTreeMap::new();
        let mut upgrade_target_tags = BTreeMap::new();
        for upg in loaded.upgrades.values() {
            if let Some(uid) = defs.lookup_def(&upg.id) {
                let td: Vec<DefId> = upg
                    .target_def_ids
                    .iter()
                    .filter_map(|d| defs.lookup_def(d))
                    .collect();
                if !td.is_empty() {
                    upgrade_target_defs.insert(uid, td);
                }
                let tt: Vec<TagId> = upg
                    .target_tags
                    .iter()
                    .filter_map(|t| tags.lookup_tag(t))
                    .collect();
                if !tt.is_empty() {
                    upgrade_target_tags.insert(uid, tt);
                }
            }
        }

        Self {
            defs,
            tags,
            def_tags,
            def_armor_tag,
            damage_matrix_lut,
            upgrade_target_defs,
            upgrade_target_tags,
            synced_mod_id: loaded.manifest.id.clone(),
            synced_counts: (
                loaded.units.len(),
                loaded.buildings.len(),
                loaded.weapons.len(),
                loaded.upgrades.len(),
            ),
        }
    }

    #[inline]
    pub fn lookup_def_id(&self, id: &str) -> DefId {
        self.defs.lookup_def(id).unwrap_or(DefId::NONE)
    }

    /// Evaluates whether `upg` applies to an entity (`Unit` or `Building`) via `DefId`/`TagId`
    /// (`target_def_ids`, `target_tags`) or legacy `target_role` (`Task 230`).
    pub fn upgrade_matches_entity(
        &self,
        upg: &UpgradeDef,
        entity_def_str: &str,
        entity_def_id: DefId,
        unit_role: Option<UnitRole>,
        is_building: bool,
    ) -> bool {
        self.upgrade_matches_entity_with_tags(
            upg,
            entity_def_str,
            entity_def_id,
            unit_role,
            is_building,
            &[],
        )
    }

    pub fn upgrade_matches_entity_with_tags(
        &self,
        upg: &UpgradeDef,
        entity_def_str: &str,
        entity_def_id: DefId,
        unit_role: Option<UnitRole>,
        is_building: bool,
        fallback_tags: &[String],
    ) -> bool {
        let resolved_def = if entity_def_id.is_some() {
            entity_def_id
        } else {
            self.lookup_def_id(entity_def_str)
        };
        let has_explicit_filter = upg.target_role.is_some()
            || !upg.target_def_ids.is_empty()
            || !upg.target_tags.is_empty();
        if !has_explicit_filter {
            // Legacy global unit upgrade applies to all units (not buildings unless explicitly targeted)
            return !is_building;
        }
        if let Some(req_role) = upg.target_role {
            if unit_role == Some(req_role) {
                return true;
            }
        }
        if !upg.target_def_ids.is_empty() {
            if let Some(upg_did) = self.defs.lookup_def(&upg.id) {
                if resolved_def.is_some()
                    && self
                        .upgrade_target_defs
                        .get(&upg_did)
                        .is_some_and(|list| list.contains(&resolved_def))
                {
                    return true;
                }
            }
            if upg.target_def_ids.iter().any(|s| s == entity_def_str) {
                return true;
            }
        }
        if !upg.target_tags.is_empty() {
            if resolved_def.is_some() {
                let ent_tags = self
                    .def_tags
                    .get(resolved_def.0 as usize)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                if !ent_tags.is_empty() {
                    if let Some(upg_did) = self.defs.lookup_def(&upg.id) {
                        if self
                            .upgrade_target_tags
                            .get(&upg_did)
                            .is_some_and(|req_tags| req_tags.iter().any(|rt| ent_tags.contains(rt)))
                        {
                            return true;
                        }
                    }
                }
            }
            if !fallback_tags.is_empty()
                && upg
                    .target_tags
                    .iter()
                    .any(|rt| fallback_tags.iter().any(|et| et == rt))
            {
                return true;
            }
        }
        false
    }

    /// Computes base weapon damage against an entity using interned `(WarheadTag, ArmorTag)` matrix
    /// with 100% fallback to legacy `ArmorType` (`Task 230`).
    pub fn effective_weapon_damage(
        &self,
        weapon: &WeaponDef,
        target_armor: ArmorType,
        target_armor_tag_str: Option<&str>,
        target_def_id: DefId,
        matrix: &DamageMatrixDef,
    ) -> f32 {
        if let Some(wh_str) = weapon.warhead_tag.as_deref() {
            if let Some(wt) = self.tags.lookup_tag(wh_str) {
                let at = if target_def_id.is_some() {
                    self.def_armor_tag
                        .get(target_def_id.0 as usize)
                        .copied()
                        .unwrap_or(TagId::NONE)
                } else if let Some(s) = target_armor_tag_str {
                    self.tags.lookup_tag(s).unwrap_or(TagId::NONE)
                } else {
                    self.tags
                        .lookup_tag(target_armor.as_tag_str())
                        .unwrap_or(TagId::NONE)
                };
                if at.is_some() {
                    if let Some(&mult) = self.damage_matrix_lut.get(&(wt, at)) {
                        return weapon.damage * mult;
                    }
                }
            }
        }
        weapon.effective_damage_with_matrix(target_armor, target_armor_tag_str, matrix)
    }
}

impl SimWorld {
    #[inline]
    pub fn mind_controlled_by(&self, id: u64) -> Option<(u64, u8)> {
        self.entities
            .get(&id)
            .and_then(|e| self.trait_stores.mind_controlled_by(e))
    }

    pub fn set_mind_controlled_by(&mut self, id: u64, val: Option<(u64, u8)>) {
        if let Some(ent) = self.entities.get_mut(&id) {
            self.trait_stores.set_mind_controlled_by_on(ent, val);
        }
    }

    #[inline]
    pub fn disguised_as(&self, id: u64) -> Option<&(String, u8)> {
        self.entities
            .get(&id)
            .and_then(|e| self.trait_stores.disguised_as(e))
    }

    pub fn set_disguised_as(&mut self, id: u64, val: Option<(String, u8)>) {
        if let Some(ent) = self.entities.get_mut(&id) {
            self.trait_stores.set_disguised_as_on(ent, val);
        }
    }

    #[inline]
    pub fn capture_bonus_claimed(&self, id: u64) -> bool {
        self.entities
            .get(&id)
            .is_some_and(|e| self.trait_stores.capture_bonus_claimed(e))
    }

    pub fn set_capture_bonus_claimed(&mut self, id: u64, claimed: bool) {
        if let Some(ent) = self.entities.get_mut(&id) {
            self.trait_stores.set_capture_bonus_claimed_on(ent, claimed);
        }
    }

    pub fn set_temporal_progress(&mut self, id: u64, progress: f32) {
        if let Some(ent) = self.entities.get_mut(&id) {
            self.trait_stores.set_temporal_progress_on(ent, progress);
        }
    }

    #[inline]
    pub fn boarding_carrier_id(&self, id: u64) -> Option<u64> {
        self.entities
            .get(&id)
            .and_then(|e| self.trait_stores.boarding_carrier_id(e))
    }

    pub fn set_boarding_carrier_id(&mut self, id: u64, val: Option<u64>) {
        if let Some(ent) = self.entities.get_mut(&id) {
            self.trait_stores.set_boarding_carrier_id_on(ent, val);
        }
    }

    #[inline]
    pub fn parent_carrier_id(&self, id: u64) -> Option<u64> {
        self.entities
            .get(&id)
            .and_then(|e| self.trait_stores.parent_carrier_id(e))
    }

    pub fn set_parent_carrier_id(&mut self, id: u64, val: Option<u64>) {
        if let Some(ent) = self.entities.get_mut(&id) {
            self.trait_stores.set_parent_carrier_id_on(ent, val);
        }
    }

    #[inline]
    pub fn carryall_pickup_target_id(&self, id: u64) -> Option<u64> {
        self.entities
            .get(&id)
            .and_then(|e| self.trait_stores.carryall_pickup_target_id(e))
    }

    pub fn set_carryall_pickup_target_id(&mut self, id: u64, val: Option<u64>) {
        if let Some(ent) = self.entities.get_mut(&id) {
            self.trait_stores.set_carryall_pickup_target_id_on(ent, val);
        }
    }

    #[inline]
    pub fn home_airfield_id(&self, id: u64) -> Option<u64> {
        self.entities
            .get(&id)
            .and_then(|e| self.trait_stores.home_airfield_id(e))
    }

    pub fn set_home_airfield_id(&mut self, id: u64, val: Option<u64>) {
        if let Some(ent) = self.entities.get_mut(&id) {
            self.trait_stores.set_home_airfield_id_on(ent, val);
        }
    }

    #[inline]
    pub fn service_depot_target_id(&self, id: u64) -> Option<u64> {
        self.entities
            .get(&id)
            .and_then(|e| self.trait_stores.service_depot_target_id(e))
    }

    pub fn set_service_depot_target_id(&mut self, id: u64, val: Option<u64>) {
        if let Some(ent) = self.entities.get_mut(&id) {
            self.trait_stores.set_service_depot_target_id_on(ent, val);
        }
    }

    #[inline]
    pub fn salvage_upgraded(&self, id: u64) -> bool {
        self.entities
            .get(&id)
            .is_some_and(|e| self.trait_stores.salvage_upgraded(e))
    }

    pub fn set_salvage_upgraded(&mut self, id: u64, val: bool) {
        if let Some(ent) = self.entities.get_mut(&id) {
            self.trait_stores.set_salvage_upgraded_on(ent, val);
        }
    }

    #[inline]
    pub fn point_defense_timer(&self, id: u64) -> f32 {
        self.entities
            .get(&id)
            .map(|e| self.trait_stores.point_defense_timer(e))
            .unwrap_or(0.0)
    }

    pub fn set_point_defense_timer(&mut self, id: u64, val: f32) {
        if let Some(ent) = self.entities.get_mut(&id) {
            self.trait_stores.set_point_defense_timer_on(ent, val);
        }
    }

    #[inline]
    pub fn planted_c4_owner(&self, id: u64) -> Option<u8> {
        self.entities
            .get(&id)
            .and_then(|e| self.trait_stores.planted_c4_owner(e))
    }

    pub fn set_planted_c4_owner(&mut self, id: u64, owner: Option<u8>) {
        if let Some(ent) = self.entities.get_mut(&id) {
            self.trait_stores.set_planted_c4_owner_on(ent, owner);
        }
    }

    #[inline]
    pub fn pending_airstrike(&self, id: u64) -> Option<(f32, f32, u8)> {
        self.entities
            .get(&id)
            .and_then(|e| self.trait_stores.pending_airstrike(e))
    }

    pub fn set_pending_airstrike(&mut self, id: u64, timer: f32, damage: f32, attacker: u8) {
        if let Some(ent) = self.entities.get_mut(&id) {
            self.trait_stores
                .set_pending_airstrike_on(ent, timer, damage, attacker);
        }
    }

    pub fn clear_pending_airstrike(&mut self, id: u64) {
        self.set_pending_airstrike(id, 0.0, 0.0, 0);
    }

    #[inline]
    pub fn hero_state(&self, id: u64) -> Option<&HeroInstanceState> {
        self.trait_stores.heroes.get(&id)
    }

    #[inline]
    pub fn energy_pool(&self, id: u64) -> Option<&EnergyPoolState> {
        self.trait_stores.energy_pools.get(&id)
    }

    #[inline]
    pub fn local_research_queue(
        &self,
        building_entity_id: u64,
    ) -> Option<&VecDeque<LocalResearchItem>> {
        self.trait_stores
            .production
            .get(&building_entity_id)
            .map(|p| &p.local_research_queue)
    }

    /// Resolves effective armor tag `&str` for `ent` from `LoadedMod` (`Task 230`).
    pub fn entity_armor_tag<'a>(&self, ent: &'a SimEntity, loaded: &'a LoadedMod) -> &'a str {
        match &ent.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| u.effective_armor_tag())
                .unwrap_or_else(|| ent.armor.as_tag_str()),
            EntityKind::Building { building_id } => loaded
                .buildings
                .get(building_id)
                .map(|b| b.effective_armor_tag())
                .unwrap_or_else(|| ent.armor.as_tag_str()),
        }
    }

    /// Computes base weapon damage against `target` using `ModRules.damage_matrix` (`ArmorTag` x `WarheadTag`)
    /// with 100% fallback to legacy `ArmorType` (`Task 230`).
    pub fn weapon_base_damage_vs_entity(
        &self,
        weapon: &WeaponDef,
        target: &SimEntity,
        loaded: &LoadedMod,
    ) -> f32 {
        let armor_tag = self.entity_armor_tag(target, loaded);
        self.intern_table.effective_weapon_damage(
            weapon,
            target.armor,
            Some(armor_tag),
            target.def_id_interned,
            &loaded.manifest.rules.damage_matrix,
        )
    }
}
