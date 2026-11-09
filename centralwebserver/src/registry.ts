import * as fs from 'node:fs';
import * as path from 'node:path';
import { RelayPortRange, RelayTunnelRoom } from './relay';
import {
  CommanderRankEntry,
  CreateMatchRequest,
  LobbyAnnouncement,
  LobbyEntry,
  LobbyHeartbeatRequest,
  LobbyStatus,
  MatchRecord,
  RegistryStats,
} from './types';

function normalizePeerIp(ip?: string): string | undefined {
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

function clamp(val: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, val));
}

export interface LobbyFilterOptions {
  modId?: string;
  onlyOpen?: boolean;
  status?: LobbyStatus;
  search?: string;
}

export class CentralMultiplayerRegistry {
  private nextLobbyId = 1;
  private nextMatchId = 1;
  private readonly ttlMs: number;
  private readonly startedAtMs: number;
  private readonly relayPortRange?: RelayPortRange;
  private totalLobbiesRegistered = 0;
  private readonly lobbies = new Map<string, LobbyEntry>();
  private readonly relayRooms = new Map<string, RelayTunnelRoom>();
  private readonly matches: MatchRecord[] = [];
  private readonly stateFilePath?: string;

  constructor(
    ttlSecs = 45,
    initialMatches: MatchRecord[] = [],
    nowMs = Date.now(),
    relayPortRange?: RelayPortRange
  ) {
    this.ttlMs = Math.max(5, ttlSecs) * 1000;
    this.startedAtMs = nowMs;
    this.relayPortRange = relayPortRange;

    if (process.env.DATA_DIR && process.env.DATA_DIR.trim().length > 0) {
      this.stateFilePath = path.join(
        process.env.DATA_DIR.trim(),
        'registry-state.json'
      );
      this.loadPersistedMatches();
    }

    for (const m of initialMatches) {
      if (!this.matches.some((existing) => existing.match_id === m.match_id)) {
        this.matches.push(m);
        this.nextMatchId++;
      }
    }

    if (this.matches.length === 0) {
      this.seedReferenceMatches(nowMs);
    }
  }

  private loadPersistedMatches(): void {
    if (!this.stateFilePath || !fs.existsSync(this.stateFilePath)) return;
    try {
      const raw = fs.readFileSync(this.stateFilePath, 'utf-8');
      const parsed = JSON.parse(raw) as {
        totalLobbiesRegistered?: number;
        matches?: MatchRecord[];
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
    } catch {
      // ignore corrupted state file
    }
  }

  private savePersistedMatches(): void {
    if (!this.stateFilePath) return;
    try {
      const dir = path.dirname(this.stateFilePath);
      fs.mkdirSync(dir, { recursive: true });
      const payload = JSON.stringify(
        {
          totalLobbiesRegistered: this.totalLobbiesRegistered,
          matches: this.matches.slice(0, 500),
        },
        null,
        2
      );
      fs.writeFileSync(this.stateFilePath, payload, 'utf-8');
    } catch {
      // ignore write errors
    }
  }

  private seedReferenceMatches(nowMs: number): void {
    const seeds: MatchRecord[] = [
      {
        match_id: `match-${this.nextMatchId++}`,
        lobby_name: 'Operation Chrono Storm [1v1 Ladder]',
        mod_id: 'rust-ra2',
        mod_name: 'Red Alert 2: Rust Total Conversion',
        mod_version: '0.1.0',
        map_id: 'skirmish_valley',
        map_name: 'Skirmish Valley',
        game_mode: 'Standard',
        players: [
          { name: 'CommanderVance', faction: 'Allied', is_winner: true },
          { name: 'GenRomanov', faction: 'Soviet', is_winner: false },
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
        lobby_name: 'Iron Curtain Siege — River Crossing',
        mod_id: 'rust-ra2',
        mod_name: 'Red Alert 2: Rust Total Conversion',
        mod_version: '0.1.0',
        map_id: 'river_crossing',
        map_name: 'River Crossing',
        game_mode: 'Standard',
        players: [
          { name: 'KirovAirship', faction: 'Soviet', is_winner: true },
          { name: 'MirageTanker', faction: 'Allied', is_winner: false },
        ],
        winner_player: 'KirovAirship',
        winner_faction: 'Soviet',
        duration_secs: 1125,
        final_tick: 22500,
        final_state_hash: '0x84a91f027c3d51b8',
        total_commands: 892,
        desync_detected: false,
        replay_file: 'saves/river_crossing_siege.replay.json',
        completed_at_unix_ms: nowMs - 54 * 60 * 1000,
      },
      {
        match_id: `match-${this.nextMatchId++}`,
        lobby_name: 'Arctic Circle 2v2 Lockstep Verification',
        mod_id: 'rust-ra2',
        mod_name: 'Red Alert 2: Rust Total Conversion',
        mod_version: '0.1.0',
        map_id: 'mp_10_arctic_circle',
        map_name: 'Skirmish: Depth Charge (2-4)',
        game_mode: 'Team Skirmish',
        players: [
          { name: 'PrismCommander', faction: 'Allied', is_winner: true },
          { name: 'AegisCruiser', faction: 'Allied', is_winner: true },
          { name: 'TeslaTrooper', faction: 'Soviet', is_winner: false },
          { name: 'ApocTank', faction: 'Soviet', is_winner: false },
        ],
        winner_player: 'PrismCommander & AegisCruiser',
        winner_faction: 'Allied',
        duration_secs: 1560,
        final_tick: 31200,
        final_state_hash: '0x3e77d4190a5bc2e1',
        total_commands: 1420,
        desync_detected: false,
        replay_file: 'saves/arctic_circle_2v2.replay.json',
        completed_at_unix_ms: nowMs - 135 * 60 * 1000,
      },
      {
        match_id: `match-${this.nextMatchId++}`,
        lobby_name: 'Psi-Corps Mind Control Showdown [Yuri vs Soviet]',
        mod_id: 'rust-ra2',
        mod_name: 'Red Alert 2: Rust Total Conversion',
        mod_version: '0.1.0',
        map_id: 'mp_yr_austin_tx',
        map_name: 'Skirmish: Austin, TX (2-4)',
        game_mode: 'Standard',
        players: [
          { name: 'PsychicMaster', faction: 'Yuri', is_winner: true },
          { name: 'GenRomanov', faction: 'Soviet', is_winner: false },
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
        lobby_name: 'South Pacific Naval & Air Superiority [1v1]',
        mod_id: 'rust-ra2',
        mod_name: 'Red Alert 2: Rust Total Conversion',
        mod_version: '0.1.0',
        map_id: 'mp_01_offense_defense',
        map_name: 'Skirmish: South Pacific (2-4)',
        game_mode: 'Standard',
        players: [
          { name: 'CommanderVance', faction: 'Allied', is_winner: true },
          { name: 'PsychicMaster', faction: 'Yuri', is_winner: false },
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

  public pruneExpired(nowMs = Date.now()): void {
    for (const [id, entry] of this.lobbies.entries()) {
      if (Math.max(0, nowMs - entry.last_heartbeat_unix_ms) > this.ttlMs) {
        this.lobbies.delete(id);
        const room = this.relayRooms.get(id);
        if (room) {
          room.close();
          this.relayRooms.delete(id);
        }
      }
    }
  }

  public closeAllRelays(): void {
    for (const room of this.relayRooms.values()) {
      room.close();
    }
    this.relayRooms.clear();
  }

  public async registerOrUpdate(
    ann: LobbyAnnouncement,
    peerIp?: string,
    serverLocalIp?: string,
    nowMs = Date.now()
  ): Promise<LobbyEntry> {
    this.pruneExpired(nowMs);

    let hostAddr = String(ann.host_addr || '127.0.0.1:7878').trim();
    const hostPlayer = String(ann.host_player || 'HostPlayer').trim();
    const cleanIp = normalizePeerIp(peerIp);
    if (cleanIp && hostAddr.startsWith('0.0.0.0:')) {
      const port = hostAddr.slice('0.0.0.0:'.length);
      hostAddr = `${cleanIp}:${port}`;
    } else if (cleanIp && cleanIp !== '127.0.0.1' && hostAddr.startsWith('127.0.0.1:')) {
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

    const isNew = !existingId;
    const lobbyId = existingId ?? `lobby-${this.nextLobbyId++}`;
    if (isNew) {
      this.totalLobbiesRegistered++;
    }

    const existingEntry = this.lobbies.get(lobbyId);
    const createdAt = existingEntry ? existingEntry.created_at_unix_ms : nowMs;

    const maxPlayers = Math.max(2, Math.floor(Number(ann.max_players) || 2));
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
    if (ann.allocate_relay) {
      let room = this.relayRooms.get(lobbyId);
      if (!room) {
        try {
          room = await RelayTunnelRoom.create(this.relayPortRange);
          this.relayRooms.set(lobbyId, room);
        } catch {
          room = undefined;
        }
      }
      if (room) {
        relayAddr = `${srvIp}:${room.clientPort}`;
        relayUplinkAddr = `${srvIp}:${room.uplinkPort}`;
      }
    }

    const hasUpnp = Boolean(ann.upnp_external_addr && ann.upnp_external_addr.trim());
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
      host_player: String(ann.host_player || 'HostPlayer').trim(),
      host_addr: hostAddr,
      external_addr: externalAddr,
      relay_addr: relayAddr,
      relay_uplink_addr: relayUplinkAddr,
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
      password_protected: Boolean(ann.password_protected),
      created_at_unix_ms: createdAt,
      last_heartbeat_unix_ms: nowMs,
    };

    this.lobbies.set(lobbyId, entry);
    return { ...entry };
  }

  public heartbeat(
    lobbyId: string,
    hb: LobbyHeartbeatRequest,
    nowMs = Date.now()
  ): LobbyEntry | null {
    this.pruneExpired(nowMs);
    const entry = this.lobbies.get(lobbyId);
    if (!entry) return null;

    entry.max_players = Math.max(2, Math.floor(Number(hb.max_players) || entry.max_players));
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

  public deleteLobby(lobbyId: string, nowMs = Date.now()): boolean {
    this.pruneExpired(nowMs);
    const room = this.relayRooms.get(lobbyId);
    if (room) {
      room.close();
      this.relayRooms.delete(lobbyId);
    }
    return this.lobbies.delete(lobbyId);
  }

  public listLobbies(
    filters: LobbyFilterOptions = {},
    nowMs = Date.now()
  ): LobbyEntry[] {
    this.pruneExpired(nowMs);
    const searchLower = filters.search?.trim().toLowerCase() ?? '';

    const result: LobbyEntry[] = [];
    for (const entry of this.lobbies.values()) {
      if (filters.modId && filters.modId.length > 0 && entry.mod_id !== filters.modId) {
        continue;
      }
      if (filters.onlyOpen && entry.status !== 'open') {
        continue;
      }
      if (filters.status && entry.status !== filters.status) {
        continue;
      }
      if (searchLower.length > 0) {
        const haystack = `${entry.lobby_name} ${entry.host_player} ${entry.map_name} ${entry.map_id} ${entry.mod_id} ${entry.host_addr}`.toLowerCase();
        if (!haystack.includes(searchLower)) {
          continue;
        }
      }
      result.push({ ...entry });
    }

    return result;
  }

  public recordMatch(req: CreateMatchRequest, nowMs = Date.now()): MatchRecord {
    const finalTick = Math.max(1, Math.floor(Number(req.final_tick) || 1200));
    const durationSecs =
      req.duration_secs !== undefined
        ? Math.max(1, Math.floor(Number(req.duration_secs)))
        : Math.max(1, Math.round(finalTick / 20));

    let formattedHash = String(req.final_state_hash ?? '0xcbf29ce484222325').trim();
    if (typeof req.final_state_hash === 'number') {
      formattedHash = '0x' + req.final_state_hash.toString(16).padStart(16, '0');
    } else if (!formattedHash.startsWith('0x')) {
      formattedHash = '0x' + formattedHash;
    }

    const winnerPlayer = String(req.winner_player || 'Player 1').trim();
    const winnerFaction = String(req.winner_faction || 'Allied').trim();

    const players =
      Array.isArray(req.players) && req.players.length > 0
        ? req.players
        : [
            { name: winnerPlayer, faction: winnerFaction, is_winner: true },
            {
              name: 'Opponent',
              faction: winnerFaction === 'Allied' ? 'Soviet' : 'Allied',
              is_winner: false,
            },
          ];

    const record: MatchRecord = {
      match_id: `match-${this.nextMatchId++}`,
      lobby_name: String(req.lobby_name || 'Lockstep Multiplayer Match').trim(),
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
      total_commands: Math.max(1, Math.floor(Number(req.total_commands) || 240)),
      desync_detected: Boolean(req.desync_detected),
      replay_file: req.replay_file ? String(req.replay_file).trim() : undefined,
      completed_at_unix_ms: nowMs,
    };

    this.matches.unshift(record);
    this.savePersistedMatches();
    return { ...record };
  }

  public listMatches(modId?: string, search?: string): MatchRecord[] {
    const searchLower = search?.trim().toLowerCase() ?? '';
    return this.matches.filter((m) => {
      if (modId && modId.length > 0 && m.mod_id !== modId) {
        return false;
      }
      if (searchLower.length > 0) {
        const hay = `${m.lobby_name} ${m.winner_player} ${m.map_name} ${m.mod_id} ${m.final_state_hash}`.toLowerCase();
        if (!hay.includes(searchLower)) {
          return false;
        }
      }
      return true;
    });
  }

  public getLeaderboard(): CommanderRankEntry[] {
    const statsByPlayer = new Map<
      string,
      {
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
                is_winner: true,
              },
            ];

      for (const p of players) {
        const name = (p.name || '').trim();
        if (!name || name === 'Opponent') continue;
        let rec = statsByPlayer.get(name);
        if (!rec) {
          rec = {
            wins: 0,
            losses: 0,
            factions: {},
            totalTicks: 0,
            verifiedMatches: 0,
            lastPlayedMs: 0,
          };
          statsByPlayer.set(name, rec);
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

    const entries: CommanderRankEntry[] = [];
    for (const [playerName, rec] of statsByPlayer.entries()) {
      const total = rec.wins + rec.losses;
      let mainFaction = 'Allied';
      let bestFacCount = -1;
      for (const [fac, cnt] of Object.entries(rec.factions)) {
        if (cnt > bestFacCount) {
          bestFacCount = cnt;
          mainFaction = fac;
        }
      }
      entries.push({
        rank: 0,
        player_name: playerName,
        main_faction: mainFaction,
        wins: rec.wins,
        losses: rec.losses,
        matches_played: total,
        win_rate_pct: total > 0 ? Math.round((rec.wins / total) * 100) : 0,
        total_ticks: rec.totalTicks,
        verified_matches: rec.verifiedMatches,
        last_played_unix_ms: rec.lastPlayedMs,
      });
    }

    entries.sort((a, b) => {
      if (b.wins !== a.wins) return b.wins - a.wins;
      if (b.win_rate_pct !== a.win_rate_pct) return b.win_rate_pct - a.win_rate_pct;
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
      mods_active: modsActive,
    };
  }
}
