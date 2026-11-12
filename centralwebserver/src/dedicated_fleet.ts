import * as crypto from 'node:crypto';
import {
  CentralMultiplayerRegistry,
  DedicatedFleetStatsProvider,
  LobbyAuthError,
  normalizePeerIp,
  RegistryValidationError,
  verifySecretToken,
} from './registry';
import {
  DedicatedMatchAllocation,
  DedicatedServerAllocateRequest,
  DedicatedServerEntry,
  DedicatedServerHeartbeatRequest,
  DedicatedServerListResponse,
  DedicatedServerRegisterRequest,
  DedicatedServerRegisterResponse,
  DedicatedServerStatus,
} from './types';

export const DEFAULT_DEDICATED_SERVER_TTL_MS = 45_000;

const VALID_SERVER_STATUSES = new Set<DedicatedServerStatus>([
  'idle',
  'allocated',
  'staging',
  'in_game',
  'draining',
]);

const SAFE_SERVER_ID_RE = /^[a-zA-Z0-9._:-]{1,96}$/;
const SAFE_HOST_RE = /^[a-zA-Z0-9](?:[a-zA-Z0-9.-]{0,251}[a-zA-Z0-9])?$/;
const SAFE_IPV6_RE = /^[0-9a-fA-F:]{2,64}$/;

export class DedicatedFleetAuthError extends LobbyAuthError {
  constructor(statusCode: 401 | 403 | 409, message: string) {
    super(statusCode, message);
    this.name = 'DedicatedFleetAuthError';
  }
}

export interface DedicatedFleetOptions {
  ttlMs?: number;
  fleetSecret?: string;
}

export interface DedicatedServerFilterOptions {
  modId?: string;
  status?: string;
  region?: string;
}

interface InternalDedicatedServerRecord {
  entry: DedicatedServerEntry;
  lastCompletedMatchToken: string | null;
}

function clampInt(val: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, val));
}

function isDedicatedServerStatus(val: unknown): val is DedicatedServerStatus {
  return (
    typeof val === 'string' &&
    VALID_SERVER_STATUSES.has(val as DedicatedServerStatus)
  );
}

export function validateDedicatedTcpAddr(raw: unknown): {
  host: string;
  port: number;
  normalized: string;
} {
  if (typeof raw !== 'string') {
    throw new RegistryValidationError(
      'Invalid tcp_addr: expected non-empty host:port string'
    );
  }
  const trimmed = raw.trim();
  if (
    !trimmed ||
    /[\s\u0000-\u001F\u007F/\\?#]/.test(trimmed) ||
    trimmed.includes('..')
  ) {
    throw new RegistryValidationError(
      'Invalid tcp_addr: malformed host:port endpoint'
    );
  }

  let host = '';
  let portStr = '';
  let formattedHost = '';

  if (trimmed.startsWith('[')) {
    const closeIdx = trimmed.indexOf(']');
    if (closeIdx <= 1 || trimmed[closeIdx + 1] !== ':') {
      throw new RegistryValidationError(
        'Invalid tcp_addr: bracketed IPv6 endpoint must use [addr]:port'
      );
    }
    host = trimmed.slice(1, closeIdx);
    portStr = trimmed.slice(closeIdx + 2);
    if (!SAFE_IPV6_RE.test(host)) {
      throw new RegistryValidationError(
        'Invalid tcp_addr: malformed IPv6 literal'
      );
    }
    formattedHost = `[${host}]`;
  } else {
    const firstColon = trimmed.indexOf(':');
    const lastColon = trimmed.lastIndexOf(':');
    if (firstColon <= 0 || firstColon !== lastColon) {
      throw new RegistryValidationError(
        'Invalid tcp_addr: expected host:port format'
      );
    }
    host = trimmed.slice(0, lastColon);
    portStr = trimmed.slice(lastColon + 1);
    if (host.length !== 1 && !SAFE_HOST_RE.test(host)) {
      throw new RegistryValidationError(
        'Invalid tcp_addr: invalid hostname or IPv4 address'
      );
    }
    formattedHost = host;
  }

  if (!/^\d{1,5}$/.test(portStr)) {
    throw new RegistryValidationError(
      'Invalid tcp_addr: port must be an integer between 1 and 65535'
    );
  }
  const port = Number(portStr);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new RegistryValidationError(
      'Invalid tcp_addr: port must be between 1 and 65535'
    );
  }

  return {
    host,
    port,
    normalized: `${formattedHost}:${port}`,
  };
}

function normalizeMutators(raw?: unknown): string[] {
  if (!Array.isArray(raw)) return [];
  const seen = new Set<string>();
  const out: string[] = [];
  for (const item of raw) {
    if (typeof item !== 'string') continue;
    const trimmed = item.trim();
    if (!trimmed) continue;
    if (!seen.has(trimmed)) {
      seen.add(trimmed);
      out.push(trimmed);
    }
  }
  return out;
}

function areMutatorsCompatible(
  serverMutators: string[],
  requestedMutators: string[]
): boolean {
  if (requestedMutators.length === 0) return true;
  if (serverMutators.length !== requestedMutators.length) return false;
  const serverSet = new Set(serverMutators);
  for (const reqMut of requestedMutators) {
    if (!serverSet.has(reqMut)) return false;
  }
  return true;
}

function cloneAllocation(
  alloc: DedicatedMatchAllocation | null
): DedicatedMatchAllocation | null {
  if (!alloc) return null;
  return {
    ...alloc,
    active_mutators: [...alloc.active_mutators],
    players: alloc.players.map((p) => ({ ...p })),
  };
}

function cloneServerEntry(
  entry: DedicatedServerEntry,
  redactUplink = false
): DedicatedServerEntry {
  return {
    ...entry,
    active_mutators: [...entry.active_mutators],
    allocation: cloneAllocation(entry.allocation),
    relay_uplink_addr: redactUplink ? null : entry.relay_uplink_addr,
  };
}

/**
 * Orchestrates authoritative headless `rts-dedicated-server` instances:
 * registration, constant-time token heartbeats, TTL eviction, room recycling,
 * and automatic match allocation for Ranked Automatch & custom lobbies.
 */
export class DedicatedServerFleetManager implements DedicatedFleetStatsProvider {
  private nextServerSeq = 1;
  private readonly ttlMs: number;
  private readonly configuredFleetSecret?: string;
  private readonly registry: CentralMultiplayerRegistry;
  private readonly servers = new Map<string, InternalDedicatedServerRecord>();
  private readonly serverTokens = new Map<string, string>();
  private readonly knownSubmitterNamesLower = new Set<string>();

  constructor(
    registry: CentralMultiplayerRegistry,
    ttlMsOrOptions: number | DedicatedFleetOptions = DEFAULT_DEDICATED_SERVER_TTL_MS
  ) {
    this.registry = registry;
    if (typeof ttlMsOrOptions === 'number') {
      this.ttlMs = Math.max(250, ttlMsOrOptions);
    } else {
      this.ttlMs = Math.max(
        250,
        ttlMsOrOptions?.ttlMs ?? DEFAULT_DEDICATED_SERVER_TTL_MS
      );
      this.configuredFleetSecret = ttlMsOrOptions?.fleetSecret;
    }
    this.registry.setDedicatedFleetProvider(this);
  }

  private resolveExpectedFleetSecret(): string {
    return (
      this.configuredFleetSecret ??
      process.env.DEDICATED_FLEET_SECRET ??
      ''
    ).trim();
  }

  public pruneExpired(nowMs = Date.now()): void {
    for (const [serverId, rec] of this.servers.entries()) {
      const elapsed = Math.max(0, nowMs - rec.entry.last_heartbeat_unix_ms);
      if (elapsed > this.ttlMs) {
        this.servers.delete(serverId);
        this.serverTokens.delete(serverId);
        this.registry.releaseMuxRelayRoom(`ds:${serverId}`);
      }
    }
  }

  public getFleetCounts(nowMs = Date.now()): { online: number; idle: number } {
    this.pruneExpired(nowMs);
    let idle = 0;
    for (const rec of this.servers.values()) {
      if (rec.entry.status === 'idle') {
        idle += 1;
      }
    }
    return {
      online: this.servers.size,
      idle,
    };
  }

  public isKnownDedicatedSubmitter(submitterName: string): boolean {
    const clean = submitterName.trim().toLowerCase();
    if (!clean) return false;
    return this.knownSubmitterNamesLower.has(clean);
  }

  public verifyAnyServerToken(token: string): boolean {
    const candidate = token.trim();
    if (!candidate) return false;
    for (const stored of this.serverTokens.values()) {
      if (verifySecretToken(candidate, stored)) {
        return true;
      }
    }
    return false;
  }

  public async registerServer(
    req: DedicatedServerRegisterRequest,
    serverLocalIp?: string,
    nowMs = Date.now(),
    headerToken?: string,
    peerIp?: string,
    headerFleetSecret?: string
  ): Promise<DedicatedServerRegisterResponse> {
    this.pruneExpired(nowMs);

    if (!req || typeof req !== 'object') {
      throw new RegistryValidationError(
        'Invalid dedicated server registration payload'
      );
    }

    const parsedEndpoint = validateDedicatedTcpAddr(req.tcp_addr);
    let tcpAddr = parsedEndpoint.normalized;
    if (parsedEndpoint.host === '0.0.0.0') {
      const fallbackIp =
        normalizePeerIp(peerIp) || normalizePeerIp(serverLocalIp) || '127.0.0.1';
      tcpAddr = `${fallbackIp}:${parsedEndpoint.port}`;
    }

    const modId = typeof req.mod_id === 'string' ? req.mod_id.trim() : '';
    if (!modId) {
      throw new RegistryValidationError(
        'Invalid mod_id: non-empty string required'
      );
    }

    let serverId = '';
    if (req.server_id !== undefined && req.server_id !== null) {
      const rawId = String(req.server_id).trim();
      if (rawId.length > 0) {
        if (!SAFE_SERVER_ID_RE.test(rawId) || rawId.includes('..')) {
          throw new RegistryValidationError(
            'Invalid server_id: only alphanumeric, dot, colon, underscore, and hyphen characters are allowed'
          );
        }
        serverId = rawId;
      }
    }
    if (!serverId) {
      while (
        !serverId ||
        this.servers.has(serverId) ||
        this.serverTokens.has(serverId)
      ) {
        serverId = `ds-${this.nextServerSeq++}`;
      }
    }

    const existingRec = this.servers.get(serverId);
    const existingToken = this.serverTokens.get(serverId);
    const expectedFleetSecret = this.resolveExpectedFleetSecret();

    if (existingRec && existingToken) {
      const candidateServerToken = (
        headerToken ||
        req.server_token ||
        ''
      ).trim();
      if (!candidateServerToken) {
        throw new DedicatedFleetAuthError(
          401,
          'Server authentication token required to re-register an active server_id'
        );
      }
      if (!verifySecretToken(candidateServerToken, existingToken)) {
        throw new DedicatedFleetAuthError(
          403,
          'Invalid server authentication token for existing server_id'
        );
      }
      if (expectedFleetSecret) {
        const explicitFleetSecret = (
          req.fleet_secret ??
          headerFleetSecret ??
          ''
        ).trim();
        if (
          explicitFleetSecret.length > 0 &&
          !verifySecretToken(explicitFleetSecret, expectedFleetSecret)
        ) {
          throw new DedicatedFleetAuthError(
            403,
            'Invalid dedicated fleet secret'
          );
        }
      }
    } else if (expectedFleetSecret) {
      const candidateFleetSecret = (
        req.fleet_secret ??
        headerFleetSecret ??
        headerToken ??
        ''
      ).trim();
      if (!candidateFleetSecret) {
        throw new DedicatedFleetAuthError(
          401,
          'Dedicated fleet secret required to register dedicated server'
        );
      }
      if (!verifySecretToken(candidateFleetSecret, expectedFleetSecret)) {
        throw new DedicatedFleetAuthError(
          403,
          'Invalid dedicated fleet secret'
        );
      }
    }

    let udpPort: number | null = existingRec?.entry.udp_port ?? null;
    if (req.udp_port !== undefined) {
      if (req.udp_port === null || req.udp_port === 0) {
        udpPort = null;
      } else {
        const parsedUdp = Number(req.udp_port);
        if (
          !Number.isInteger(parsedUdp) ||
          parsedUdp < 1 ||
          parsedUdp > 65535
        ) {
          throw new RegistryValidationError(
            'Invalid udp_port: expected integer between 1 and 65535'
          );
        }
        udpPort = parsedUdp;
      }
    }

    let status: DedicatedServerStatus = existingRec?.entry.status ?? 'idle';
    if (req.status !== undefined) {
      if (!isDedicatedServerStatus(req.status)) {
        throw new RegistryValidationError(
          `Invalid dedicated server status '${String(req.status)}'`
        );
      }
      status = req.status;
    }

    const maxPlayers =
      req.max_players !== undefined
        ? clampInt(Math.floor(Number(req.max_players) || 2), 1, 32)
        : (existingRec?.entry.max_players ?? 8);
    const currentPlayers =
      req.current_players !== undefined
        ? clampInt(Math.floor(Number(req.current_players) || 0), 0, maxPlayers)
        : clampInt(existingRec?.entry.current_players ?? 0, 0, maxPlayers);

    const displayName =
      (req.display_name ?? existingRec?.entry.display_name ?? '').trim() ||
      serverId;
    const region =
      (req.region ?? existingRec?.entry.region ?? 'eu-central').trim() ||
      'eu-central';
    const modVersion =
      (req.mod_version ?? existingRec?.entry.mod_version ?? '0.1.0').trim() ||
      '0.1.0';
    const modContentHash = (
      req.mod_content_hash ??
      existingRec?.entry.mod_content_hash ??
      ''
    ).trim();
    const activeMutators =
      req.active_mutators !== undefined
        ? normalizeMutators(req.active_mutators)
        : [...(existingRec?.entry.active_mutators ?? [])];

    const srvIp = normalizePeerIp(serverLocalIp) || '127.0.0.1';
    let relayAddr: string | null = null;
    let relayUplinkAddr: string | null = null;
    let relayRoomToken: string | null = null;

    if (req.allocate_relay !== false) {
      const muxAlloc = await this.registry.allocateMuxRelayRoom(
        `ds:${serverId}`,
        srvIp
      );
      relayAddr = muxAlloc.relayAddr;
      relayUplinkAddr = muxAlloc.relayUplinkAddr;
      relayRoomToken = muxAlloc.roomToken;
    } else if (existingRec) {
      this.registry.releaseMuxRelayRoom(`ds:${serverId}`);
    }

    const serverToken =
      existingToken ?? crypto.randomBytes(32).toString('hex');
    this.serverTokens.set(serverId, serverToken);

    const entry: DedicatedServerEntry = {
      server_id: serverId,
      display_name: displayName,
      tcp_addr: tcpAddr,
      udp_port: udpPort,
      region,
      mod_id: modId,
      mod_version: modVersion,
      mod_content_hash: modContentHash,
      active_mutators: activeMutators,
      max_players: maxPlayers,
      current_players: currentPlayers,
      active_tick: existingRec?.entry.active_tick ?? 0,
      status,
      allocated_match_token:
        status === 'idle'
          ? null
          : (existingRec?.entry.allocated_match_token ?? null),
      allocated_lobby_id:
        status === 'idle'
          ? null
          : (existingRec?.entry.allocated_lobby_id ?? null),
      allocation:
        status === 'idle'
          ? null
          : cloneAllocation(existingRec?.entry.allocation ?? null),
      relay_addr: relayAddr,
      relay_uplink_addr: relayUplinkAddr,
      relay_room_token: relayRoomToken,
      registered_at_unix_ms: existingRec?.entry.registered_at_unix_ms ?? nowMs,
      last_heartbeat_unix_ms: nowMs,
      matches_hosted: existingRec?.entry.matches_hosted ?? 0,
    };

    this.servers.set(serverId, {
      entry,
      lastCompletedMatchToken: existingRec?.lastCompletedMatchToken ?? null,
    });
    this.knownSubmitterNamesLower.add(displayName.toLowerCase());
    this.knownSubmitterNamesLower.add(serverId.toLowerCase());

    return {
      ...cloneServerEntry(entry, false),
      server_token: serverToken,
    };
  }

  public heartbeat(
    req: DedicatedServerHeartbeatRequest,
    nowMs = Date.now(),
    headerToken?: string
  ): DedicatedServerEntry | null {
    this.pruneExpired(nowMs);

    if (!req || typeof req !== 'object') {
      throw new RegistryValidationError(
        'Invalid dedicated server heartbeat payload'
      );
    }

    const serverId =
      typeof req.server_id === 'string' ? req.server_id.trim() : '';
    if (!serverId) {
      throw new RegistryValidationError('server_id is required');
    }

    const rec = this.servers.get(serverId);
    if (!rec) {
      return null;
    }

    const expectedToken = this.serverTokens.get(serverId);
    const candidateToken = (headerToken || req.server_token || '').trim();
    if (!candidateToken) {
      throw new DedicatedFleetAuthError(
        401,
        'Missing server authentication token (server_token, x-server-token, or Authorization: Bearer)'
      );
    }
    if (!expectedToken || !verifySecretToken(candidateToken, expectedToken)) {
      throw new DedicatedFleetAuthError(
        403,
        'Invalid server authentication token'
      );
    }

    if (req.status !== undefined && !isDedicatedServerStatus(req.status)) {
      throw new RegistryValidationError(
        `Invalid dedicated server status '${String(req.status)}'`
      );
    }

    const entry = rec.entry;
    const prevStatus = entry.status;
    const prevMatchToken = entry.allocated_match_token;

    if (req.max_players !== undefined) {
      entry.max_players = clampInt(
        Math.floor(Number(req.max_players) || entry.max_players),
        1,
        32
      );
    }
    if (req.current_players !== undefined) {
      entry.current_players = clampInt(
        Math.floor(Number(req.current_players) || 0),
        0,
        entry.max_players
      );
    } else if (entry.current_players > entry.max_players) {
      entry.current_players = entry.max_players;
    }

    if (req.active_tick !== undefined) {
      entry.active_tick = Math.max(
        0,
        Math.floor(Number(req.active_tick) || 0)
      );
    }
    if (typeof req.mod_id === 'string' && req.mod_id.trim().length > 0) {
      entry.mod_id = req.mod_id.trim();
    }
    if (req.active_mutators !== undefined) {
      entry.active_mutators = normalizeMutators(req.active_mutators);
    }

    const completedToken =
      typeof req.completed_match_token === 'string' &&
      req.completed_match_token.trim().length > 0
        ? req.completed_match_token.trim()
        : null;

    const wasActiveMatch =
      prevStatus === 'allocated' ||
      prevStatus === 'staging' ||
      prevStatus === 'in_game';
    const recycledViaIdleTransition = req.status === 'idle' && wasActiveMatch;
    const recycledViaCompletedToken =
      completedToken !== null &&
      completedToken !== rec.lastCompletedMatchToken;

    if (recycledViaIdleTransition || recycledViaCompletedToken) {
      entry.matches_hosted += 1;
      rec.lastCompletedMatchToken =
        completedToken ??
        prevMatchToken ??
        `recycled-${nowMs}-${entry.matches_hosted}`;
      entry.allocated_match_token = null;
      entry.allocated_lobby_id = null;
      entry.allocation = null;
      entry.status = req.status ?? 'idle';
      if (req.current_players === undefined) {
        entry.current_players = 0;
      }
      if (req.active_tick === undefined) {
        entry.active_tick = 0;
      }
    } else if (
      completedToken !== null &&
      completedToken === rec.lastCompletedMatchToken
    ) {
      entry.allocated_match_token = null;
      entry.allocated_lobby_id = null;
      entry.allocation = null;
      entry.status = req.status ?? 'idle';
    } else if (req.status !== undefined) {
      entry.status = req.status;
    }

    entry.last_heartbeat_unix_ms = nowMs;
    return cloneServerEntry(entry, false);
  }

  public allocateServer(
    req: DedicatedServerAllocateRequest,
    nowMs = Date.now()
  ): DedicatedServerEntry | null {
    this.pruneExpired(nowMs);

    if (!req || typeof req !== 'object') {
      throw new RegistryValidationError(
        'Invalid dedicated server allocation payload'
      );
    }

    const modId = typeof req.mod_id === 'string' ? req.mod_id.trim() : '';
    if (!modId) {
      throw new RegistryValidationError(
        'mod_id is required to allocate a dedicated server'
      );
    }

    const modVersion =
      typeof req.mod_version === 'string' ? req.mod_version.trim() : '';
    const requestedMutators = normalizeMutators(req.active_mutators);
    const playersList = Array.isArray(req.players)
      ? req.players.map((p) => ({ ...p }))
      : [];
    const requiredPlayers =
      req.required_players !== undefined
        ? Math.max(1, Math.floor(Number(req.required_players) || 2))
        : Math.max(2, playersList.length || 2);
    const preferredRegion =
      typeof req.region === 'string' ? req.region.trim().toLowerCase() : '';

    const candidates: DedicatedServerEntry[] = [];
    for (const rec of this.servers.values()) {
      const srv = rec.entry;
      if (srv.status !== 'idle') continue;
      if (srv.mod_id !== modId) continue;
      if (modVersion.length > 0 && srv.mod_version !== modVersion) continue;
      if (srv.max_players < requiredPlayers) continue;
      if (
        requestedMutators.length > 0 &&
        !areMutatorsCompatible(srv.active_mutators, requestedMutators)
      ) {
        continue;
      }
      candidates.push(srv);
    }

    if (candidates.length === 0) {
      return null;
    }

    candidates.sort((a, b) => {
      if (preferredRegion.length > 0) {
        const aRegMatch = a.region.toLowerCase() === preferredRegion ? 0 : 1;
        const bRegMatch = b.region.toLowerCase() === preferredRegion ? 0 : 1;
        if (aRegMatch !== bRegMatch) return aRegMatch - bRegMatch;
      }
      if (requestedMutators.length === 0) {
        const aVanilla = a.active_mutators.length === 0 ? 0 : 1;
        const bVanilla = b.active_mutators.length === 0 ? 0 : 1;
        if (aVanilla !== bVanilla) return aVanilla - bVanilla;
      }
      if (a.matches_hosted !== b.matches_hosted) {
        return a.matches_hosted - b.matches_hosted;
      }
      return a.registered_at_unix_ms - b.registered_at_unix_ms;
    });

    const chosen = candidates[0];
    const matchToken =
      (typeof req.match_token === 'string' ? req.match_token.trim() : '') ||
      `ds-match-${crypto.randomBytes(8).toString('hex')}`;
    const lobbyId =
      (typeof req.lobby_id === 'string' ? req.lobby_id.trim() : '') ||
      this.registry.allocateNextLobbyId();
    const mode =
      (typeof req.mode === 'string' ? req.mode.trim() : '') ||
      (requiredPlayers === 4 ? '2v2' : '1v1');
    const mapId =
      (typeof req.map_id === 'string' ? req.map_id.trim() : '') ||
      (mode === '2v2' ? 'ra2_mp_02_iron_crossroads' : 'ra2_mp_01_urban_clash');
    const randomSeed =
      typeof req.random_seed === 'number' &&
      Number.isFinite(req.random_seed) &&
      req.random_seed > 0
        ? Math.floor(req.random_seed)
        : Math.max(1, crypto.randomBytes(6).readUIntBE(0, 6));
    const effectiveMutators =
      requestedMutators.length > 0
        ? [...requestedMutators]
        : [...chosen.active_mutators];

    const allocation: DedicatedMatchAllocation = {
      match_token: matchToken,
      lobby_id: lobbyId,
      mode,
      mod_id: modId,
      active_mutators: effectiveMutators,
      map_id: mapId,
      random_seed: randomSeed,
      expected_players: requiredPlayers,
      players: playersList,
      allocated_at_unix_ms: nowMs,
    };

    chosen.status = 'allocated';
    chosen.allocated_match_token = matchToken;
    chosen.allocated_lobby_id = lobbyId;
    chosen.allocation = allocation;

    return cloneServerEntry(chosen, false);
  }

  public listServers(
    filters: DedicatedServerFilterOptions = {},
    nowMs = Date.now()
  ): DedicatedServerListResponse {
    this.pruneExpired(nowMs);

    const modIdFilter = filters.modId?.trim() ?? '';
    const statusFilter = filters.status?.trim() ?? '';
    const regionFilter = filters.region?.trim().toLowerCase() ?? '';

    const servers: DedicatedServerEntry[] = [];
    for (const rec of this.servers.values()) {
      const srv = rec.entry;
      if (modIdFilter.length > 0 && srv.mod_id !== modIdFilter) {
        continue;
      }
      if (statusFilter.length > 0 && srv.status !== statusFilter) {
        continue;
      }
      if (
        regionFilter.length > 0 &&
        srv.region.toLowerCase() !== regionFilter
      ) {
        continue;
      }
      // Redact internal host-only relay_uplink_addr on public listings
      servers.push(cloneServerEntry(srv, true));
    }

    let idle = 0;
    let active = 0;
    for (const srv of servers) {
      if (srv.status === 'idle') {
        idle += 1;
      } else if (
        srv.status === 'allocated' ||
        srv.status === 'staging' ||
        srv.status === 'in_game'
      ) {
        active += 1;
      }
    }

    return {
      servers,
      total: servers.length,
      idle,
      active,
    };
  }
}
