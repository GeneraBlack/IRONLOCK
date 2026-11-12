use super::models_core::{
    sanitize_replay_reference, CommanderRankEntry, CreateMatchRequest, MatchPlayerRecord,
    MatchRecord,
};
use super::state_lobbies::RegistryState;
use crate::player_identity::{
    constant_time_eq_str, derive_player_id_from_public_token,
    verify_signed_match_report_with_authority, SignedMatchReport,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub(super) struct PlayerBindingRecord {
    pub(super) player_id: String,
    pub(super) display_name: String,
    pub(super) public_token: String,
}

#[derive(Debug, Clone)]
pub(super) struct PlayerMmrRecord {
    pub(super) display_name: String,
    pub(super) player_id: Option<String>,
    pub(super) rating: f64,
    pub(super) rd: f64,
    pub(super) sigma: f64,
    pub(super) wins: u32,
    pub(super) losses: u32,
    pub(super) streak: i32,
    pub(super) faction_counts: BTreeMap<String, u32>,
}

impl PlayerMmrRecord {
    pub(super) fn new(display_name: String, player_id: Option<String>) -> Self {
        Self {
            display_name,
            player_id,
            rating: 1500.0,
            rd: 350.0,
            sigma: 0.06,
            wins: 0,
            losses: 0,
            streak: 0,
            faction_counts: BTreeMap::new(),
        }
    }

    pub(super) fn favorite_faction(&self) -> String {
        self.faction_counts
            .iter()
            .max_by_key(|(_, &cnt)| cnt)
            .map(|(f, _)| f.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    }
}

/// Computes a genuine Glickman Glicko-2 rating update against an opponent `(opp_rating, opp_rd)` with outcome `score` (`1.0`, `0.5`, or `0.0`).
pub fn glicko2_update_single(
    rating: f64,
    rd: f64,
    sigma: f64,
    opp_rating: f64,
    opp_rd: f64,
    score: f64,
) -> (f64, f64, f64) {
    const SCALE: f64 = 173.7178;
    const TAU: f64 = 0.5;
    let mu = (rating - 1500.0) / SCALE;
    let phi = (rd.clamp(30.0, 350.0)) / SCALE;
    let mu_j = (opp_rating - 1500.0) / SCALE;
    let phi_j = (opp_rd.clamp(30.0, 350.0)) / SCALE;

    let g_phi =
        1.0 / (1.0 + 3.0 * phi_j * phi_j / (std::f64::consts::PI * std::f64::consts::PI)).sqrt();
    let e_val = 1.0 / (1.0 + (-g_phi * (mu - mu_j)).exp());
    let v = 1.0 / (g_phi * g_phi * e_val * (1.0 - e_val)).max(1e-9);
    let delta = v * g_phi * (score - e_val);

    let a = (sigma * sigma).ln();
    let f = |x: f64| -> f64 {
        let ex = x.exp();
        let num = ex * (delta * delta - phi * phi - v - ex);
        let den = 2.0 * (phi * phi + v + ex).powi(2);
        (num / den) - ((x - a) / (TAU * TAU))
    };

    let mut upper_a = a;
    let mut upper_b = if delta * delta > phi * phi + v {
        (delta * delta - phi * phi - v).ln()
    } else {
        let mut k = 1.0;
        while f(a - k * TAU) < 0.0 && k < 50.0 {
            k += 1.0;
        }
        a - k * TAU
    };

    let mut f_a = f(upper_a);
    let mut f_b = f(upper_b);
    for _ in 0..40 {
        if (upper_b - upper_a).abs() <= 1e-6 {
            break;
        }
        let denom = f_b - f_a;
        if denom.abs() < 1e-12 {
            break;
        }
        let c = upper_a + (upper_a - upper_b) * f_a / denom;
        let f_c = f(c);
        if f_c * f_b <= 0.0 {
            upper_a = upper_b;
            f_a = f_b;
        } else {
            f_a *= 0.5;
        }
        upper_b = c;
        f_b = f_c;
    }

    let new_sigma = (upper_a * 0.5).exp().clamp(0.01, 0.25);
    let phi_star = (phi * phi + new_sigma * new_sigma).sqrt();
    let new_phi = 1.0 / ((1.0 / (phi_star * phi_star)) + (1.0 / v)).sqrt();
    let new_mu = mu + new_phi * new_phi * g_phi * (score - e_val);

    let new_rating = (1500.0 + SCALE * new_mu).clamp(100.0, 4000.0);
    let new_rd = (SCALE * new_phi).clamp(30.0, 350.0);
    (new_rating, new_rd, new_sigma)
}

impl RegistryState {
    pub(super) fn register_player_binding(
        &mut self,
        player_id: &str,
        display_name: &str,
        public_token: &str,
    ) -> std::result::Result<(), (u16, String)> {
        let pid = player_id.trim();
        let name = display_name.trim();
        let pub_tok = public_token.trim();
        if pid.is_empty() || name.is_empty() || pub_tok.len() < 32 {
            return Err((
                400,
                "Invalid player_id, display_name, or public_token".to_string(),
            ));
        }
        let derived_id = derive_player_id_from_public_token(pub_tok);
        if !constant_time_eq_str(pid, &derived_id) {
            return Err((
                401,
                "player_id does not match cryptographic public_token".to_string(),
            ));
        }

        let name_lower = name.to_ascii_lowercase();
        if let Some(existing_owner_id) = self.player_id_by_name_lower.get(&name_lower) {
            if existing_owner_id != pid {
                return Err((
                    403,
                    format!(
                        "Commander name '{name}' is already cryptographically bound to another player profile"
                    ),
                ));
            }
        }
        if let Some(existing_rec) = self.player_bindings_by_id.get(pid) {
            if existing_rec.player_id != pid
                || !constant_time_eq_str(&existing_rec.public_token, pub_tok)
            {
                return Err((401, "Public token mismatch for player_id".to_string()));
            }
            let prev_lower = existing_rec.display_name.to_ascii_lowercase();
            if prev_lower != name_lower {
                self.player_id_by_name_lower.remove(&prev_lower);
            }
        }

        self.player_id_by_name_lower
            .insert(name_lower.clone(), pid.to_string());
        self.player_bindings_by_id.insert(
            pid.to_string(),
            PlayerBindingRecord {
                player_id: pid.to_string(),
                display_name: name.to_string(),
                public_token: pub_tok.to_string(),
            },
        );
        let entry = self
            .mmr_by_name_lower
            .entry(name_lower)
            .or_insert_with(|| PlayerMmrRecord::new(name.to_string(), Some(pid.to_string())));
        entry.display_name = name.to_string();
        entry.player_id = Some(pid.to_string());
        Ok(())
    }

    pub(super) fn update_mmr_for_record(&mut self, record: &MatchRecord) {
        if record.desync_detected {
            return;
        }
        let mut seen = BTreeSet::new();
        let mut unique_players = Vec::new();
        for p in &record.players {
            let trimmed = p.name.trim();
            if trimmed.is_empty() {
                continue;
            }
            let lower = trimmed.to_ascii_lowercase();
            if seen.insert(lower) {
                unique_players.push(p.clone());
            }
        }
        if unique_players.is_empty() {
            return;
        }

        let winners: Vec<&MatchPlayerRecord> =
            unique_players.iter().filter(|p| p.is_winner).collect();
        let losers: Vec<&MatchPlayerRecord> =
            unique_players.iter().filter(|p| !p.is_winner).collect();

        let snapshot_for =
            |name: &str, map: &BTreeMap<String, PlayerMmrRecord>| -> (f64, f64, f64) {
                let lower = name.trim().to_ascii_lowercase();
                map.get(&lower)
                    .map(|r| (r.rating, r.rd, r.sigma))
                    .unwrap_or((1500.0, 350.0, 0.06))
            };

        let has_competitive_outcome = !winners.is_empty() && !losers.is_empty();
        let (win_avg_r, win_avg_rd, lose_avg_r, lose_avg_rd) = if has_competitive_outcome {
            let wr: f64 = winners
                .iter()
                .map(|p| snapshot_for(&p.name, &self.mmr_by_name_lower).0)
                .sum::<f64>()
                / (winners.len() as f64);
            let wrd: f64 = winners
                .iter()
                .map(|p| snapshot_for(&p.name, &self.mmr_by_name_lower).1)
                .sum::<f64>()
                / (winners.len() as f64);
            let lr: f64 = losers
                .iter()
                .map(|p| snapshot_for(&p.name, &self.mmr_by_name_lower).0)
                .sum::<f64>()
                / (losers.len() as f64);
            let lrd: f64 = losers
                .iter()
                .map(|p| snapshot_for(&p.name, &self.mmr_by_name_lower).1)
                .sum::<f64>()
                / (losers.len() as f64);
            (wr, wrd, lr, lrd)
        } else {
            (1500.0, 350.0, 1500.0, 350.0)
        };

        for p in &unique_players {
            let name_trim = p.name.trim();
            let lower = name_trim.to_ascii_lowercase();
            let bound_pid = self.player_id_by_name_lower.get(&lower).cloned();
            let pre = snapshot_for(name_trim, &self.mmr_by_name_lower);
            let entry = self
                .mmr_by_name_lower
                .entry(lower)
                .or_insert_with(|| PlayerMmrRecord::new(name_trim.to_string(), bound_pid.clone()));
            if entry.player_id.is_none() {
                entry.player_id = bound_pid;
            }
            if !p.faction.trim().is_empty() {
                *entry
                    .faction_counts
                    .entry(p.faction.trim().to_string())
                    .or_insert(0) += 1;
            }
            if p.is_winner {
                entry.wins += 1;
                entry.streak = if entry.streak > 0 {
                    entry.streak + 1
                } else {
                    1
                };
                if has_competitive_outcome {
                    let (nr, nrd, nsig) =
                        glicko2_update_single(pre.0, pre.1, pre.2, lose_avg_r, lose_avg_rd, 1.0);
                    entry.rating = (nr * 100.0).round() / 100.0;
                    entry.rd = (nrd * 100.0).round() / 100.0;
                    entry.sigma = nsig;
                }
            } else {
                entry.losses += 1;
                entry.streak = if entry.streak < 0 {
                    entry.streak - 1
                } else {
                    -1
                };
                if has_competitive_outcome {
                    let (nr, nrd, nsig) =
                        glicko2_update_single(pre.0, pre.1, pre.2, win_avg_r, win_avg_rd, 0.0);
                    entry.rating = (nr * 100.0).round() / 100.0;
                    entry.rd = (nrd * 100.0).round() / 100.0;
                    entry.sigma = nsig;
                }
            }
        }
    }

    pub(super) fn record_match(&mut self, req: CreateMatchRequest, now_ms: u64) -> MatchRecord {
        let match_id = format!("match-{}", self.next_match_id);
        self.next_match_id += 1;

        let map_id = if req.map_id.trim().is_empty() {
            "skirmish_valley".to_string()
        } else {
            req.map_id.trim().to_string()
        };
        let map_name = if req.map_name.trim().is_empty() {
            map_id.clone()
        } else {
            req.map_name.trim().to_string()
        };
        let duration_secs = if req.duration_secs > 0 {
            req.duration_secs
        } else if req.final_tick > 0 {
            (req.final_tick / 20).max(1)
        } else {
            0
        };
        let raw_hash = req.final_state_hash.trim();
        let final_state_hash = if raw_hash.is_empty() {
            "0x0".to_string()
        } else if raw_hash.starts_with("0x") || raw_hash.starts_with("0X") {
            raw_hash.to_string()
        } else {
            format!("0x{raw_hash}")
        };

        let record = MatchRecord {
            match_id,
            lobby_id: req.lobby_id,
            lobby_name: if req.lobby_name.trim().is_empty() {
                "Skirmish Match".to_string()
            } else {
                req.lobby_name.trim().to_string()
            },
            mod_id: if req.mod_id.trim().is_empty() {
                "rust-ra2".to_string()
            } else {
                req.mod_id.trim().to_string()
            },
            mod_version: if req.mod_version.trim().is_empty() {
                "1.0.0".to_string()
            } else {
                req.mod_version.trim().to_string()
            },
            map_id,
            map_name,
            game_mode: if req.game_mode.trim().is_empty() {
                "Standard".to_string()
            } else {
                req.game_mode.trim().to_string()
            },
            players: req.players,
            winner_player: if req.winner_player.trim().is_empty() {
                "Draw".to_string()
            } else {
                req.winner_player.trim().to_string()
            },
            winner_faction: if req.winner_faction.trim().is_empty() {
                "Unknown".to_string()
            } else {
                req.winner_faction.trim().to_string()
            },
            duration_secs,
            final_tick: req.final_tick,
            final_state_hash,
            total_commands: req.total_commands,
            desync_detected: req.desync_detected,
            replay_file: sanitize_replay_reference(req.replay_file),
            completed_at_unix_ms: now_ms,
        };
        self.update_mmr_for_record(&record);
        self.matches.insert(0, record.clone());
        if self.matches.len() > 250 {
            self.matches.truncate(250);
        }
        record
    }

    pub(super) fn record_signed_match(
        &mut self,
        report: SignedMatchReport,
        server_token_header: Option<&str>,
        now_ms: u64,
    ) -> std::result::Result<MatchRecord, (u16, String)> {
        let is_dedicated_auth = self.is_registered_dedicated_authority(
            &report.player_id,
            &report.public_token,
            server_token_header,
        );
        if let Err(err) = verify_signed_match_report_with_authority(&report, is_dedicated_auth) {
            return Err((
                401,
                format!("Cryptographic signature verification failed: {err}"),
            ));
        }
        let sig_lower = report.signature.trim().to_ascii_lowercase();
        if self.used_match_signatures.contains(&sig_lower) {
            return Err((
                409,
                "Replay of previously submitted signed match report rejected".to_string(),
            ));
        }

        if !is_dedicated_auth {
            self.register_player_binding(
                &report.player_id,
                &report.submitter_name,
                &report.public_token,
            )?;
        }

        self.used_match_signatures.insert(sig_lower);
        if self.used_match_signatures.len() > 2048 {
            if let Some(first) = self.used_match_signatures.iter().next().cloned() {
                self.used_match_signatures.remove(&first);
            }
        }
        Ok(self.record_match(report.match_request, now_ms))
    }

    pub(super) fn build_leaderboard(&self) -> Vec<CommanderRankEntry> {
        let mut rows: Vec<CommanderRankEntry> = self
            .mmr_by_name_lower
            .values()
            .filter(|r| r.wins + r.losses > 0)
            .map(|r| CommanderRankEntry {
                player: r.display_name.clone(),
                wins: r.wins,
                losses: r.losses,
                games: r.wins + r.losses,
                favorite_faction: r.favorite_faction(),
                rating: r.rating,
                rd: r.rd,
                streak: r.streak,
                player_id: r.player_id.clone(),
            })
            .collect();
        rows.sort_by(|a, b| {
            b.rating
                .partial_cmp(&a.rating)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.wins.cmp(&a.wins))
                .then_with(|| a.losses.cmp(&b.losses))
        });
        rows
    }
}
