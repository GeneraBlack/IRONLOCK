import * as crypto from 'node:crypto';
import { DedicatedServerFleetManager } from './dedicated_fleet';
import {
  CentralMultiplayerRegistry,
  normalizePeerIp,
  RegistryValidationError,
} from './registry';
import {
  AutomatchAssignment,
  AutomatchEnqueueRequest,
  AutomatchMode,
  AutomatchPlayerAssignment,
  AutomatchPollResponse,
  AutomatchPollStatus,
} from './types';

export const AUTOMATCH_TICKET_TTL_MS = 90_000;

interface InternalAutomatchTicket {
  ticketId: string;
  playerId: string;
  displayName: string;
  modId: string;
  mode: AutomatchMode;
  preferredFaction: string;
  hostAddr: string | null;
  upnpExternalAddr: string | null;
  rating: number;
  enqueuedAtMs: number;
  lastPolledAtMs: number;
  status: AutomatchPollStatus;
  assignment?: AutomatchAssignment;
}

export function computeAutomatchSearchWindow(
  enqueuedAtMs: number,
  nowMs: number
): number {
  const waitSecs = Math.max(0, nowMs - enqueuedAtMs) / 1000;
  return Math.min(1500, Math.round((100 + waitSecs * 25) * 100) / 100);
}

/**
 * 1v1 and 2v2 Glicko-2 / ELO Automatch Queue Manager (`/api/v1/matchmaking/*`).
 * Pairs commanders per `(mod_id, mode)` within an expanding MMR window and
 * automatically provisions a token-multiplexed single-port relay room.
 */
export class MatchmakingQueueManager {
  private nextTicketSeq = 1;
  private readonly tickets = new Map<string, InternalAutomatchTicket>();
  private readonly registry: CentralMultiplayerRegistry;
  private readonly ticketTtlMs: number;
  private readonly dedicatedFleet?: DedicatedServerFleetManager;

  constructor(
    registry: CentralMultiplayerRegistry,
    ticketTtlMsOrFleet: number | DedicatedServerFleetManager = AUTOMATCH_TICKET_TTL_MS,
    dedicatedFleet?: DedicatedServerFleetManager
  ) {
    this.registry = registry;
    if (typeof ticketTtlMsOrFleet === 'number') {
      this.ticketTtlMs = Math.max(5_000, ticketTtlMsOrFleet);
      this.dedicatedFleet = dedicatedFleet;
    } else {
      this.ticketTtlMs = AUTOMATCH_TICKET_TTL_MS;
      this.dedicatedFleet = ticketTtlMsOrFleet ?? dedicatedFleet;
    }
  }

  public pruneExpired(nowMs = Date.now()): void {
    for (const [id, ticket] of this.tickets.entries()) {
      const idleMs = Math.max(0, nowMs - ticket.lastPolledAtMs);
      if (ticket.status === 'searching' && idleMs > this.ticketTtlMs) {
        ticket.status = 'expired';
      }
      if (idleMs > this.ticketTtlMs * 2) {
        if (ticket.assignment) {
          this.registry.releaseMuxRelayRoom(
            `am:${ticket.assignment.match_token}`
          );
        }
        this.tickets.delete(id);
      }
    }
  }

  public async enqueue(
    req: AutomatchEnqueueRequest,
    serverLocalIp?: string,
    nowMs = Date.now()
  ): Promise<AutomatchPollResponse> {
    this.pruneExpired(nowMs);

    const mode: AutomatchMode | null =
      req.mode === '1v1' || req.mode === '2v2' ? req.mode : null;
    if (!mode) {
      throw new RegistryValidationError(
        "Invalid automatch mode: expected '1v1' or '2v2'"
      );
    }

    // Cryptographically bind/verify player identity before entering ranked automatch
    const bound = this.registry.registerPlayerBinding(
      req.player_id,
      req.display_name,
      req.public_token
    );

    // Cancel any prior active searching ticket owned by the same player_id
    for (const existing of this.tickets.values()) {
      if (existing.playerId === bound.player_id && existing.status === 'searching') {
        existing.status = 'cancelled';
      }
    }

    const rating = this.registry.getPlayerRating(bound.display_name);
    const ticketId = `tkt-${this.nextTicketSeq++}`;
    const modId = String(req.mod_id || 'rust-ra2').trim() || 'rust-ra2';
    const preferredFaction =
      String(req.preferred_faction || 'random').trim() || 'random';

    const hostAddr =
      typeof req.host_addr === 'string' && req.host_addr.trim().length > 0
        ? req.host_addr.trim()
        : null;
    const upnpExternalAddr =
      typeof req.upnp_external_addr === 'string' &&
      req.upnp_external_addr.trim().length > 0
        ? req.upnp_external_addr.trim()
        : null;

    this.tickets.set(ticketId, {
      ticketId,
      playerId: bound.player_id,
      displayName: bound.display_name,
      modId,
      mode,
      preferredFaction,
      hostAddr,
      upnpExternalAddr,
      rating,
      enqueuedAtMs: nowMs,
      lastPolledAtMs: nowMs,
      status: 'searching',
    });

    const srvIp = normalizePeerIp(serverLocalIp) || '127.0.0.1';
    await this.tryPairQueues(srvIp, nowMs);
    const polled = await this.poll(ticketId, srvIp, nowMs);
    if (!polled) {
      throw new Error('Automatch ticket missing immediately after enqueue');
    }
    return polled;
  }

  public async poll(
    ticketIdRaw: string,
    serverLocalIp?: string,
    nowMs = Date.now()
  ): Promise<AutomatchPollResponse | null> {
    const ticketId = String(ticketIdRaw ?? '').trim();
    if (!ticketId) return null;

    const existing = this.tickets.get(ticketId);
    if (!existing) return null;

    existing.lastPolledAtMs = nowMs;
    this.pruneExpired(nowMs);

    const srvIp = normalizePeerIp(serverLocalIp) || '127.0.0.1';
    await this.tryPairQueues(srvIp, nowMs);

    const updated = this.tickets.get(ticketId);
    if (!updated) return null;

    return {
      ticket_id: updated.ticketId,
      status: updated.status,
      rating: updated.rating,
      search_window: computeAutomatchSearchWindow(updated.enqueuedAtMs, nowMs),
      ...(updated.assignment ? { assignment: updated.assignment } : {}),
    };
  }

  public cancel(
    ticketIdRaw: string,
    playerIdRaw: string,
    nowMs = Date.now()
  ): boolean {
    this.pruneExpired(nowMs);
    const ticketId = String(ticketIdRaw ?? '').trim();
    const playerId = String(playerIdRaw ?? '').trim();
    if (!ticketId || !playerId) return false;

    const ticket = this.tickets.get(ticketId);
    if (!ticket) return false;
    if (ticket.playerId !== playerId || ticket.status !== 'searching') {
      return false;
    }
    ticket.status = 'cancelled';
    ticket.lastPolledAtMs = nowMs;
    return true;
  }

  private async tryPairQueues(srvIp: string, nowMs: number): Promise<void> {
    const buckets = new Map<string, InternalAutomatchTicket[]>();
    for (const t of this.tickets.values()) {
      if (t.status === 'searching') {
        const key = `${t.modId}::${t.mode}`;
        let arr = buckets.get(key);
        if (!arr) {
          arr = [];
          buckets.set(key, arr);
        }
        arr.push(t);
      }
    }

    for (const group of buckets.values()) {
      if (group.length < 2) continue;
      group.sort((a, b) => a.rating - b.rating);
      const needed = group[0].mode === '2v2' ? 4 : 2;
      let idx = 0;

      while (idx + needed <= group.length) {
        const slice = group.slice(idx, idx + needed);
        const minRating = slice[0].rating;
        const maxRating = slice[needed - 1].rating;
        const maxWindow = slice.reduce(
          (acc, t) =>
            Math.max(acc, computeAutomatchSearchWindow(t.enqueuedAtMs, nowMs)),
          100
        );

        if (maxRating - minRating <= maxWindow) {
          await this.finalizeMatchedGroup(slice, srvIp, nowMs);
          idx += needed;
        } else {
          idx += 1;
        }
      }
    }
  }

  private async finalizeMatchedGroup(
    slice: InternalAutomatchTicket[],
    srvIp: string,
    nowMs: number
  ): Promise<void> {
    // Sort descending by rating so 2v2 uses balanced snake-draft [1, 2, 2, 1] (#1+#4 vs #2+#3)
    const sorted = [...slice].sort((a, b) => b.rating - a.rating);
    const mode = sorted[0].mode;
    const modId = sorted[0].modId;
    const matchToken = `am-${crypto.randomBytes(10).toString('hex')}`;
    const lobbyId = this.registry.allocateNextLobbyId();

    const muxAlloc = await this.registry.allocateMuxRelayRoom(
      `am:${matchToken}`,
      srvIp
    );

    const randomSeed = Math.max(1, crypto.randomBytes(6).readUIntBE(0, 6));
    const mapId =
      mode === '2v2' ? 'ra2_mp_02_iron_crossroads' : 'ra2_mp_01_urban_clash';

    const teamPattern = mode === '2v2' ? [1, 2, 2, 1] : [1, 2];
    const players: AutomatchPlayerAssignment[] = sorted.map((m, i) => ({
      player_id: m.playerId,
      display_name: m.displayName,
      slot_index: i,
      assigned_player_id: i + 1,
      team_id: teamPattern[i] ?? 1,
      preferred_faction: m.preferredFaction,
      rating: m.rating,
    }));

    const dedicatedNode =
      this.dedicatedFleet?.allocateServer(
        {
          mod_id: modId,
          required_players: sorted.length,
          map_id: mapId,
          mode,
          match_token: matchToken,
          lobby_id: lobbyId,
          random_seed: randomSeed,
          players,
        },
        nowMs
      ) ?? null;

    if (dedicatedNode) {
      const dedicatedCandidates: string[] = [
        dedicatedNode.tcp_addr,
        ...(dedicatedNode.relay_addr ? [dedicatedNode.relay_addr] : []),
        muxAlloc.relayAddr,
      ];
      const effectiveRelayAddr =
        dedicatedNode.relay_addr ?? muxAlloc.relayAddr;
      const effectiveUplinkAddr =
        dedicatedNode.relay_uplink_addr ?? muxAlloc.relayUplinkAddr;
      const effectiveRoomToken =
        dedicatedNode.relay_room_token ?? muxAlloc.roomToken;

      sorted.forEach((m, i) => {
        const live = this.tickets.get(m.ticketId);
        if (!live) return;
        live.status = 'matched';
        live.assignment = {
          match_token: matchToken,
          lobby_id: lobbyId,
          mode,
          mod_id: modId,
          map_id: mapId,
          random_seed: randomSeed,
          host_player_id: dedicatedNode.server_id,
          is_host: false,
          assigned_player_id: i + 1,
          relay_addr: effectiveRelayAddr,
          relay_uplink_addr: effectiveUplinkAddr,
          relay_room_token: effectiveRoomToken,
          candidate_addrs: [...dedicatedCandidates],
          players: players.map((p) => ({ ...p })),
          authoritative_dedicated: true,
          dedicated_server_id: dedicatedNode.server_id,
          dedicated_addr: dedicatedNode.tcp_addr,
        };
      });
      return;
    }

    const hostMember = sorted[0];
    const candidateAddrs: string[] = [];
    if (hostMember.upnpExternalAddr) {
      candidateAddrs.push(hostMember.upnpExternalAddr);
    }
    if (
      hostMember.hostAddr &&
      !candidateAddrs.includes(hostMember.hostAddr)
    ) {
      candidateAddrs.push(hostMember.hostAddr);
    }
    candidateAddrs.push(muxAlloc.relayAddr);

    sorted.forEach((m, i) => {
      const live = this.tickets.get(m.ticketId);
      if (!live) return;
      live.status = 'matched';
      live.assignment = {
        match_token: matchToken,
        lobby_id: lobbyId,
        mode,
        mod_id: modId,
        map_id: mapId,
        random_seed: randomSeed,
        host_player_id: hostMember.playerId,
        is_host: i === 0,
        assigned_player_id: i + 1,
        relay_addr: muxAlloc.relayAddr,
        relay_uplink_addr: muxAlloc.relayUplinkAddr,
        relay_room_token: muxAlloc.roomToken,
        candidate_addrs: [...candidateAddrs],
        players: players.map((p) => ({ ...p })),
        authoritative_dedicated: false,
        dedicated_server_id: null,
        dedicated_addr: null,
      };
    });
  }
}
