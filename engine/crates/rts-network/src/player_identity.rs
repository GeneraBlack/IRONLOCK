use crate::lobby_registry::{CreateMatchRequest, MatchPlayerRecord};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_PLAYER_IDENTITY_PATH: &str = "saves/player_identity.toml";

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

const SHA256_H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// Pure-Rust FIPS 180-4 SHA-256 digest (zero C or external crate dependencies).
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut state = SHA256_H0;

    let full_chunks = data.len() / 64;
    for chunk_idx in 0..full_chunks {
        let block = &data[chunk_idx * 64..(chunk_idx + 1) * 64];
        sha256_compress(&mut state, block);
    }

    let rem = &data[full_chunks * 64..];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut tail = [0u8; 128];
    tail[..rem.len()].copy_from_slice(rem);
    tail[rem.len()] = 0x80;

    if rem.len() < 56 {
        tail[56..64].copy_from_slice(&bit_len.to_be_bytes());
        sha256_compress(&mut state, &tail[..64]);
    } else {
        tail[120..128].copy_from_slice(&bit_len.to_be_bytes());
        sha256_compress(&mut state, &tail[..64]);
        sha256_compress(&mut state, &tail[64..128]);
    }

    let mut out = [0u8; 32];
    for (i, word) in state.iter().enumerate() {
        out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn sha256_compress(state: &mut [u32; 8], block: &[u8]) {
    let mut w = [0u32; 64];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([
            block[i * 4],
            block[i * 4 + 1],
            block[i * 4 + 2],
            block[i * 4 + 3],
        ]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }

    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];
    let mut e = state[4];
    let mut f = state[5];
    let mut g = state[6];
    let mut h = state[7];

    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let temp1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(SHA256_K[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = s0.wrapping_add(maj);

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

/// Encodes bytes as lowercase hexadecimal.
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Computes lowercase hex SHA-256 digest of `data`.
pub fn sha256_hex(data: &[u8]) -> String {
    bytes_to_hex(&sha256(data))
}

/// Pure-Rust RFC 2104 HMAC-SHA256 message authentication code.
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut key_block = [0u8; 64];
    if key.len() > 64 {
        let hashed = sha256(key);
        key_block[..32].copy_from_slice(&hashed);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= key_block[i];
        opad[i] ^= key_block[i];
    }

    let mut inner = Vec::with_capacity(64 + data.len());
    inner.extend_from_slice(&ipad);
    inner.extend_from_slice(data);
    let inner_digest = sha256(&inner);

    let mut outer = [0u8; 96];
    outer[..64].copy_from_slice(&opad);
    outer[64..96].copy_from_slice(&inner_digest);
    sha256(&outer)
}

/// Computes lowercase hex HMAC-SHA256 digest.
pub fn hmac_sha256_hex(key: &[u8], data: &[u8]) -> String {
    bytes_to_hex(&hmac_sha256(key, data))
}

/// Constant-time byte comparison with zero early-exit branches.
pub fn constant_time_eq_bytes(a: &[u8], b: &[u8]) -> bool {
    let max_len = a.len().max(b.len()).max(1);
    let mut diff = a.len() ^ b.len();
    for i in 0..max_len {
        let byte_a = a.get(i).copied().unwrap_or(0);
        let byte_b = b.get(i).copied().unwrap_or(0);
        diff |= usize::from(byte_a ^ byte_b);
    }
    diff == 0
}

/// Constant-time string comparison.
pub fn constant_time_eq_str(a: &str, b: &str) -> bool {
    constant_time_eq_bytes(a.as_bytes(), b.as_bytes())
}

/// Deterministically derives `public_token` from `secret_key`.
pub fn derive_public_token(secret_key: &str) -> String {
    hmac_sha256_hex(secret_key.as_bytes(), b"ironlock:player-public-token:v1")
}

/// Deterministically derives `player_id` (`plr-<16 hex>`) from `public_token`.
pub fn derive_player_id_from_token(public_token: &str) -> String {
    let digest = sha256_hex(format!("ironlock:player-id:v1:{}", public_token.trim()).as_bytes());
    format!("plr-{}", &digest[..16])
}

/// Alias of [`derive_player_id_from_token`] used by registry binding verification.
pub fn derive_player_id_from_public_token(public_token: &str) -> String {
    derive_player_id_from_token(public_token)
}

fn generate_os_secret_key_hex() -> String {
    let mut entropy = Vec::with_capacity(96);
    let now_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    entropy.extend_from_slice(&now_ns.to_le_bytes());
    entropy.extend_from_slice(&std::process::id().to_le_bytes());
    for round in 0u64..8 {
        let rs = RandomState::new();
        let mut hasher = rs.build_hasher();
        hasher.write_u64(round ^ (now_ns as u64));
        entropy.extend_from_slice(&hasher.finish().to_le_bytes());
    }
    sha256_hex(&entropy)
}

/// Persistent cryptographic commander identity (`saves/player_identity.toml`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerIdentityProfile {
    pub player_id: String,
    pub display_name: String,
    pub public_token: String,
    pub secret_key: String,
}

impl PlayerIdentityProfile {
    /// Generates a fresh cryptographic commander identity for `display_name`.
    pub fn generate(display_name: impl Into<String>) -> Self {
        let secret_key = generate_os_secret_key_hex();
        Self::from_secret_key(display_name, secret_key)
    }

    /// Deterministically constructs a [`PlayerIdentityProfile`] from an existing `secret_key`.
    pub fn from_secret_key(display_name: impl Into<String>, secret_key: impl Into<String>) -> Self {
        let clean_name = {
            let raw = display_name.into();
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                "Commander".to_string()
            } else {
                trimmed.chars().take(48).collect()
            }
        };
        let secret_key = secret_key.into().trim().to_ascii_lowercase();
        let public_token = derive_public_token(&secret_key);
        let player_id = derive_player_id_from_token(&public_token);
        Self {
            player_id,
            display_name: clean_name,
            public_token,
            secret_key,
        }
    }

    /// Default persistence path (`saves/player_identity.toml`).
    pub fn default_path() -> PathBuf {
        PathBuf::from(DEFAULT_PLAYER_IDENTITY_PATH)
    }

    /// Verifies internal key-derivation consistency (`secret_key -> public_token -> player_id`).
    pub fn verify_self(&self) -> bool {
        if self.secret_key.len() < 32 || self.public_token.len() != 64 {
            return false;
        }
        let expected_pub = derive_public_token(&self.secret_key);
        let expected_id = derive_player_id_from_token(&expected_pub);
        constant_time_eq_str(&self.public_token, &expected_pub)
            && constant_time_eq_str(&self.player_id, &expected_id)
    }

    /// Serializes the profile to canonical `.toml` text.
    pub fn to_toml_string(&self) -> String {
        format!(
            "# IronLock RTS Cryptographic Commander Identity\n\
             player_id = \"{}\"\n\
             display_name = \"{}\"\n\
             public_token = \"{}\"\n\
             secret_key = \"{}\"\n",
            escape_toml_basic_string(&self.player_id),
            escape_toml_basic_string(&self.display_name),
            escape_toml_basic_string(&self.public_token),
            escape_toml_basic_string(&self.secret_key),
        )
    }

    /// Parses a [`PlayerIdentityProfile`] from `.toml` text and validates its cryptographic binding.
    pub fn from_toml_str(raw: &str) -> Result<Self> {
        let mut player_id: Option<String> = None;
        let mut display_name: Option<String> = None;
        let mut public_token: Option<String> = None;
        let mut secret_key: Option<String> = None;

        for (line_idx, raw_line) in raw.lines().enumerate() {
            let trimmed = raw_line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
                continue;
            }
            let Some((k, v)) = trimmed.split_once('=') else {
                bail!("Invalid TOML syntax at line {}", line_idx + 1);
            };
            let key = k.trim();
            let val = parse_toml_string_value(v.trim())
                .with_context(|| format!("Invalid TOML string value at line {}", line_idx + 1))?;
            match key {
                "player_id" => player_id = Some(val),
                "display_name" => display_name = Some(val),
                "public_token" => public_token = Some(val),
                "secret_key" => secret_key = Some(val),
                _ => {}
            }
        }

        let secret = secret_key.context("Missing 'secret_key' in player_identity.toml")?;
        let name = display_name.unwrap_or_else(|| "Commander".to_string());
        let expected_pub = derive_public_token(&secret);
        let pub_tok = public_token.unwrap_or_else(|| expected_pub.clone());
        let expected_id = derive_player_id_from_token(&pub_tok);
        let pid = player_id.unwrap_or_else(|| expected_id.clone());

        let profile = Self {
            player_id: pid,
            display_name: name,
            public_token: pub_tok,
            secret_key: secret,
        };
        if !profile.verify_self() {
            bail!("Corrupted or mismatched cryptographic tokens in player_identity.toml");
        }
        Ok(profile)
    }

    /// Atomically persists the profile to `path` (`temp` write + rename).
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create directory {}", parent.display()))?;
            }
        }
        let tmp_path = path.with_extension(format!("toml.tmp.{}", std::process::id()));
        std::fs::write(&tmp_path, self.to_toml_string())
            .with_context(|| format!("Failed to write temp identity {}", tmp_path.display()))?;
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
        std::fs::rename(&tmp_path, path).with_context(|| {
            format!(
                "Failed to atomically rename {} -> {}",
                tmp_path.display(),
                path.display()
            )
        })?;
        Ok(())
    }

    /// Loads an existing profile from `path`.
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        Self::from_toml_str(&raw)
    }

    /// Loads `path` if present and valid, otherwise generates a new profile and saves it atomically.
    pub fn load_or_create(
        path: impl AsRef<Path>,
        default_display_name: impl Into<String>,
    ) -> Result<Self> {
        let path = path.as_ref();
        if path.exists() {
            if let Ok(existing) = Self::load_from_file(path) {
                return Ok(existing);
            }
        }
        let created = Self::generate(default_display_name);
        created.save_to_file(path)?;
        Ok(created)
    }
}

fn escape_toml_basic_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

fn parse_toml_string_value(raw: &str) -> Result<String> {
    let without_comment = strip_unquoted_toml_comment(raw).trim();
    if without_comment.starts_with('"')
        && without_comment.ends_with('"')
        && without_comment.len() >= 2
    {
        let inner = &without_comment[1..without_comment.len() - 1];
        let mut out = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                match chars.next() {
                    Some('\\') => out.push('\\'),
                    Some('"') => out.push('"'),
                    Some('n') => out.push('\n'),
                    Some('r') => out.push('\r'),
                    Some('t') => out.push('\t'),
                    Some(other) => out.push(other),
                    None => bail!("Trailing backslash in TOML string"),
                }
            } else {
                out.push(ch);
            }
        }
        Ok(out)
    } else if without_comment.starts_with('\'')
        && without_comment.ends_with('\'')
        && without_comment.len() >= 2
    {
        Ok(without_comment[1..without_comment.len() - 1].to_string())
    } else {
        bail!("Expected quoted TOML string")
    }
}

fn strip_unquoted_toml_comment(s: &str) -> &str {
    let mut in_quotes = false;
    let mut escaped = false;
    for (idx, ch) in s.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_quotes {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_quotes = !in_quotes;
            continue;
        }
        if ch == '#' && !in_quotes {
            return &s[..idx];
        }
    }
    s
}

/// Cryptographically signed match report submitted to `POST /api/v1/matches`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignedMatchReport {
    #[serde(flatten)]
    pub match_request: CreateMatchRequest,
    pub player_id: String,
    pub submitter_name: String,
    pub public_token: String,
    pub timestamp_unix_ms: u64,
    pub signature: String,
}

fn clean_canonical_field(s: &str) -> String {
    s.trim()
        .chars()
        .filter(|&c| c != '\r' && c != '\n' && c != '|')
        .collect()
}

/// Builds the exact deterministic canonical string hashed by both Rust (`rts-network`)
/// and TypeScript (`centralwebserver`) for HMAC-SHA256 match report verification.
pub fn canonical_match_report_payload(
    req: &CreateMatchRequest,
    player_id: &str,
    submitter_name: &str,
    timestamp_unix_ms: u64,
) -> String {
    let mut out = String::with_capacity(512);
    out.push_str("ironlock-match-v1\n");
    out.push_str(&format!("player_id={}\n", clean_canonical_field(player_id)));
    out.push_str(&format!(
        "submitter_name={}\n",
        clean_canonical_field(submitter_name)
    ));
    out.push_str(&format!("timestamp_unix_ms={timestamp_unix_ms}\n"));
    out.push_str(&format!(
        "lobby_id={}\n",
        clean_canonical_field(req.lobby_id.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!("mod_id={}\n", clean_canonical_field(&req.mod_id)));
    out.push_str(&format!("map_id={}\n", clean_canonical_field(&req.map_id)));
    out.push_str(&format!(
        "game_mode={}\n",
        clean_canonical_field(&req.game_mode)
    ));
    out.push_str(&format!("final_tick={}\n", req.final_tick));
    out.push_str(&format!(
        "final_state_hash={}\n",
        clean_canonical_field(&req.final_state_hash).to_ascii_lowercase()
    ));
    out.push_str(&format!("total_commands={}\n", req.total_commands));
    out.push_str(&format!(
        "desync_detected={}\n",
        if req.desync_detected { 1 } else { 0 }
    ));
    out.push_str(&format!(
        "winner_player={}\n",
        clean_canonical_field(&req.winner_player)
    ));
    out.push_str(&format!(
        "winner_faction={}\n",
        clean_canonical_field(&req.winner_faction)
    ));
    out.push_str(&format!("players_count={}\n", req.players.len()));
    for p in &req.players {
        append_canonical_player(&mut out, p);
    }
    out
}

fn append_canonical_player(out: &mut String, p: &MatchPlayerRecord) {
    out.push_str(&format!(
        "player={}|{}|{}|{}\n",
        clean_canonical_field(&p.name),
        clean_canonical_field(&p.faction),
        p.team,
        if p.is_winner { 1 } else { 0 }
    ));
}

/// Signs `req` deterministically with `profile` using pure-Rust HMAC-SHA256.
pub fn sign_match_report(
    req: &CreateMatchRequest,
    profile: &PlayerIdentityProfile,
) -> SignedMatchReport {
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(1);
    sign_match_report_at(req, profile, ts_ms)
}

/// Signs `req` at an explicit `timestamp_unix_ms` (deterministic for tests and replay verification).
pub fn sign_match_report_at(
    req: &CreateMatchRequest,
    profile: &PlayerIdentityProfile,
    timestamp_unix_ms: u64,
) -> SignedMatchReport {
    let canonical = canonical_match_report_payload(
        req,
        &profile.player_id,
        &profile.display_name,
        timestamp_unix_ms,
    );
    let signature = hmac_sha256_hex(profile.public_token.as_bytes(), canonical.as_bytes());
    SignedMatchReport {
        match_request: req.clone(),
        player_id: profile.player_id.clone(),
        submitter_name: profile.display_name.clone(),
        public_token: profile.public_token.clone(),
        timestamp_unix_ms,
        signature,
    }
}

/// Verifies a [`SignedMatchReport`]'s `player_id <-> public_token` derivation, participant membership,
/// and HMAC-SHA256 signature in constant time.
pub fn verify_signed_match_report(report: &SignedMatchReport) -> Result<()> {
    verify_signed_match_report_with_authority(report, false)
}

/// Verifies a [`SignedMatchReport`], optionally permitting an authenticated external authority
/// (such as a registered neutral `rts-dedicated-server` fleet node) whose `submitter_name` is not
/// one of the human players in `report.match_request.players`.
pub fn verify_signed_match_report_with_authority(
    report: &SignedMatchReport,
    allow_external_authority: bool,
) -> Result<()> {
    let pub_tok = report.public_token.trim();
    let sig_lower = report.signature.trim().to_ascii_lowercase();
    if pub_tok.len() != 64 || sig_lower.len() != 64 {
        bail!("Malformed public_token or signature length");
    }
    let expected_pid = derive_player_id_from_token(pub_tok);
    if !constant_time_eq_str(report.player_id.trim(), &expected_pid) {
        bail!("player_id does not match public_token derivation");
    }
    let submitter_trimmed = report.submitter_name.trim();
    if submitter_trimmed.is_empty() {
        bail!("submitter_name must not be empty");
    }
    if !allow_external_authority
        && !report.match_request.players.is_empty()
        && !report
            .match_request
            .players
            .iter()
            .any(|p| p.name.trim().eq_ignore_ascii_case(submitter_trimmed))
    {
        bail!("submitter_name is not listed as a match participant");
    }
    let canonical = canonical_match_report_payload(
        &report.match_request,
        &report.player_id,
        &report.submitter_name,
        report.timestamp_unix_ms,
    );
    let expected_sig = hmac_sha256_hex(pub_tok.as_bytes(), canonical.as_bytes());
    if !constant_time_eq_str(&sig_lower, &expected_sig) {
        bail!("HMAC-SHA256 match report signature mismatch");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_and_hmac_sha256_match_nist_and_rfc4231_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );

        // RFC 4231 Test Case 2 ("Jefe", "what do ya want for nothing?")
        assert_eq!(
            hmac_sha256_hex(b"Jefe", b"what do ya want for nothing?"),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );

        // RFC 4231 Test Case 1 (20 bytes of 0x0b, "Hi There")
        let key_tc1 = [0x0bu8; 20];
        assert_eq!(
            hmac_sha256_hex(&key_tc1, b"Hi There"),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn player_identity_toml_roundtrip_and_match_signing_verification() {
        let base_tmp = std::env::var("CARGO_TARGET_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("target_net2"));
        let dir = base_tmp
            .join("tmp_tests")
            .join(format!("rts_identity_test_{}", std::process::id()));
        let toml_path = dir.join("player_identity.toml");
        let _ = std::fs::remove_dir_all(&dir);

        let profile = PlayerIdentityProfile::load_or_create(&toml_path, "Cmdr \"Vanguard\"")
            .expect("create identity");
        assert!(profile.verify_self());
        assert!(profile.player_id.starts_with("plr-"));

        let loaded_again = PlayerIdentityProfile::load_or_create(&toml_path, "IgnoredOtherName")
            .expect("reload identity");
        assert_eq!(profile, loaded_again);

        let req = CreateMatchRequest {
            lobby_id: Some("lob-42".to_string()),
            lobby_name: "Ranked 1v1".to_string(),
            mod_id: "ra2_mod".to_string(),
            mod_version: "1.0.0".to_string(),
            map_id: "twin_rivers".to_string(),
            map_name: "Twin Rivers".to_string(),
            game_mode: "1v1".to_string(),
            players: vec![
                MatchPlayerRecord {
                    name: profile.display_name.clone(),
                    faction: "allies".to_string(),
                    team: 1,
                    is_winner: true,
                },
                MatchPlayerRecord {
                    name: "Rival".to_string(),
                    faction: "soviet".to_string(),
                    team: 2,
                    is_winner: false,
                },
            ],
            winner_player: profile.display_name.clone(),
            winner_faction: "allies".to_string(),
            duration_secs: 240,
            final_tick: 4800,
            final_state_hash: "0xdeadbeefcafe1234".to_string(),
            total_commands: 312,
            desync_detected: false,
            replay_file: None,
        };

        let mut signed = sign_match_report_at(&req, &profile, 1_700_000_000_000);
        assert!(verify_signed_match_report(&signed).is_ok());

        // Tamper with winner -> signature check must fail
        signed.match_request.winner_player = "Rival".to_string();
        assert!(verify_signed_match_report(&signed).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
