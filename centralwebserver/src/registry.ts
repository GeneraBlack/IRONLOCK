import * as crypto from 'node:crypto';
import * as fs from 'node:fs';
import * as net from 'node:net';
import * as path from 'node:path';
import {
  formatMuxEndpoint,
  maxRelayRoomsForRange,
  MultiplexedRelayOptions,
  MultiplexedRelayServer,
  RelayCapacityError,
  RelayPortRange,
  RelayRoomOptions,
  RelayTunnelRoom,
  resolveDefaultRelayPortRange,
  UdpRendezvousOptions,
  UdpRendezvousServer,
} from './relay';
import {
  CommanderRankEntry,
  CreateMatchRequest,
  LobbyAnnouncement,
  LobbyEntry,
  LobbyHeartbeatRequest,
  LobbyStatus,
  MatchPlayerRecord,
  MatchRecord,
  RegisteredLobbyResponse,
  RegistryStats,
  SignedMatchReport,
} from './types';

export class LobbyAuthError extends Error {
  public readonly statusCode: number;

  constructor(statusCode: 401 | 403 | 409, message: string) {
    super(message);
    this.name = 'LobbyAuthError';
    this.statusCode = statusCode;
  }
}

export class RegistryValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'RegistryValidationError';
  }
}

export function normalizePeerIp(ip?: string): string | undefined {
  if (!ip) return undefined;
  const trimmed = ip.trim();
  if (!trimmed) return undefined;
  if (trimmed.startsWith('::ffff:')) {
    return trimmed.slice('::ffff:'.length);
  }
  if (trimmed === '::1' || trimmed === '0.0.0.0') {
    return '127.0.0.1';
  }
  return trimmed;
}

/**
 * Detects RFC 6598 Carrier-Grade NAT (`100.64.0.0/10`), RFC 1918 private LANs,
 * loopback, link-local, and IPv6 ULA/link-local addresses.
 */
export function isCgnatOrPrivateIp(rawIp?: string): boolean {
  if (!rawIp) return true;
  let ip = rawIp.trim();
  if (!ip) return true;
  if (ip.startsWith('::ffff:')) {
    ip = ip.slice('::ffff:'.length);
  }
  const version = net.isIP(ip);
  if (version === 4) {
    const parts = ip.split('.').map((n) => Number(n));
    if (parts.length !== 4 || parts.some((n) => !Number.isInteger(n) || n < 0 || n > 255)) {
      return true;
    }
    const [a, b] = parts;
    if (a === 0 || a === 10 || a === 127) return true;
    if (a === 100 && b >= 64 && b <= 127) return true; // RFC 6598 CGNAT
    if (a === 169 && b === 254) return true; // Link-local
    if (a === 172 && b >= 16 && b <= 31) return true; // RFC 1918
    if (a === 192 && b === 168) return true; // RFC 1918
    return false;
  }
  if (version === 6) {
    const lower = ip.toLowerCase();
    if (lower === '::1' || lower === '::') return true;
    if (
      lower.startsWith('fc') ||
      lower.startsWith('fd') ||
      lower.startsWith('fe8') ||
      lower.startsWith('fe9') ||
      lower.startsWith('fea') ||
      lower.startsWith('feb')
    ) {
      return true;
    }
    return false;
  }
  return true;
}

function clamp(val: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, val));
}

function generateHostToken(): string {
  return crypto.randomBytes(32).toString('hex');
}

export function verifySecretToken(provided: string, expected: string): boolean {
  const a = Buffer.from(provided, 'utf-8');
  const b = Buffer.from(expected, 'utf-8');
  if (a.length !== b.length || a.length === 0) {
    return false;
  }
  return crypto.timingSafeEqual(a, b);
}

function hashLobbyPassword(password: string): string {
  const salt = crypto.randomBytes(16).toString('hex');
  const derived = crypto.scryptSync(password, salt, 32).toString('hex');
  return `${salt}:${derived}`;
}

function verifyLobbyPasswordHash(candidate: string, stored: string): boolean {
  const sepIdx = stored.indexOf(':');
  if (sepIdx <= 0) return false;
  const salt = stored.slice(0, sepIdx);
  const expectedHex = stored.slice(sepIdx + 1);
  const derivedHex = crypto.scryptSync(candidate, salt, 32).toString('hex');
  return verifySecretToken(derivedHex, expectedHex);
}

/**
 * Deterministically derives `public_token` from `secret_key` (matching `rts-network/src/player_identity.rs`).
 */
export function derivePublicToken(secretKey: string): string {
  return crypto
    .createHmac('sha256', Buffer.from(secretKey.trim().toLowerCase(), 'utf-8'))
    .update('ironlock:player-public-token:v1', 'utf-8')
    .digest('hex');
}

/**
 * Deterministically derives `player_id` (`plr-<16 hex>`) from `public_token` (matching `rts-network/src/player_identity.rs`).
 */
export function derivePlayerIdFromToken(publicToken: string): string {
  const digest = crypto
    .createHash('sha256')
    .update(`ironlock:player-id:v1:${publicToken.trim()}`, 'utf-8')
    .digest('hex');
  return `plr-${digest.slice(0, 16)}`;
}

export interface PlayerIdentityKeypair {
  player_id: string;
  display_name: string;
  public_token: string;
  secret_key: string;
}

export function createPlayerIdentityProfile(
  displayName: string,
  secretKey?: string
): PlayerIdentityKeypair {
  const cleanName = (displayName || '').trim().slice(0, 48) || 'Commander';
  const cleanSecret =
    secretKey && secretKey.trim().length >= 32
      ? secretKey.trim().toLowerCase()
      : crypto.randomBytes(32).toString('hex');
  const publicToken = derivePublicToken(cleanSecret);
  const playerId = derivePlayerIdFromToken(publicToken);
  return {
    player_id: playerId,
    display_name: cleanName,
    public_token: publicToken,
    secret_key: cleanSecret,
  };
}

function cleanCanonicalField(val: unknown): string {
  return String(val ?? '')
    .trim()
    .replace(/[\r\n|]/g, '');
}

/**
 * Exact canonical match report payload shared byte-for-byte with `rts-network/src/player_identity.rs`.
 */
export function canonicalMatchReportPayload(
  req: CreateMatchRequest,
  playerId: string,
  submitterName: string,
  timestampUnixMs: number
): string {
  const players = Array.isArray(req.players) ? req.players : [];
  const lines: string[] = [
    'ironlock-match-v1',
    `player_id=${cleanCanonicalField(playerId)}`,
    `submitter_name=${cleanCanonicalField(submitterName)}`,
    `timestamp_unix_ms=${Math.max(0, Math.floor(Number(timestampUnixMs) || 0))}`,
    `lobby_id=${cleanCanonicalField(req.lobby_id ?? '')}`,
    `mod_id=${cleanCanonicalField(req.mod_id ?? '')}`,
    `map_id=${cleanCanonicalField(req.map_id ?? '')}`,
    `game_mode=${cleanCanonicalField(req.game_mode ?? '')}`,
    `final_tick=${Math.max(0, Math.floor(Number(req.final_tick) || 0))}`,
    `final_state_hash=${cleanCanonicalField(req.final_state_hash ?? '').toLowerCase()}`,
    `total_commands=${Math.max(0, Math.floor(Number(req.total_commands) || 0))}`,
    `desync_detected=${req.desync_detected ? 1 : 0}`,
    `winner_player=${cleanCanonicalField(req.winner_player ?? '')}`,
    `winner_faction=${cleanCanonicalField(req.winner_faction ?? '')}`,
    `players_count=${players.length}`,
  ];
  for (const p of players) {
    const teamNum =
      typeof p.team === 'number' && Number.isFinite(p.team)
        ? Math.max(0, Math.floor(p.team))
        : 0;
    lines.push(
      `player=${cleanCanonicalField(p.name)}|${cleanCanonicalField(p.faction)}|${teamNum}|${p.is_winner ? 1 : 0}`
    );
  }
  return lines.join('\n') + '\n';
}

export function signMatchReport(
  req: CreateMatchRequest,
  profile: { player_id: string; display_name: string; public_token: string },
  timestampUnixMs = Date.now()
): SignedMatchReport {
  const ts = Math.max(0, Math.floor(Number(timestampUnixMs) || 0));
  const canonical = canonicalMatchReportPayload(
    req,
    profile.player_id,
    profile.display_name,
    ts
  );
  const signature = crypto
    .createHmac('sha256', Buffer.from(profile.public_token.trim(), 'utf-8'))
    .update(canonical, 'utf-8')
    .digest('hex');
  return {
    ...req,
    player_id: profile.player_id,
    submitter_name: profile.display_name,
    public_token: profile.public_token,
    timestamp_unix_ms: ts,
    signature,
  };
}

export function extractMatchRequestFromSignedReport(
  report: SignedMatchReport
): CreateMatchRequest {
  if (report.match_request && typeof report.match_request === 'object') {
    return report.match_request;
  }
  return {
    lobby_id: report.lobby_id,
    lobby_name: report.lobby_name,
    mod_id: report.mod_id,
    mod_name: report.mod_name,
    mod_version: report.mod_version,
    map_id: report.map_id,
    map_name: report.map_name,
    game_mode: report.game_mode,
    players: report.players,
    winner_player: report.winner_player,
    winner_faction: report.winner_faction,
    duration_secs: report.duration_secs,
    final_tick: report.final_tick,
    final_state_hash: report.final_state_hash,
    total_commands: report.total_commands,
    desync_detected: report.desync_detected,
    replay_file: report.replay_file,
    replay_url: report.replay_url,
    replayUrl: report.replayUrl,
  };
}

export function verifySignedMatchReport(
  report: SignedMatchReport,
  options?: { allowNeutralSubmitter?: boolean }
): {
  valid: boolean;
  statusCode?: 401 | 403;
  error?: string;
  matchRequest: CreateMatchRequest;
} {
  const matchRequest = extractMatchRequestFromSignedReport(report);
  const pubTok = typeof report.public_token === 'string' ? report.public_token.trim() : '';
  const sigLower =
    typeof report.signature === 'string' ? report.signature.trim().toLowerCase() : '';
  const playerId = typeof report.player_id === 'string' ? report.player_id.trim() : '';
  const submitterName =
    typeof report.submitter_name === 'string' ? report.submitter_name.trim() : '';

  if (pubTok.length !== 64 || sigLower.length !== 64 || !playerId || !submitterName) {
    return {
      valid: false,
      statusCode: 401,
      error: 'Missing or malformed cryptographic player_id, public_token, or signature',
      matchRequest,
    };
  }

  const expectedPid = derivePlayerIdFromToken(pubTok);
  if (!verifySecretToken(playerId, expectedPid)) {
    return {
      valid: false,
      statusCode: 401,
      error: 'player_id does not match cryptographic public_token derivation',
      matchRequest,
    };
  }

  const hasExplicitPlayerIds =
    Array.isArray(matchRequest.players) &&
    matchRequest.players.some(
      (p) => typeof p?.player_id === 'string' && p.player_id.trim().length > 0
    );
  const looksLikeDedicatedSubmitter =
    Boolean(options?.allowNeutralSubmitter) ||
    hasExplicitPlayerIds ||
    /\b(?:dedicated|rts-dedicated-server)\b/i.test(submitterName) ||
    submitterName.toLowerCase().startsWith('ds-');

  if (
    !looksLikeDedicatedSubmitter &&
    Array.isArray(matchRequest.players) &&
    matchRequest.players.length > 0 &&
    !matchRequest.players.some(
      (p) => String(p.name ?? '').trim().toLowerCase() === submitterName.toLowerCase()
    )
  ) {
    return {
      valid: false,
      statusCode: 403,
      error: 'submitter_name is not a participant in the reported match',
      matchRequest,
    };
  }

  const canonical = canonicalMatchReportPayload(
    matchRequest,
    playerId,
    submitterName,
    report.timestamp_unix_ms
  );
  const expectedSig = crypto
    .createHmac('sha256', Buffer.from(pubTok, 'utf-8'))
    .update(canonical, 'utf-8')
    .digest('hex');

  if (!verifySecretToken(sigLower, expectedSig)) {
    return {
      valid: false,
      statusCode: 401,
      error: 'HMAC-SHA256 match report signature verification failed',
      matchRequest,
    };
  }

  return { valid: true, matchRequest };
}

/**
 * Genuine Mark Glickman Glicko-2 single-period rating update (`rating`, `rd`, `sigma`).
 */
export function glicko2UpdateSingle(
  rating: number,
  rd: number,
  sigma: number,
  oppRating: number,
  oppRd: number,
  score: number
): { rating: number; rd: number; sigma: number } {
  const SCALE = 173.7178;
  const TAU = 0.5;
  const mu = (rating - 1500.0) / SCALE;
  const phi = clamp(rd, 30.0, 350.0) / SCALE;
  const muJ = (oppRating - 1500.0) / SCALE;
  const phiJ = clamp(oppRd, 30.0, 350.0) / SCALE;

  const gPhi =
    1.0 / Math.sqrt(1.0 + (3.0 * phiJ * phiJ) / (Math.PI * Math.PI));
  const eVal = 1.0 / (1.0 + Math.exp(-gPhi * (mu - muJ)));
  const v = 1.0 / Math.max(1e-9, gPhi * gPhi * eVal * (1.0 - eVal));
  const delta = v * gPhi * (score - eVal);

  const a = Math.log(sigma * sigma);
  const f = (x: number): number => {
    const ex = Math.exp(x);
    const num = ex * (delta * delta - phi * phi - v - ex);
    const base = phi * phi + v + ex;
    const den = 2.0 * base * base;
    return num / den - (x - a) / (TAU * TAU);
  };

  let upperA = a;
  let upperB: number;
  if (delta * delta > phi * phi + v) {
    upperB = Math.log(delta * delta - phi * phi - v);
  } else {
    let k = 1.0;
    while (f(a - k * TAU) < 0.0 && k < 50.0) {
      k += 1.0;
    }
    upperB = a - k * TAU;
  }

  let fA = f(upperA);
  let fB = f(upperB);
  for (let iter = 0; iter < 40; iter++) {
    if (Math.abs(upperB - upperA) <= 1e-6) break;
    const denom = fB - fA;
    if (Math.abs(denom) < 1e-12) break;
    const c = upperA + ((upperA - upperB) * fA) / denom;
    const fC = f(c);
    if (fC * fB <= 0.0) {
      upperA = upperB;
      fA = fB;
    } else {
      fA *= 0.5;
    }
    upperB = c;
    fB = fC;
  }

  const newSigma = clamp(Math.exp(upperA * 0.5), 0.01, 0.25);
  const phiStar = Math.sqrt(phi * phi + newSigma * newSigma);
  const newPhi =
    1.0 / Math.sqrt(1.0 / (phiStar * phiStar) + 1.0 / v);
  const newMu = mu + newPhi * newPhi * gPhi * (score - eVal);

  return {
    rating: clamp(1500.0 + SCALE * newMu, 100.0, 4000.0),
    rd: clamp(SCALE * newPhi, 30.0, 350.0),
    sigma: newSigma,
  };
}

export function validateSafeReplayUrl(
  raw: string | undefined | null
): string | null {
  if (raw === undefined || raw === null) return null;
  const trimmed = String(raw).trim();
  if (!trimmed) return null;
  if (/[\u0000-\u001F\u007F]/.test(trimmed)) return null;
  if (trimmed.startsWith('//') || trimmed.startsWith('\\\\')) return null;

  const schemeMatch = trimmed.match(/^([a-zA-Z][a-zA-Z0-9+.-]*):/);
  if (schemeMatch) {
    const proto = schemeMatch[1].toLowerCase();
    if (proto !== 'https' && proto !== 'http') {
      return null;
    }
    try {
      const parsed = new URL(trimmed);
      if (parsed.protocol !== 'https:' && parsed.protocol !== 'http:') {
        return null;
      }
      return parsed.toString();
    } catch {
      return null;
    }
  }

  // Relative path: forbid directory traversal or HTML injection characters
  if (
    trimmed.includes('..') ||
    trimmed.includes('\\') ||
    /[<>"'`]/.test(trimmed)
  ) {
    return null;
  }
  return trimmed;
}

export interface LobbyFilterOptions {
  modId?: string;
  onlyOpen?: boolean;
  status?: LobbyStatus;
  search?: string;
}

export interface RegistryOptions {
  seedDemoMatches?: boolean;
  relayRoomOptions?: RelayRoomOptions;
  relayMuxPort?: number;
  muxRelayOptions?: MultiplexedRelayOptions;
  udpRendezvousPort?: number;
  udpRendezvousOptions?: UdpRendezvousOptions;
  preferMuxRelay?: boolean;
  dataDir?: string;
}

interface PersistedPlayerBinding {
  player_id: string;
  display_name: string;
  public_token: string;
}

interface PersistedPlayerMmr {
  display_name: string;
  player_id?: string | null;
  rating: number;
  rd: number;
  sigma: number;
  wins: number;
  losses: number;
  streak: number;
}

export interface DedicatedFleetStatsProvider {
  getFleetCounts(nowMs: number): { online: number; idle: number };
  isKnownDedicatedSubmitter(submitterName: string, playerId: string): boolean;
  verifyAnyServerToken?(token: string): boolean;
}

export class CentralMultiplayerRegistry {
  private nextLobbyId = 1;
  private nextMatchId = 1;
  private atomicSaveSeq = 1;
  private readonly ttlMs: number;
  private readonly startedAtMs: number;
  private readonly hasExplicitLegacyPortRange: boolean;
  private readonly relayPortRange: RelayPortRange;
  private readonly relayRoomOptions?: RelayRoomOptions;
  private readonly preferredMuxPort: number;
  private readonly muxRelayOptions?: MultiplexedRelayOptions;
  private readonly preferredUdpRendezvousPort: number;
  private readonly udpRendezvousOptions?: UdpRendezvousOptions;
  private readonly preferMuxRelayByDefault: boolean;
  private muxServer: MultiplexedRelayServer | null = null;
  private muxServerPromise: Promise<MultiplexedRelayServer> | null = null;
  private udpServer: UdpRendezvousServer | null = null;
  private udpServerPromise: Promise<UdpRendezvousServer> | null = null;
  private readonly muxTokensByOwner = new Map<string, string>();
  private totalLobbiesRegistered = 0;
  private readonly lobbies = new Map<string, LobbyEntry>();
  private readonly lobbyTokens = new Map<string, string>();
  private readonly lobbyPasswords = new Map<string, string>();
  private readonly relayRooms = new Map<string, RelayTunnelRoom>();
  private readonly matches: MatchRecord[] = [];
  private readonly playerBindingsById = new Map<string, PersistedPlayerBinding>();
  private readonly playerIdByNameLower = new Map<string, string>();
  private readonly playerMmrByNameLower = new Map<string, PersistedPlayerMmr>();
  private readonly usedMatchSignatures = new Set<string>();
  private readonly stateFilePath?: string;
  private dedicatedFleetProvider?: DedicatedFleetStatsProvider;

  constructor(
    ttlSecs = 45,
    initialMatches: MatchRecord[] = [],
    nowMs = Date.now(),
    relayPortRange?: RelayPortRange,
    options: RegistryOptions = {}
  ) {
    this.ttlMs = Math.max(5, ttlSecs) * 1000;
    this.startedAtMs = nowMs;
    this.hasExplicitLegacyPortRange = relayPortRange !== undefined;
    this.relayPortRange = relayPortRange ?? resolveDefaultRelayPortRange();
    this.relayRoomOptions = options.relayRoomOptions;
    this.muxRelayOptions = options.muxRelayOptions;
    this.udpRendezvousOptions = options.udpRendezvousOptions;
    this.preferredMuxPort =
      options.relayMuxPort ??
      (process.env.RELAY_MUX_PORT
        ? Math.max(0, Number(process.env.RELAY_MUX_PORT) || 0)
        : 0);
    this.preferredUdpRendezvousPort =
      options.udpRendezvousPort ??
      (process.env.UDP_RENDEZVOUS_PORT
        ? Math.max(0, Number(process.env.UDP_RENDEZVOUS_PORT) || 0)
        : 0);
    this.preferMuxRelayByDefault =
      options.preferMuxRelay ?? !this.hasExplicitLegacyPortRange;

    const effectiveDataDir =
      options.dataDir?.trim() ||
      (process.env.DATA_DIR && process.env.DATA_DIR.trim().length > 0
        ? process.env.DATA_DIR.trim()
        : undefined);
    if (effectiveDataDir) {
      this.stateFilePath = path.join(effectiveDataDir, 'registry-state.json');
      this.loadPersistedState();
    }

    const newlyAddedInitial: MatchRecord[] = [];
    for (const m of initialMatches) {
      if (!this.matches.some((existing) => existing.match_id === m.match_id)) {
        this.matches.push(m);
        newlyAddedInitial.push(m);
        this.nextMatchId++;
      }
    }

    const shouldSeedDemo =
      options.seedDemoMatches ?? process.env.SEED_DEMO_MATCHES === 'true';
    if (this.matches.length === 0 && shouldSeedDemo) {
      this.seedReferenceMatches(nowMs);
      newlyAddedInitial.push(...this.matches);
    }

    // Backfill Glicko-2 MMR if matches existed without persisted MMR entries
    if (this.playerMmrByNameLower.size === 0 && this.matches.length > 0) {
      const chronological = [...this.matches].sort(
        (a, b) => a.completed_at_unix_ms - b.completed_at_unix_ms
      );
      for (const m of chronological) {
        this.applyMatchToMmr(m);
      }
    } else if (newlyAddedInitial.length > 0) {
      const chronological = [...newlyAddedInitial].sort(
        (a, b) => a.completed_at_unix_ms - b.completed_at_unix_ms
      );
      for (const m of chronological) {
        this.applyMatchToMmr(m);
      }
    }
  }

  private loadPersistedState(): void {
    if (!this.stateFilePath || !fs.existsSync(this.stateFilePath)) return;
    try {
      const raw = fs.readFileSync(this.stateFilePath, 'utf-8');
      const parsed = JSON.parse(raw) as {
        totalLobbiesRegistered?: number;
        matches?: MatchRecord[];
        playerBindings?: PersistedPlayerBinding[];
        playerMmr?: PersistedPlayerMmr[];
        usedMatchSignatures?: string[];
      };
      if (typeof parsed.totalLobbiesRegistered === 'number') {
        this.totalLobbiesRegistered = parsed.totalLobbiesRegistered;
      }
      if (Array.isArray(parsed.matches)) {
        for (const m of parsed.matches) {
          this.matches.push(m);
          this.nextMatchId++;
        }
      }
      if (Array.isArray(parsed.playerBindings)) {
        for (const b of parsed.playerBindings) {
          if (b && b.player_id && b.display_name && b.public_token) {
            this.playerBindingsById.set(b.player_id, b);
            this.playerIdByNameLower.set(
              b.display_name.toLowerCase(),
              b.player_id
            );
          }
        }
      }
      if (Array.isArray(parsed.playerMmr)) {
        for (const r of parsed.playerMmr) {
          if (r && r.display_name) {
            this.playerMmrByNameLower.set(r.display_name.toLowerCase(), {
              display_name: r.display_name,
              player_id: r.player_id ?? null,
              rating: typeof r.rating === 'number' ? r.rating : 1500,
              rd: typeof r.rd === 'number' ? r.rd : 350,
              sigma: typeof r.sigma === 'number' ? r.sigma : 0.06,
              wins: typeof r.wins === 'number' ? r.wins : 0,
              losses: typeof r.losses === 'number' ? r.losses : 0,
              streak: typeof r.streak === 'number' ? r.streak : 0,
            });
          }
        }
      }
      if (Array.isArray(parsed.usedMatchSignatures)) {
        for (const sig of parsed.usedMatchSignatures) {
          if (typeof sig === 'string' && sig.length > 0) {
            this.usedMatchSignatures.add(sig.toLowerCase());
          }
        }
      }
    } catch {
      // ignore corrupted state file
    }
  }

  /**
   * Atomically persists registry ledger, player bindings, MMR, and replay-protection signatures
   * via `fs.writeFileSync(tempFile)` + `fs.renameSync(tempFile, stateFilePath)`.
   */
  private savePersistedState(): void {
    if (!this.stateFilePath) return;
    try {
      const dir = path.dirname(this.stateFilePath);
      fs.mkdirSync(dir, { recursive: true });
      const payload = JSON.stringify(
        {
          totalLobbiesRegistered: this.totalLobbiesRegistered,
          matches: this.matches.slice(0, 500),
          playerBindings: [...this.playerBindingsById.values()],
          playerMmr: [...this.playerMmrByNameLower.values()],
          usedMatchSignatures: [...this.usedMatchSignatures].slice(-2048),
        },
        null,
        2
      );
      const tmpPath = `${this.stateFilePath}.tmp.${process.pid}.${this.atomicSaveSeq++}`;
      fs.writeFileSync(tmpPath, payload, 'utf-8');
      fs.renameSync(tmpPath, this.stateFilePath);
    } catch {
      // ignore write errors
    }
  }

  private seedReferenceMatches(nowMs: number): void {
    const seeds: MatchRecord[] = [
      {
        match_id: `match-${this.nextMatchId++}`,
        lobby_id: null,
        lobby_name: 'Operation Chrono Storm [1v1 Ladder]',
        mod_id: 'rust-ra2',
        mod_name: 'Red Alert 2: Rust Total Conversion',
        mod_version: '0.1.0',
        map_id: 'skirmish_valley',
        map_name: 'Skirmish Valley',
        game_mode: 'Standard',
        players: [
          { name: 'CommanderVance', faction: 'Allied', team: 1, is_winner: true },
          { name: 'GenRomanov', faction: 'Soviet', team: 2, is_winner: false },
        ],
        winner_player: 'CommanderVance',
        winner_faction: 'Allied',
        duration_secs: 840,
        final_tick: 16800,
        final_state_hash: '0xcbf29ce484222325',
        total_commands: 614,
        desync_detected: false,
        replay_file: 'saves/chrono_storm_1v1.replay.json',
        completed_at_unix_ms: nowMs - 18 * 60 * 1000,
      },
      {
        match_id: `match-${this.nextMatchId++}`,
        lobby_id: null,
        lobby_name: 'Iron Curtain Siege — River Crossing',
        mod_id: 'rust-ra2',
        mod_name: 'Red Alert 2: Rust Total Conversion',
        mod_version: '0.1.0',
        map_id: 'river_crossing',
        map_name: 'River Crossing',
        game_mode: 'Standard',
        players: [
          { name: 'GenRomanov', faction: 'Soviet', team: 1, is_winner: true },
          { name: 'CommanderVance', faction: 'Allied', team: 2, is_winner: false },
        ],
        winner_player: 'GenRomanov',
        winner_faction: 'Soviet',
        duration_secs: 1120,
        final_tick: 22400,
        final_state_hash: '0xe419a07b11c88320',
        total_commands: 902,
        desync_detected: false,
        replay_file: 'saves/iron_curtain_river_crossing.replay.json',
        completed_at_unix_ms: nowMs - 75 * 60 * 1000,
      },
      {
        match_id: `match-${this.nextMatchId++}`,
        lobby_id: null,
        lobby_name: '2v2 Allied Airforce & Prism Tank Push',
        mod_id: 'rust-ra2',
        mod_name: 'Red Alert 2: Rust Total Conversion',
        mod_version: '0.1.0',
        map_id: 'mp_02_isle_of_war',
        map_name: 'Skirmish: Isle of War (2-4)',
        game_mode: 'Team 2v2',
        players: [
          { name: 'SkyMarshal', faction: 'Allied', team: 1, is_winner: true },
          { name: 'CommanderVance', faction: 'Allied', team: 1, is_winner: true },
          { name: 'GenRomanov', faction: 'Soviet', team: 2, is_winner: false },
          { name: 'YuriPrime', faction: 'Yuri', team: 2, is_winner: false },
        ],
        winner_player: 'SkyMarshal',
        winner_faction: 'Allied',
        duration_secs: 1540,
        final_tick: 30800,
        final_state_hash: '0x77b4d90231fa6c19',
        total_commands: 1490,
        desync_detected: false,
        replay_file: 'saves/isle_of_war_2v2.replay.json',
        completed_at_unix_ms: nowMs - 135 * 60 * 1000,
      },
      {
        match_id: `match-${this.nextMatchId++}`,
        lobby_id: null,
        lobby_name: 'Psi-Corps Mind Control Showdown [Yuri vs Soviet]',
        mod_id: 'rust-ra2',
        mod_name: 'Red Alert 2: Rust Total Conversion',
        mod_version: '0.1.0',
        map_id: 'mp_yr_austin_tx',
        map_name: 'Skirmish: Austin, TX (2-4)',
        game_mode: 'Standard',
        players: [
          { name: 'PsychicMaster', faction: 'Yuri', team: 1, is_winner: true },
          { name: 'GenRomanov', faction: 'Soviet', team: 2, is_winner: false },
        ],
        winner_player: 'PsychicMaster',
        winner_faction: 'Yuri',
        duration_secs: 980,
        final_tick: 19600,
        final_state_hash: '0x9a21f408cc71e304',
        total_commands: 745,
        desync_detected: false,
        replay_file: 'saves/austin_tx_yuri_victory.replay.json',
        completed_at_unix_ms: nowMs - 210 * 60 * 1000,
      },
      {
        match_id: `match-${this.nextMatchId++}`,
        lobby_id: null,
        lobby_name: 'South Pacific Naval & Air Superiority [1v1]',
        mod_id: 'rust-ra2',
        mod_name: 'Red Alert 2: Rust Total Conversion',
        mod_version: '0.1.0',
        map_id: 'mp_01_offense_defense',
        map_name: 'Skirmish: South Pacific (2-4)',
        game_mode: 'Standard',
        players: [
          { name: 'CommanderVance', faction: 'Allied', team: 1, is_winner: true },
          { name: 'PsychicMaster', faction: 'Yuri', team: 2, is_winner: false },
        ],
        winner_player: 'CommanderVance',
        winner_faction: 'Allied',
        duration_secs: 1310,
        final_tick: 26200,
        final_state_hash: '0x5d8b02e74f19a6c2',
        total_commands: 1084,
        desync_detected: false,
        replay_file: 'saves/south_pacific_1v1.replay.json',
        completed_at_unix_ms: nowMs - 320 * 60 * 1000,
      },
    ];

    this.matches.push(...seeds);
  }

  public async ensureMuxRelayServer(): Promise<MultiplexedRelayServer> {
    if (this.muxServer) {
      return this.muxServer;
    }
    if (!this.muxServerPromise) {
      this.muxServerPromise = MultiplexedRelayServer.create(
        this.preferredMuxPort,
        '0.0.0.0',
        {
          ...this.relayRoomOptions,
          ...this.muxRelayOptions,
        }
      ).then((srv) => {
        this.muxServer = srv;
        return srv;
      });
    }
    return this.muxServerPromise;
  }

  public get relayMuxPort(): number | null {
    return this.muxServer ? this.muxServer.port : null;
  }

  public async ensureUdpRendezvousServer(): Promise<UdpRendezvousServer> {
    if (this.udpServer) {
      return this.udpServer;
    }
    if (!this.udpServerPromise) {
      this.udpServerPromise = UdpRendezvousServer.create(
        this.preferredUdpRendezvousPort,
        '0.0.0.0',
        this.udpRendezvousOptions
      ).then((srv) => {
        this.udpServer = srv;
        return srv;
      });
    }
    return this.udpServerPromise;
  }

  public get udpRendezvousPort(): number | null {
    return this.udpServer ? this.udpServer.port : null;
  }

  public async allocateMuxRelayRoom(
    ownerKey: string,
    serverLocalIp?: string
  ): Promise<{
    relayAddr: string;
    relayUplinkAddr: string;
    roomToken: string;
    muxPort: number;
    udpPort: number;
    udpAddr: string;
  }> {
    const [mux, udp] = await Promise.all([
      this.ensureMuxRelayServer(),
      this.ensureUdpRendezvousServer(),
    ]);
    let roomToken = this.muxTokensByOwner.get(ownerKey);
    if (!roomToken) {
      roomToken = `rm-${crypto.randomBytes(12).toString('hex')}`;
      this.muxTokensByOwner.set(ownerKey, roomToken);
    }
    mux.registerRoom(roomToken);
    const srvIp = normalizePeerIp(serverLocalIp) || '127.0.0.1';
    const endpoint = formatMuxEndpoint(`${srvIp}:${mux.port}`, roomToken);
    return {
      relayAddr: endpoint,
      relayUplinkAddr: endpoint,
      roomToken,
      muxPort: mux.port,
      udpPort: udp.port,
      udpAddr: `${srvIp}:${udp.port}`,
    };
  }

  public releaseMuxRelayRoom(ownerKey: string): void {
    const tok = this.muxTokensByOwner.get(ownerKey);
    if (tok) {
      this.muxTokensByOwner.delete(ownerKey);
      this.muxServer?.removeRoom(tok);
    }
  }

  public allocateNextLobbyId(): string {
    this.totalLobbiesRegistered++;
    return `lobby-${this.nextLobbyId++}`;
  }

  public pruneExpired(nowMs = Date.now()): void {
    for (const [id, entry] of this.lobbies.entries()) {
      if (Math.max(0, nowMs - entry.last_heartbeat_unix_ms) > this.ttlMs) {
        this.lobbies.delete(id);
        this.lobbyTokens.delete(id);
        this.lobbyPasswords.delete(id);
        const room = this.relayRooms.get(id);
        if (room) {
          room.close();
          this.relayRooms.delete(id);
        }
        this.releaseMuxRelayRoom(id);
      }
    }
  }

  public closeAllRelays(): void {
    for (const room of this.relayRooms.values()) {
      room.close();
    }
    this.relayRooms.clear();
    this.muxTokensByOwner.clear();
    if (this.muxServer) {
      const srv = this.muxServer;
      this.muxServer = null;
      this.muxServerPromise = null;
      void srv.close();
    } else if (this.muxServerPromise) {
      const pending = this.muxServerPromise;
      this.muxServerPromise = null;
      void pending.then((srv) => srv.close()).catch(() => {});
    }
    if (this.udpServer) {
      const udp = this.udpServer;
      this.udpServer = null;
      this.udpServerPromise = null;
      void udp.close();
    } else if (this.udpServerPromise) {
      const pendingUdp = this.udpServerPromise;
      this.udpServerPromise = null;
      void pendingUdp.then((udp) => udp.close()).catch(() => {});
    }
  }

  public async closeAllRelayRooms(): Promise<void> {
    const rooms = [...this.relayRooms.values()];
    this.relayRooms.clear();
    this.muxTokensByOwner.clear();
    rooms.forEach((room) => room.close());
    if (this.muxServer) {
      const srv = this.muxServer;
      this.muxServer = null;
      this.muxServerPromise = null;
      await srv.close();
    } else if (this.muxServerPromise) {
      const pending = this.muxServerPromise;
      this.muxServerPromise = null;
      const srv = await pending;
      await srv.close();
    }
    if (this.udpServer) {
      const udp = this.udpServer;
      this.udpServer = null;
      this.udpServerPromise = null;
      await udp.close();
    } else if (this.udpServerPromise) {
      const pendingUdp = this.udpServerPromise;
      this.udpServerPromise = null;
      const udp = await pendingUdp;
      await udp.close();
    }
  }

  public get maxRelayRooms(): number | null {
    if (this.preferMuxRelayByDefault) {
      return null;
    }
    return this.relayPortRange
      ? maxRelayRoomsForRange(this.relayPortRange)
      : null;
  }

  public get activeRelayRooms(): number {
    return this.relayRooms.size + this.muxTokensByOwner.size;
  }

  public async registerOrUpdate(
    ann: LobbyAnnouncement,
    peerIp?: string,
    serverLocalIp?: string,
    nowMs = Date.now(),
    providedHostToken?: string
  ): Promise<RegisteredLobbyResponse> {
    this.pruneExpired(nowMs);

    let hostAddr = String(ann.host_addr || '127.0.0.1:7878').trim();
    const hostPlayer = String(ann.host_player || 'HostPlayer').trim();
    const cleanIp = normalizePeerIp(peerIp);
    if (cleanIp && hostAddr.startsWith('0.0.0.0:')) {
      const port = hostAddr.slice('0.0.0.0:'.length);
      hostAddr = `${cleanIp}:${port}`;
    } else if (
      cleanIp &&
      cleanIp !== '127.0.0.1' &&
      hostAddr.startsWith('127.0.0.1:')
    ) {
      const port = hostAddr.slice('127.0.0.1:'.length);
      hostAddr = `${cleanIp}:${port}`;
    }

    let existingId: string | undefined;
    for (const entry of this.lobbies.values()) {
      if (entry.host_addr === hostAddr && entry.host_player === hostPlayer) {
        existingId = entry.lobby_id;
        break;
      }
    }

    const tokenCandidate = (
      providedHostToken ||
      ann.hostToken ||
      ann.host_token ||
      ''
    ).trim();

    if (existingId) {
      const expectedToken = this.lobbyTokens.get(existingId);
      if (expectedToken) {
        if (!tokenCandidate) {
          throw new LobbyAuthError(
            401,
            'Host authentication token required to update an active lobby'
          );
        }
        if (!verifySecretToken(tokenCandidate, expectedToken)) {
          throw new LobbyAuthError(403, 'Invalid host authentication token');
        }
      }
    }

    const isNew = !existingId;
    const candidateLobbyId = existingId ?? `lobby-${this.nextLobbyId}`;
    const existingEntry = existingId
      ? this.lobbies.get(existingId)
      : undefined;
    const createdAt = existingEntry ? existingEntry.created_at_unix_ms : nowMs;

    const maxPlayers = clamp(
      Math.floor(Number(ann.max_players) || 2),
      2,
      16
    );
    const currentPlayers = clamp(
      Math.floor(Number(ann.current_players) || 1),
      1,
      maxPlayers
    );
    const status: LobbyStatus =
      currentPlayers >= maxPlayers ? 'full' : 'open';

    const hostPortMatch = hostAddr.split(':').pop();
    const hostPort = Number(hostPortMatch) || 7878;
    let externalAddr: string | null = null;
    if (ann.upnp_external_addr && ann.upnp_external_addr.trim().length > 0) {
      externalAddr = ann.upnp_external_addr.trim();
    } else if (cleanIp && cleanIp !== '127.0.0.1') {
      const candidate = `${cleanIp}:${hostPort}`;
      if (candidate !== hostAddr) {
        externalAddr = candidate;
      }
    }

    const srvIp = normalizePeerIp(serverLocalIp) || '127.0.0.1';
    let relayAddr: string | null = null;
    let relayUplinkAddr: string | null = null;
    let relayRoomToken: string | null = null;
    let relayMuxPort: number | null = null;
    let udpRendezvousPort: number | null = null;
    let udpRendezvousAddr: string | null = null;

    if (ann.allocate_relay) {
      const useMux =
        Boolean(ann.prefer_mux_relay) || this.preferMuxRelayByDefault;
      if (useMux) {
        const muxAlloc = await this.allocateMuxRelayRoom(
          candidateLobbyId,
          srvIp
        );
        relayAddr = muxAlloc.relayAddr;
        relayUplinkAddr = muxAlloc.relayUplinkAddr;
        relayRoomToken = muxAlloc.roomToken;
        relayMuxPort = muxAlloc.muxPort;
        udpRendezvousPort = muxAlloc.udpPort;
        udpRendezvousAddr = muxAlloc.udpAddr;
      } else {
        let room = existingId ? this.relayRooms.get(existingId) : undefined;
        if (!room) {
          if (this.relayPortRange) {
            const maxRooms = maxRelayRoomsForRange(this.relayPortRange);
            if (maxRooms < 1 || this.relayRooms.size >= maxRooms) {
              throw new RelayCapacityError(
                `Relay port pool exhausted: maximum ${maxRooms} concurrent relay rooms (${this.relayPortRange.start}-${this.relayPortRange.end}) are active`
              );
            }
          }
          room = await RelayTunnelRoom.create(
            this.relayPortRange,
            this.relayRoomOptions
          );
          this.relayRooms.set(candidateLobbyId, room);
        }
        relayAddr = `${srvIp}:${room.clientPort}`;
        relayUplinkAddr = `${srvIp}:${room.uplinkPort}`;
      }
    }

    if (isNew) {
      this.nextLobbyId++;
      this.totalLobbiesRegistered++;
    }

    const lobbyId = candidateLobbyId;
    let hostToken = this.lobbyTokens.get(lobbyId);
    if (!hostToken) {
      hostToken = generateHostToken();
      this.lobbyTokens.set(lobbyId, hostToken);
    }

    const rawPassword =
      typeof ann.password === 'string' ? ann.password.trim() : '';
    if (rawPassword.length > 0) {
      this.lobbyPasswords.set(lobbyId, hashLobbyPassword(rawPassword));
    } else if (ann.password === null || ann.password_protected === false) {
      this.lobbyPasswords.delete(lobbyId);
    }
    const passwordProtected =
      this.lobbyPasswords.has(lobbyId) || Boolean(ann.password_protected);

    const hasUpnp = Boolean(
      ann.upnp_external_addr && ann.upnp_external_addr.trim()
    );
    const hasRelay = Boolean(relayAddr);
    const natMode =
      hasUpnp && hasRelay
        ? 'UPnP + Relay-Fallback'
        : hasUpnp
          ? 'UPnP Direct P2P'
          : hasRelay
            ? 'Relay-Tunnel (NAT-Safe)'
            : 'LAN / Direct IP';

    const entry: LobbyEntry = {
      lobby_id: lobbyId,
      lobby_name: String(ann.lobby_name || 'Unbenannte RTS-Lobby').trim(),
      host_player: hostPlayer,
      host_addr: hostAddr,
      external_addr: externalAddr,
      relay_addr: relayAddr,
      relay_uplink_addr: relayUplinkAddr,
      ...(relayRoomToken !== null ? { relay_room_token: relayRoomToken } : {}),
      ...(relayMuxPort !== null ? { relay_mux_port: relayMuxPort } : {}),
      ...(udpRendezvousPort !== null
        ? { udp_rendezvous_port: udpRendezvousPort }
        : {}),
      ...(udpRendezvousAddr !== null
        ? { udp_rendezvous_addr: udpRendezvousAddr }
        : {}),
      nat_mode: natMode,
      mod_id: String(ann.mod_id || 'rust-ra2').trim(),
      mod_name: String(ann.mod_name || ann.mod_id || 'rust-ra2').trim(),
      mod_version: String(ann.mod_version || '0.1.0').trim(),
      map_id: String(ann.map_id || 'skirmish_valley').trim(),
      map_name: String(ann.map_name || ann.map_id || 'Skirmish Valley').trim(),
      current_players: currentPlayers,
      max_players: maxPlayers,
      status,
      game_mode: String(ann.game_mode || 'Standard').trim(),
      password_protected: passwordProtected,
      created_at_unix_ms: createdAt,
      last_heartbeat_unix_ms: nowMs,
    };

    this.lobbies.set(lobbyId, entry);
    return {
      ...entry,
      hostToken,
      host_token: hostToken,
    };
  }

  public heartbeat(
    lobbyId: string,
    hb: LobbyHeartbeatRequest,
    nowMs = Date.now(),
    providedHostToken?: string
  ): LobbyEntry | null {
    this.pruneExpired(nowMs);
    const entry = this.lobbies.get(lobbyId);
    if (!entry) return null;

    const expectedToken = this.lobbyTokens.get(lobbyId);
    if (expectedToken) {
      const candidate = (
        providedHostToken ||
        hb.hostToken ||
        hb.host_token ||
        ''
      ).trim();
      if (!candidate) {
        throw new LobbyAuthError(
          401,
          'Missing host authentication token (x-host-token or Authorization: Bearer)'
        );
      }
      if (!verifySecretToken(candidate, expectedToken)) {
        throw new LobbyAuthError(403, 'Invalid host authentication token');
      }
    }

    entry.max_players = clamp(
      Math.floor(Number(hb.max_players) || entry.max_players),
      2,
      16
    );
    entry.current_players = clamp(
      Math.floor(Number(hb.current_players) || entry.current_players),
      1,
      entry.max_players
    );
    if (hb.map_id) {
      entry.map_id = String(hb.map_id).trim();
    }
    if (hb.map_name) {
      entry.map_name = String(hb.map_name).trim();
    }

    if (
      hb.status === 'open' ||
      hb.status === 'full' ||
      hb.status === 'in_game'
    ) {
      entry.status = hb.status;
    } else {
      entry.status =
        entry.current_players >= entry.max_players ? 'full' : 'open';
    }

    entry.last_heartbeat_unix_ms = nowMs;
    return { ...entry };
  }

  public deleteLobby(
    lobbyId: string,
    nowMs = Date.now(),
    providedHostToken?: string
  ): boolean {
    this.pruneExpired(nowMs);
    if (!this.lobbies.has(lobbyId)) {
      return false;
    }

    const expectedToken = this.lobbyTokens.get(lobbyId);
    if (expectedToken) {
      const candidate = (providedHostToken || '').trim();
      if (!candidate) {
        throw new LobbyAuthError(
          401,
          'Missing host authentication token (x-host-token or Authorization: Bearer)'
        );
      }
      if (!verifySecretToken(candidate, expectedToken)) {
        throw new LobbyAuthError(403, 'Invalid host authentication token');
      }
    }

    const room = this.relayRooms.get(lobbyId);
    if (room) {
      room.close();
      this.relayRooms.delete(lobbyId);
    }
    this.releaseMuxRelayRoom(lobbyId);
    this.lobbyTokens.delete(lobbyId);
    this.lobbyPasswords.delete(lobbyId);
    return this.lobbies.delete(lobbyId);
  }

  public verifyLobbyPassword(
    lobbyId: string,
    candidatePassword?: string | null,
    nowMs = Date.now()
  ): { found: boolean; valid: boolean; lobby?: LobbyEntry } {
    this.pruneExpired(nowMs);
    const entry = this.lobbies.get(lobbyId);
    if (!entry) {
      return { found: false, valid: false };
    }
    const storedHash = this.lobbyPasswords.get(lobbyId);
    if (!storedHash) {
      if (!entry.password_protected) {
        return {
          found: true,
          valid: true,
          lobby: { ...entry, relay_uplink_addr: null },
        };
      }
      const nonEmpty = Boolean(candidatePassword && candidatePassword.trim());
      return {
        found: true,
        valid: nonEmpty,
        lobby: nonEmpty ? { ...entry, relay_uplink_addr: null } : undefined,
      };
    }

    const candidate = (candidatePassword ?? '').trim();
    if (!candidate || !verifyLobbyPasswordHash(candidate, storedHash)) {
      return { found: true, valid: false };
    }
    return {
      found: true,
      valid: true,
      lobby: { ...entry, relay_uplink_addr: null },
    };
  }

  public listLobbies(
    filters: LobbyFilterOptions = {},
    nowMs = Date.now()
  ): LobbyEntry[] {
    this.pruneExpired(nowMs);
    const searchLower = filters.search?.trim().toLowerCase() ?? '';

    const result: LobbyEntry[] = [];
    for (const entry of this.lobbies.values()) {
      if (
        filters.modId &&
        filters.modId.length > 0 &&
        entry.mod_id !== filters.modId
      ) {
        continue;
      }
      if (filters.onlyOpen && entry.status !== 'open') {
        continue;
      }
      if (filters.status && entry.status !== filters.status) {
        continue;
      }
      if (searchLower.length > 0) {
        const haystack =
          `${entry.lobby_name} ${entry.host_player} ${entry.map_name} ${entry.map_id} ${entry.mod_id} ${entry.host_addr}`.toLowerCase();
        if (!haystack.includes(searchLower)) {
          continue;
        }
      }
      // Public listing intentionally omits host tokens and internal host-only relay_uplink_addr
      result.push({
        ...entry,
        relay_uplink_addr: null,
      });
    }

    return result;
  }

  public registerPlayerBinding(
    playerIdRaw: string,
    displayNameRaw: string,
    publicTokenRaw: string
  ): PersistedPlayerBinding {
    const playerId = String(playerIdRaw ?? '').trim();
    const displayName = String(displayNameRaw ?? '').trim();
    const publicToken = String(publicTokenRaw ?? '').trim();

    if (!playerId || !displayName || publicToken.length !== 64) {
      throw new RegistryValidationError(
        'Invalid player_id, display_name, or public_token'
      );
    }

    const expectedPid = derivePlayerIdFromToken(publicToken);
    if (!verifySecretToken(playerId, expectedPid)) {
      throw new LobbyAuthError(
        401,
        'player_id does not match cryptographic public_token derivation'
      );
    }

    const nameLower = displayName.toLowerCase();
    const existingOwner = this.playerIdByNameLower.get(nameLower);
    if (existingOwner && existingOwner !== playerId) {
      throw new LobbyAuthError(
        403,
        `Commander name '${displayName}' is already cryptographically bound to another player profile`
      );
    }

    const existingBinding = this.playerBindingsById.get(playerId);
    if (existingBinding) {
      if (!verifySecretToken(existingBinding.public_token, publicToken)) {
        throw new LobbyAuthError(
          401,
          'Public token mismatch for existing player_id'
        );
      }
      const prevLower = existingBinding.display_name.toLowerCase();
      if (prevLower !== nameLower) {
        this.playerIdByNameLower.delete(prevLower);
      }
    }

    const binding: PersistedPlayerBinding = {
      player_id: playerId,
      display_name: displayName,
      public_token: publicToken,
    };
    this.playerBindingsById.set(playerId, binding);
    this.playerIdByNameLower.set(nameLower, playerId);

    const existingMmr = this.playerMmrByNameLower.get(nameLower);
    if (existingMmr) {
      existingMmr.display_name = displayName;
      existingMmr.player_id = playerId;
    } else {
      this.playerMmrByNameLower.set(nameLower, {
        display_name: displayName,
        player_id: playerId,
        rating: 1500,
        rd: 350,
        sigma: 0.06,
        wins: 0,
        losses: 0,
        streak: 0,
      });
    }

    this.savePersistedState();
    return { ...binding };
  }

  public setDedicatedFleetProvider(provider: DedicatedFleetStatsProvider): void {
    this.dedicatedFleetProvider = provider;
  }

  public getPlayerRating(displayName: string): number {
    const rec = this.playerMmrByNameLower.get(displayName.trim().toLowerCase());
    return rec ? rec.rating : 1500;
  }

  private applyMatchToMmr(record: MatchRecord): void {
    const rawPlayers = Array.isArray(record.players) ? record.players : [];
    const seen = new Set<string>();
    interface ResolvedMatchParticipant {
      raw: MatchPlayerRecord;
      effectiveName: string;
      lowerKey: string;
      resolvedPlayerId: string | null;
    }
    const uniquePlayers: ResolvedMatchParticipant[] = [];
    for (const p of rawPlayers) {
      const explicitPid =
        typeof p.player_id === 'string' && p.player_id.trim().length > 0
          ? p.player_id.trim()
          : null;
      const boundById = explicitPid
        ? this.playerBindingsById.get(explicitPid)
        : undefined;
      const effectiveName = boundById
        ? boundById.display_name
        : String(p.name ?? '').trim();
      if (!effectiveName) continue;
      const lowerKey = effectiveName.toLowerCase();
      const dedupeKey = boundById ? `id:${boundById.player_id}` : `name:${lowerKey}`;
      if (!seen.has(dedupeKey)) {
        seen.add(dedupeKey);
        const resolvedPlayerId = boundById
          ? boundById.player_id
          : explicitPid ?? this.playerIdByNameLower.get(lowerKey) ?? null;
        uniquePlayers.push({
          raw: p,
          effectiveName,
          lowerKey,
          resolvedPlayerId,
        });
      }
    }
    if (uniquePlayers.length === 0) return;

    const snapshot = (
      lowerKey: string
    ): { rating: number; rd: number; sigma: number } => {
      const rec = this.playerMmrByNameLower.get(lowerKey);
      return rec
        ? { rating: rec.rating, rd: rec.rd, sigma: rec.sigma }
        : { rating: 1500, rd: 350, sigma: 0.06 };
    };

    const winners = uniquePlayers.filter((p) => p.raw.is_winner);
    const losers = uniquePlayers.filter((p) => !p.raw.is_winner);
    const hasCompetitiveOutcome =
      !record.desync_detected && winners.length > 0 && losers.length > 0;

    let winAvgR = 1500;
    let winAvgRd = 350;
    let loseAvgR = 1500;
    let loseAvgRd = 350;
    if (hasCompetitiveOutcome) {
      winAvgR =
        winners.reduce((acc, p) => acc + snapshot(p.lowerKey).rating, 0) /
        winners.length;
      winAvgRd =
        winners.reduce((acc, p) => acc + snapshot(p.lowerKey).rd, 0) /
        winners.length;
      loseAvgR =
        losers.reduce((acc, p) => acc + snapshot(p.lowerKey).rating, 0) /
        losers.length;
      loseAvgRd =
        losers.reduce((acc, p) => acc + snapshot(p.lowerKey).rd, 0) /
        losers.length;
    }

    for (const p of uniquePlayers) {
      const trimmed = p.effectiveName;
      if (trimmed === 'Opponent') continue;
      const lower = p.lowerKey;
      const pre = snapshot(lower);
      const boundPid = p.resolvedPlayerId;

      let entry = this.playerMmrByNameLower.get(lower);
      if (!entry) {
        entry = {
          display_name: trimmed,
          player_id: boundPid,
          rating: 1500,
          rd: 350,
          sigma: 0.06,
          wins: 0,
          losses: 0,
          streak: 0,
        };
        this.playerMmrByNameLower.set(lower, entry);
      } else if (boundPid) {
        entry.player_id = boundPid;
        entry.display_name = trimmed;
      }

      if (p.raw.is_winner) {
        entry.wins += 1;
        entry.streak = entry.streak > 0 ? entry.streak + 1 : 1;
        if (hasCompetitiveOutcome) {
          const upd = glicko2UpdateSingle(
            pre.rating,
            pre.rd,
            pre.sigma,
            loseAvgR,
            loseAvgRd,
            1.0
          );
          entry.rating = Math.round(upd.rating * 100) / 100;
          entry.rd = Math.round(upd.rd * 100) / 100;
          entry.sigma = upd.sigma;
        }
      } else {
        entry.losses += 1;
        entry.streak = entry.streak < 0 ? entry.streak - 1 : -1;
        if (hasCompetitiveOutcome) {
          const upd = glicko2UpdateSingle(
            pre.rating,
            pre.rd,
            pre.sigma,
            winAvgR,
            winAvgRd,
            0.0
          );
          entry.rating = Math.round(upd.rating * 100) / 100;
          entry.rd = Math.round(upd.rd * 100) / 100;
          entry.sigma = upd.sigma;
        }
      }
    }
  }

  public recordSignedMatch(
    report: SignedMatchReport,
    nowMs = Date.now(),
    serverTokenHeader?: string
  ): MatchRecord {
    const submitterName =
      typeof report.submitter_name === 'string'
        ? report.submitter_name.trim()
        : '';
    const submitterPid =
      typeof report.player_id === 'string' ? report.player_id.trim() : '';
    const hasValidFleetToken = Boolean(
      serverTokenHeader &&
        this.dedicatedFleetProvider?.verifyAnyServerToken?.(serverTokenHeader)
    );
    const isFleetSubmitter =
      hasValidFleetToken ||
      Boolean(
        this.dedicatedFleetProvider?.isKnownDedicatedSubmitter(
          submitterName,
          submitterPid
        )
      );

    const check = verifySignedMatchReport(report, {
      allowNeutralSubmitter: isFleetSubmitter,
    });
    if (!check.valid) {
      throw new LobbyAuthError(
        check.statusCode ?? 401,
        check.error ?? 'Signed match report verification failed'
      );
    }

    const sigLower = report.signature.trim().toLowerCase();
    if (this.usedMatchSignatures.has(sigLower)) {
      throw new LobbyAuthError(
        409,
        'Replay of previously submitted signed match report rejected'
      );
    }

    const submitterLower = submitterName.toLowerCase();
    const isParticipantSubmitter =
      !Array.isArray(check.matchRequest.players) ||
      check.matchRequest.players.length === 0 ||
      check.matchRequest.players.some(
        (p) => String(p.name ?? '').trim().toLowerCase() === submitterLower
      );

    // Enforce & persist cryptographic binding when submitted by a participating commander
    if (isParticipantSubmitter) {
      this.registerPlayerBinding(
        report.player_id,
        report.submitter_name,
        report.public_token
      );
    }

    this.usedMatchSignatures.add(sigLower);
    if (this.usedMatchSignatures.size > 4096) {
      const oldest = this.usedMatchSignatures.values().next().value;
      if (oldest) this.usedMatchSignatures.delete(oldest);
    }

    return this.recordMatchInternal(check.matchRequest, nowMs, true);
  }

  public recordMatch(req: CreateMatchRequest, nowMs = Date.now()): MatchRecord {
    return this.recordMatchInternal(req, nowMs, false);
  }

  private recordMatchInternal(
    req: CreateMatchRequest,
    nowMs: number,
    isAuthenticatedSignedSubmission: boolean
  ): MatchRecord {
    if (!isAuthenticatedSignedSubmission) {
      const candidateNames: string[] = [];
      if (Array.isArray(req.players)) {
        for (const p of req.players) {
          if (p && typeof p.name === 'string' && p.name.trim()) {
            candidateNames.push(p.name.trim());
          }
          const rawPid =
            p && typeof p.player_id === 'string' ? p.player_id.trim() : '';
          if (rawPid && this.playerBindingsById.has(rawPid)) {
            throw new LobbyAuthError(
              403,
              `Commander profile '${rawPid}' is cryptographically registered; submit a signed match report via POST /api/v1/matches`
            );
          }
        }
      }
      if (typeof req.winner_player === 'string' && req.winner_player.trim()) {
        candidateNames.push(req.winner_player.trim());
      }
      for (const name of candidateNames) {
        if (this.playerIdByNameLower.has(name.toLowerCase())) {
          throw new LobbyAuthError(
            403,
            `Commander '${name}' is cryptographically registered; submit a signed match report via POST /api/v1/matches`
          );
        }
      }
    }

    const rawReplayUrl = req.replay_url ?? req.replayUrl;
    let safeReplayUrl: string | undefined;
    if (
      rawReplayUrl !== undefined &&
      rawReplayUrl !== null &&
      String(rawReplayUrl).trim().length > 0
    ) {
      const validated = validateSafeReplayUrl(String(rawReplayUrl));
      if (!validated) {
        throw new RegistryValidationError(
          'Unsafe or invalid replayUrl: only https:, http:, or clean relative paths are allowed'
        );
      }
      safeReplayUrl = validated;
    }

    let safeReplayFile: string | undefined;
    if (
      req.replay_file !== undefined &&
      String(req.replay_file).trim().length > 0
    ) {
      const validatedFile = validateSafeReplayUrl(String(req.replay_file));
      if (!validatedFile) {
        throw new RegistryValidationError(
          'Unsafe or invalid replay_file path'
        );
      }
      safeReplayFile = validatedFile;
    }

    const finalTick =
      req.final_tick !== undefined
        ? Math.max(0, Math.floor(Number(req.final_tick) || 0))
        : 1200;
    const durationSecs =
      req.duration_secs !== undefined
        ? Math.max(0, Math.floor(Number(req.duration_secs) || 0))
        : Math.max(1, Math.round(finalTick / 20));

    let formattedHash = String(
      req.final_state_hash ?? '0xcbf29ce484222325'
    ).trim();
    if (typeof req.final_state_hash === 'number') {
      formattedHash =
        '0x' + req.final_state_hash.toString(16).padStart(16, '0');
    } else if (!formattedHash) {
      formattedHash = '0x0000000000000000';
    } else if (!formattedHash.startsWith('0x') && !formattedHash.startsWith('0X')) {
      formattedHash = '0x' + formattedHash;
    }

    const winnerPlayer = String(req.winner_player || 'Player 1').trim();
    const winnerFaction = String(req.winner_faction || 'Allied').trim();

    const players: MatchPlayerRecord[] =
      Array.isArray(req.players) && req.players.length > 0
        ? req.players.map((p, idx) => {
            const rawPid =
              typeof p.player_id === 'string' && p.player_id.trim().length > 0
                ? p.player_id.trim()
                : p.player_id === null
                  ? null
                  : undefined;
            return {
              name: String(p.name || `Player ${idx + 1}`).trim(),
              faction: String(p.faction || 'Allied').trim(),
              team:
                typeof p.team === 'number' && Number.isFinite(p.team)
                  ? clamp(Math.floor(p.team), 0, 255)
                  : idx + 1,
              is_winner: Boolean(p.is_winner),
              ...(rawPid !== undefined ? { player_id: rawPid } : {}),
            };
          })
        : [
            {
              name: winnerPlayer,
              faction: winnerFaction,
              team: 1,
              is_winner: true,
            },
            {
              name: 'Opponent',
              faction: winnerFaction === 'Allied' ? 'Soviet' : 'Allied',
              team: 2,
              is_winner: false,
            },
          ];

    const lobbyName = String(req.lobby_name ?? '').trim() || 'Skirmish Match';
    const lobbyId =
      req.lobby_id !== undefined && req.lobby_id !== null
        ? String(req.lobby_id).trim() || null
        : null;

    const record: MatchRecord = {
      match_id: `match-${this.nextMatchId++}`,
      lobby_id: lobbyId,
      lobby_name: lobbyName,
      mod_id: String(req.mod_id || 'rust-ra2').trim(),
      mod_name: String(
        req.mod_name || 'Red Alert 2: Rust Total Conversion'
      ).trim(),
      mod_version: String(req.mod_version || '0.1.0').trim(),
      map_id: String(req.map_id || 'skirmish_valley').trim(),
      map_name: String(req.map_name || req.map_id || 'Skirmish Valley').trim(),
      game_mode: String(req.game_mode || 'Standard').trim(),
      players,
      winner_player: winnerPlayer,
      winner_faction: winnerFaction,
      duration_secs: durationSecs,
      final_tick: finalTick,
      final_state_hash: formattedHash,
      total_commands:
        req.total_commands !== undefined
          ? Math.max(0, Math.floor(Number(req.total_commands) || 0))
          : 240,
      desync_detected: Boolean(req.desync_detected),
      ...(safeReplayFile ? { replay_file: safeReplayFile } : {}),
      ...(safeReplayUrl
        ? { replay_url: safeReplayUrl, replayUrl: safeReplayUrl }
        : {}),
      completed_at_unix_ms: nowMs,
    };

    this.applyMatchToMmr(record);
    this.matches.unshift(record);
    this.savePersistedState();
    return { ...record };
  }

  public listMatches(modId?: string, search?: string): MatchRecord[] {
    const searchLower = search?.trim().toLowerCase() ?? '';
    return this.matches.filter((m) => {
      if (modId && modId.length > 0 && m.mod_id !== modId) {
        return false;
      }
      if (searchLower.length > 0) {
        const hay =
          `${m.lobby_name} ${m.winner_player} ${m.map_name} ${m.mod_id} ${m.final_state_hash}`.toLowerCase();
        if (!hay.includes(searchLower)) {
          return false;
        }
      }
      return true;
    });
  }

  public getLeaderboard(): CommanderRankEntry[] {
    const statsByPlayerLower = new Map<
      string,
      {
        displayName: string;
        playerId: string | null;
        wins: number;
        losses: number;
        factions: Record<string, number>;
        totalTicks: number;
        verifiedMatches: number;
        lastPlayedMs: number;
      }
    >();

    for (const m of this.matches) {
      const players =
        Array.isArray(m.players) && m.players.length > 0
          ? m.players
          : [
              {
                name: m.winner_player,
                faction: m.winner_faction,
                team: 1,
                is_winner: true,
              },
            ];

      for (const p of players) {
        const explicitPid =
          typeof p.player_id === 'string' && p.player_id.trim().length > 0
            ? p.player_id.trim()
            : null;
        const boundById = explicitPid
          ? this.playerBindingsById.get(explicitPid)
          : undefined;
        const name = boundById ? boundById.display_name : (p.name || '').trim();
        if (!name || name === 'Opponent') continue;
        const lower = name.toLowerCase();
        let rec = statsByPlayerLower.get(lower);
        if (!rec) {
          rec = {
            displayName: name,
            playerId: boundById?.player_id ?? explicitPid,
            wins: 0,
            losses: 0,
            factions: {},
            totalTicks: 0,
            verifiedMatches: 0,
            lastPlayedMs: 0,
          };
          statsByPlayerLower.set(lower, rec);
        } else if (!rec.playerId && (boundById?.player_id || explicitPid)) {
          rec.playerId = boundById?.player_id ?? explicitPid;
        }
        if (p.is_winner) {
          rec.wins += 1;
        } else {
          rec.losses += 1;
        }
        const fac = p.faction || m.winner_faction || 'Allied';
        rec.factions[fac] = (rec.factions[fac] ?? 0) + 1;
        rec.totalTicks += m.final_tick;
        if (!m.desync_detected) {
          rec.verifiedMatches += 1;
        }
        if (m.completed_at_unix_ms > rec.lastPlayedMs) {
          rec.lastPlayedMs = m.completed_at_unix_ms;
        }
      }
    }

    // Also include persisted MMR records if `matches` list was truncated or loaded from MMR snapshot
    for (const [lower, mmr] of this.playerMmrByNameLower.entries()) {
      if (mmr.wins + mmr.losses > 0 && !statsByPlayerLower.has(lower)) {
        statsByPlayerLower.set(lower, {
          displayName: mmr.display_name,
          playerId: mmr.player_id ?? null,
          wins: mmr.wins,
          losses: mmr.losses,
          factions: { Allied: mmr.wins + mmr.losses },
          totalTicks: 0,
          verifiedMatches: mmr.wins + mmr.losses,
          lastPlayedMs: 0,
        });
      }
    }

    const entries: CommanderRankEntry[] = [];
    for (const [lower, rec] of statsByPlayerLower.entries()) {
      const mmr = this.playerMmrByNameLower.get(lower);
      const wins = mmr ? mmr.wins : rec.wins;
      const losses = mmr ? mmr.losses : rec.losses;
      const total = wins + losses;
      let mainFaction = 'Allied';
      let bestFacCount = -1;
      for (const [fac, cnt] of Object.entries(rec.factions)) {
        if (cnt > bestFacCount) {
          bestFacCount = cnt;
          mainFaction = fac;
        }
      }
      const playerName = mmr?.display_name ?? rec.displayName;
      const rating = mmr ? mmr.rating : 1500;
      const rd = mmr ? mmr.rd : 350;
      const streak = mmr ? mmr.streak : 0;
      const playerId =
        mmr?.player_id ??
        this.playerIdByNameLower.get(lower) ??
        rec.playerId ??
        null;

      entries.push({
        rank: 0,
        player_name: playerName,
        player: playerName,
        main_faction: mainFaction,
        favorite_faction: mainFaction,
        wins,
        losses,
        matches_played: total,
        games: total,
        win_rate_pct: total > 0 ? Math.round((wins / total) * 100) : 0,
        total_ticks: rec.totalTicks,
        verified_matches: rec.verifiedMatches,
        last_played_unix_ms: rec.lastPlayedMs,
        rating,
        rd,
        streak,
        player_id: playerId,
      });
    }

    entries.sort((a, b) => {
      if (b.rating !== a.rating) return b.rating - a.rating;
      if (b.wins !== a.wins) return b.wins - a.wins;
      if (b.win_rate_pct !== a.win_rate_pct)
        return b.win_rate_pct - a.win_rate_pct;
      return b.total_ticks - a.total_ticks;
    });

    entries.forEach((e, idx) => {
      e.rank = idx + 1;
    });
    return entries;
  }

  public getStats(nowMs = Date.now()): RegistryStats {
    this.pruneExpired(nowMs);

    let openCount = 0;
    let fullCount = 0;
    let inGameCount = 0;
    let playersOnline = 0;
    const modsActive: Record<string, { lobbies: number; players: number }> = {};

    for (const entry of this.lobbies.values()) {
      if (entry.status === 'open') openCount++;
      else if (entry.status === 'full') fullCount++;
      else if (entry.status === 'in_game') inGameCount++;

      playersOnline += entry.current_players;
      if (!modsActive[entry.mod_id]) {
        modsActive[entry.mod_id] = { lobbies: 0, players: 0 };
      }
      modsActive[entry.mod_id].lobbies += 1;
      modsActive[entry.mod_id].players += entry.current_players;
    }

    const fleetCounts = this.dedicatedFleetProvider?.getFleetCounts(nowMs);

    return {
      status: 'online',
      server_version: '1.0.0',
      public_domain: process.env.PUBLIC_DOMAIN ?? 'rust-rts.bls-isp.net',
      node_id: process.env.NODE_ID ?? 'bls-isp-eu-central-01',
      uptime_secs: Math.max(0, Math.floor((nowMs - this.startedAtMs) / 1000)),
      heartbeat_ttl_secs: Math.floor(this.ttlMs / 1000),
      active_lobbies: this.lobbies.size,
      open_lobbies: openCount,
      full_lobbies: fullCount,
      in_game_lobbies: inGameCount,
      players_online: playersOnline,
      total_lobbies_registered: this.totalLobbiesRegistered,
      total_matches_completed: this.matches.length,
      relay_mux_port: this.relayMuxPort,
      relay_rooms_active: this.activeRelayRooms,
      udp_rendezvous_port: this.udpRendezvousPort,
      ...(fleetCounts
        ? {
            dedicated_servers_online: fleetCounts.online,
            dedicated_servers_idle: fleetCounts.idle,
          }
        : {}),
      mods_active: modsActive,
    };
  }
}
