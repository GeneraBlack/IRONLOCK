use rts_simulation::command::GameCommand;
use rts_simulation::world::SubsystemHashes;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesyncAlert {
    pub tick: u64,
    pub local_hash: u64,
    pub authoritative_hash: u64,
    #[serde(default)]
    pub diagnostic_notes: Vec<String>,
}

#[derive(Debug)]
pub struct LockstepSchedule {
    pub input_delay_ticks: u64,
    local_outbox: BTreeMap<u64, Vec<GameCommand>>,
    sent_ticks: BTreeSet<u64>,
    ready_bundles: BTreeMap<u64, Vec<GameCommand>>,
    pub local_hashes: BTreeMap<u64, u64>,
    pub local_subsystems: BTreeMap<u64, SubsystemHashes>,
    pub last_completed_hash: Option<(u64, u64)>,
    pub desync_alerts: Vec<DesyncAlert>,
}

impl LockstepSchedule {
    pub fn new(input_delay_ticks: u64) -> Self {
        let mut ready_bundles = BTreeMap::new();
        // Pre-seed initial delay ticks with empty command bundles so tick 0..input_delay can execute immediately
        for t in 0..input_delay_ticks {
            ready_bundles.insert(t, Vec::new());
        }
        Self {
            input_delay_ticks,
            local_outbox: BTreeMap::new(),
            sent_ticks: BTreeSet::new(),
            ready_bundles,
            local_hashes: BTreeMap::new(),
            local_subsystems: BTreeMap::new(),
            last_completed_hash: None,
            desync_alerts: Vec::new(),
        }
    }

    /// Schedules a local player command for `current_tick + input_delay_ticks`.
    pub fn queue_local_command(&mut self, current_tick: u64, cmd: GameCommand) -> u64 {
        let target_tick = current_tick.saturating_add(self.input_delay_ticks);
        self.local_outbox.entry(target_tick).or_default().push(cmd);
        target_tick
    }

    pub fn has_sent_tick(&self, target_tick: u64) -> bool {
        self.sent_ticks.contains(&target_tick)
    }

    pub fn take_outgoing_for_tick(&mut self, target_tick: u64) -> Vec<GameCommand> {
        self.sent_ticks.insert(target_tick);
        self.local_outbox.remove(&target_tick).unwrap_or_default()
    }

    pub fn insert_server_bundle(
        &mut self,
        tick: u64,
        commands: Vec<GameCommand>,
        verified_tick_hash: Option<(u64, u64)>,
    ) {
        self.ready_bundles.insert(tick, commands);
        if let Some((h_tick, auth_hash)) = verified_tick_hash {
            if let Some(&local_h) = self.local_hashes.get(&h_tick) {
                if local_h != auth_hash {
                    let mut diagnostic_notes = Vec::new();
                    if let Some(sub) = self.local_subsystems.get(&h_tick) {
                        diagnostic_notes.push(format!(
                            "local subsystems @ tick {}: econ=0x{:08x} tf=0x{:08x} combat=0x{:08x} res=0x{:08x} proj=0x{:08x} trig=0x{:08x}",
                            h_tick,
                            sub.economy as u32,
                            sub.entities_transform as u32,
                            sub.entities_combat as u32,
                            sub.resources_and_mines as u32,
                            sub.projectiles as u32,
                            sub.triggers as u32,
                        ));
                    }
                    self.desync_alerts.push(DesyncAlert {
                        tick: h_tick,
                        local_hash: local_h,
                        authoritative_hash: auth_hash,
                        diagnostic_notes,
                    });
                }
            }
        }
    }

    pub fn take_ready_bundle(&mut self, tick: u64) -> Option<Vec<GameCommand>> {
        self.ready_bundles.remove(&tick)
    }

    pub fn record_completed_tick_hash(&mut self, completed_tick: u64, hash: u64) {
        self.local_hashes.insert(completed_tick, hash);
        self.last_completed_hash = Some((completed_tick, hash));
    }

    pub fn record_completed_tick_with_subsystems(
        &mut self,
        completed_tick: u64,
        hash: u64,
        subsystems: SubsystemHashes,
    ) {
        self.local_hashes.insert(completed_tick, hash);
        self.local_subsystems.insert(completed_tick, subsystems);
        self.last_completed_hash = Some((completed_tick, hash));
    }
}
