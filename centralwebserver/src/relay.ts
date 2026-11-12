import * as crypto from 'node:crypto';
import * as dgram from 'node:dgram';
import * as net from 'node:net';

export interface RelayPortRange {
  start: number;
  end: number;
}

export interface RelayRoomOptions {
  idleTimeoutMs?: number;
  maxWaitingPerSide?: number;
}

export const DEFAULT_RELAY_IDLE_TIMEOUT_MS = 120_000;
export const DEFAULT_MAX_WAITING_PER_SIDE = 16;
export const DEFAULT_UDP_RENDEZVOUS_PORT = 30098;
export const DEFAULT_RELAY_MUX_PORT = 30099;
export const DEFAULT_MAX_MUX_ROOMS = 4096;
export const MAX_MUX_PREAMBLE_BYTES = 256;
export const MAX_UDP_RENDEZVOUS_BYTES = 2048;
export const DEFAULT_UDP_SESSION_TTL_MS = 60_000;
export const DEFAULT_RELAY_PORT_RANGE: RelayPortRange = {
  start: 30100,
  end: 30139,
};

export function formatMuxEndpoint(addrPort: string, roomToken: string): string {
  return `${addrPort.trim()}#mux:${roomToken.trim()}`;
}

export function resolveDefaultRelayPortRange(): RelayPortRange {
  const envStart = Number(process.env.RELAY_PORT_START) || 0;
  const envEnd = Number(process.env.RELAY_PORT_END) || 0;
  if (envStart > 0 && envEnd >= envStart) {
    return { start: envStart, end: envEnd };
  }
  return { ...DEFAULT_RELAY_PORT_RANGE };
}

export class RelayCapacityError extends Error {
  public readonly code = 'RELAY_POOL_EXHAUSTED';

  constructor(message: string) {
    super(message);
    this.name = 'RelayCapacityError';
  }
}

export function maxRelayRoomsForRange(range?: RelayPortRange): number {
  if (!range || range.start <= 0 || range.end < range.start) {
    return 0;
  }
  return Math.floor((range.end - range.start + 1) / 2);
}

function closeNetServer(srv: net.Server): Promise<void> {
  return new Promise((resolve) => {
    try {
      srv.close(() => resolve());
    } catch {
      resolve();
    }
  });
}

function listenInConfiguredRange(
  srv: net.Server,
  range: RelayPortRange,
  startFromPort?: number
): Promise<void> {
  if (maxRelayRoomsForRange(range) < 1) {
    return Promise.reject(
      new RelayCapacityError(
        `Invalid relay port range ${range.start}-${range.end}: at least 2 ports are required per room`
      )
    );
  }

  return new Promise<void>((resolve, reject) => {
    let port = Math.max(range.start, startFromPort ?? range.start);
    const tryNext = (): void => {
      if (port > range.end) {
        reject(
          new RelayCapacityError(
            `Relay port pool exhausted: all ports in range ${range.start}-${range.end} (${maxRelayRoomsForRange(
              range
            )} rooms max) are currently in use`
          )
        );
        return;
      }
      const currentPort = port++;
      const onError = (err: NodeJS.ErrnoException): void => {
        srv.removeListener('error', onError);
        if (err.code === 'EADDRINUSE' || err.code === 'EACCES') {
          tryNext();
        } else {
          reject(err);
        }
      };
      srv.once('error', onError);
      srv.listen(currentPort, '0.0.0.0', () => {
        srv.removeListener('error', onError);
        resolve();
      });
    };
    tryNext();
  });
}

interface WaitingSocketEntry {
  socket: net.Socket;
  onTimeout: () => void;
}

export class RelayTunnelRoom {
  public readonly clientPort: number;
  public readonly uplinkPort: number;
  public readonly idleTimeoutMs: number;
  private readonly maxWaitingPerSide: number;
  private readonly clientServer: net.Server;
  private readonly uplinkServer: net.Server;
  private readonly waitingHostUplinks: WaitingSocketEntry[] = [];
  private readonly waitingClients: WaitingSocketEntry[] = [];
  private readonly activeSockets = new Set<net.Socket>();
  private closed = false;

  private constructor(
    clientServer: net.Server,
    uplinkServer: net.Server,
    clientPort: number,
    uplinkPort: number,
    options: RelayRoomOptions = {}
  ) {
    this.clientServer = clientServer;
    this.uplinkServer = uplinkServer;
    this.clientPort = clientPort;
    this.uplinkPort = uplinkPort;
    this.idleTimeoutMs = Math.max(
      1_000,
      options.idleTimeoutMs ?? DEFAULT_RELAY_IDLE_TIMEOUT_MS
    );
    this.maxWaitingPerSide = Math.max(
      1,
      options.maxWaitingPerSide ?? DEFAULT_MAX_WAITING_PER_SIDE
    );

    this.uplinkServer.on('connection', (socket) => {
      this.registerWaitingSocket(this.waitingHostUplinks, socket);
    });

    this.clientServer.on('connection', (socket) => {
      this.registerWaitingSocket(this.waitingClients, socket);
    });
  }

  public static async create(
    range?: RelayPortRange,
    options?: RelayRoomOptions
  ): Promise<RelayTunnelRoom> {
    const effectiveRange = range ?? resolveDefaultRelayPortRange();

    const clientServer = net.createServer();
    const uplinkServer = net.createServer();

    try {
      await listenInConfiguredRange(clientServer, effectiveRange);
      const cAddr = clientServer.address();
      const nextHint =
        cAddr && typeof cAddr !== 'string' ? cAddr.port + 1 : undefined;
      await listenInConfiguredRange(uplinkServer, effectiveRange, nextHint);
    } catch (err) {
      await Promise.all([
        closeNetServer(clientServer),
        closeNetServer(uplinkServer),
      ]);
      throw err;
    }

    const cAddr = clientServer.address();
    const uAddr = uplinkServer.address();
    if (
      !cAddr ||
      typeof cAddr === 'string' ||
      !uAddr ||
      typeof uAddr === 'string'
    ) {
      await Promise.all([
        closeNetServer(clientServer),
        closeNetServer(uplinkServer),
      ]);
      throw new Error('Failed to bind RelayTunnelRoom TCP ports');
    }

    return new RelayTunnelRoom(
      clientServer,
      uplinkServer,
      cAddr.port,
      uAddr.port,
      options
    );
  }

  private registerWaitingSocket(
    queue: WaitingSocketEntry[],
    socket: net.Socket
  ): void {
    if (this.closed) {
      socket.destroy();
      return;
    }

    this.pruneDestroyedFromQueue(queue);
    while (queue.length >= this.maxWaitingPerSide) {
      const evicted = queue.shift();
      if (evicted) {
        evicted.socket.removeListener('timeout', evicted.onTimeout);
        this.activeSockets.delete(evicted.socket);
        evicted.socket.destroy();
      }
    }

    socket.setNoDelay(true);
    socket.setKeepAlive(true, 30_000);
    socket.setTimeout(this.idleTimeoutMs);
    this.activeSockets.add(socket);

    const onTimeout = (): void => {
      this.removeWaiting(queue, socket);
      this.activeSockets.delete(socket);
      socket.destroy();
    };

    socket.once('timeout', onTimeout);
    socket.on('error', () => {
      socket.removeListener('timeout', onTimeout);
      this.removeWaiting(queue, socket);
      this.activeSockets.delete(socket);
      if (!socket.destroyed) {
        socket.destroy();
      }
    });
    socket.on('close', () => {
      socket.removeListener('timeout', onTimeout);
      this.removeWaiting(queue, socket);
      this.activeSockets.delete(socket);
    });

    queue.push({ socket, onTimeout });
    this.tryPair();
  }

  private pruneDestroyedFromQueue(queue: WaitingSocketEntry[]): void {
    for (let i = queue.length - 1; i >= 0; i--) {
      const item = queue[i];
      if (item.socket.destroyed || !item.socket.writable) {
        item.socket.removeListener('timeout', item.onTimeout);
        this.activeSockets.delete(item.socket);
        if (!item.socket.destroyed) {
          item.socket.destroy();
        }
        queue.splice(i, 1);
      }
    }
  }

  private removeWaiting(queue: WaitingSocketEntry[], target: net.Socket): void {
    const idx = queue.findIndex((item) => item.socket === target);
    if (idx !== -1) {
      const [removed] = queue.splice(idx, 1);
      removed.socket.removeListener('timeout', removed.onTimeout);
    }
  }

  private tryPair(): void {
    this.pruneDestroyedFromQueue(this.waitingHostUplinks);
    this.pruneDestroyedFromQueue(this.waitingClients);

    while (
      this.waitingClients.length > 0 &&
      this.waitingHostUplinks.length > 0
    ) {
      const hostItem = this.waitingHostUplinks.shift()!;
      const hostSocket = hostItem.socket;
      hostSocket.removeListener('timeout', hostItem.onTimeout);
      if (hostSocket.destroyed || !hostSocket.writable) {
        this.activeSockets.delete(hostSocket);
        continue;
      }

      const clientItem = this.waitingClients.shift()!;
      const clientSocket = clientItem.socket;
      clientSocket.removeListener('timeout', clientItem.onTimeout);
      if (clientSocket.destroyed || !clientSocket.writable) {
        this.activeSockets.delete(clientSocket);
        // Re-arm idle timeout on hostSocket and return it to front of queue
        hostSocket.setTimeout(this.idleTimeoutMs);
        hostSocket.once('timeout', hostItem.onTimeout);
        this.waitingHostUplinks.unshift(hostItem);
        continue;
      }

      // Clear pre-game idle timeout now that the pair is promoted to an active tunnel
      hostSocket.setTimeout(0);
      clientSocket.setTimeout(0);

      const teardownBoth = (): void => {
        if (!clientSocket.destroyed) clientSocket.destroy();
        if (!hostSocket.destroyed) hostSocket.destroy();
        this.activeSockets.delete(clientSocket);
        this.activeSockets.delete(hostSocket);
      };

      clientSocket.once('close', teardownBoth);
      clientSocket.once('error', teardownBoth);
      hostSocket.once('close', teardownBoth);
      hostSocket.once('error', teardownBoth);

      // Send 1-byte wakeup marker 'R' so the Host's LockstepServer promotes this standby uplink to an active peer
      hostSocket.write(Buffer.from('R'), (err) => {
        if (err) {
          teardownBoth();
          return;
        }
        clientSocket.pipe(hostSocket);
        hostSocket.pipe(clientSocket);
      });
    }
  }

  public get waitingClientCount(): number {
    this.pruneDestroyedFromQueue(this.waitingClients);
    return this.waitingClients.length;
  }

  public get waitingUplinkCount(): number {
    this.pruneDestroyedFromQueue(this.waitingHostUplinks);
    return this.waitingHostUplinks.length;
  }

  public close(): void {
    if (this.closed) return;
    this.closed = true;
    for (const item of this.waitingClients) {
      item.socket.removeListener('timeout', item.onTimeout);
    }
    for (const item of this.waitingHostUplinks) {
      item.socket.removeListener('timeout', item.onTimeout);
    }
    for (const s of this.activeSockets) {
      s.destroy();
    }
    this.activeSockets.clear();
    this.waitingClients.length = 0;
    this.waitingHostUplinks.length = 0;
    this.clientServer.close();
    this.uplinkServer.close();
  }
}

export interface MultiplexedRelayOptions extends RelayRoomOptions {
  maxRooms?: number;
  handshakeTimeoutMs?: number;
  allowEphemeralFallback?: boolean;
}

interface MuxWaitingHostEntry {
  socket: net.Socket;
  onTimeout: () => void;
}

interface MuxWaitingClientEntry {
  socket: net.Socket;
  trailing: Buffer;
  onTimeout: () => void;
}

interface MuxRoomBucket {
  waitingHosts: MuxWaitingHostEntry[];
  waitingClients: MuxWaitingClientEntry[];
  activeSockets: Set<net.Socket>;
}

/**
 * Token-multiplexed single-port TCP relay server (`RELAY_MUX_PORT`, default 30099).
 * Host standby uplinks send `MUX <room_token> HOST\n` and joining clients/spectators send
 * `MUX <room_token> CLIENT\n` before raw length-prefixed lockstep framing, supporting 500+
 * concurrent rooms on 1 TCP listener port with zero port-range exhaustion.
 */
export class MultiplexedRelayServer {
  public readonly port: number;
  public readonly idleTimeoutMs: number;
  private readonly maxWaitingPerSide: number;
  private readonly maxRooms: number;
  private readonly handshakeTimeoutMs: number;
  private readonly server: net.Server;
  private readonly pendingSockets = new Set<net.Socket>();
  private readonly rooms = new Map<string, MuxRoomBucket>();
  private closed = false;

  private constructor(
    server: net.Server,
    port: number,
    options: MultiplexedRelayOptions = {}
  ) {
    this.server = server;
    this.port = port;
    this.idleTimeoutMs = Math.max(
      1_000,
      options.idleTimeoutMs ?? DEFAULT_RELAY_IDLE_TIMEOUT_MS
    );
    this.maxWaitingPerSide = Math.max(
      1,
      options.maxWaitingPerSide ?? DEFAULT_MAX_WAITING_PER_SIDE
    );
    this.maxRooms = Math.max(1, options.maxRooms ?? DEFAULT_MAX_MUX_ROOMS);
    this.handshakeTimeoutMs = Math.max(
      500,
      options.handshakeTimeoutMs ?? 10_000
    );

    this.server.on('connection', (socket) => {
      this.handleIncomingConnection(socket);
    });
  }

  public static async create(
    preferredPort = 0,
    host = '0.0.0.0',
    options: MultiplexedRelayOptions = {}
  ): Promise<MultiplexedRelayServer> {
    const srv = net.createServer();
    const allowFallback = options.allowEphemeralFallback ?? true;

    await new Promise<void>((resolve, reject) => {
      const tryBind = (targetPort: number, canFallback: boolean): void => {
        const onError = (err: NodeJS.ErrnoException): void => {
          srv.removeListener('error', onError);
          if (
            canFallback &&
            targetPort !== 0 &&
            (err.code === 'EADDRINUSE' || err.code === 'EACCES')
          ) {
            tryBind(0, false);
          } else {
            reject(err);
          }
        };
        srv.once('error', onError);
        srv.listen(targetPort, host, () => {
          srv.removeListener('error', onError);
          resolve();
        });
      };
      tryBind(Math.max(0, Math.floor(preferredPort)), allowFallback);
    });

    const addr = srv.address();
    if (!addr || typeof addr === 'string') {
      await closeNetServer(srv);
      throw new Error('Failed to bind MultiplexedRelayServer TCP port');
    }

    return new MultiplexedRelayServer(srv, addr.port, options);
  }

  public registerRoom(roomToken: string): void {
    const token = roomToken.trim();
    if (!token) {
      throw new Error('Mux roomToken must not be empty');
    }
    if (this.rooms.has(token)) {
      return;
    }
    if (this.rooms.size >= this.maxRooms) {
      throw new RelayCapacityError(
        `Multiplexed relay room limit (${this.maxRooms}) reached`
      );
    }
    this.rooms.set(token, {
      waitingHosts: [],
      waitingClients: [],
      activeSockets: new Set<net.Socket>(),
    });
  }

  public hasRoom(roomToken: string): boolean {
    return this.rooms.has(roomToken.trim());
  }

  public get activeRoomCount(): number {
    return this.rooms.size;
  }

  public removeRoom(roomToken: string): void {
    const token = roomToken.trim();
    const bucket = this.rooms.get(token);
    if (!bucket) return;
    this.destroyBucket(bucket);
    this.rooms.delete(token);
  }

  public retainRooms(validTokens: Set<string>): void {
    for (const [token, bucket] of this.rooms.entries()) {
      if (!validTokens.has(token)) {
        this.destroyBucket(bucket);
        this.rooms.delete(token);
      }
    }
  }

  private destroyBucket(bucket: MuxRoomBucket): void {
    for (const h of bucket.waitingHosts) {
      h.socket.removeListener('timeout', h.onTimeout);
      if (!h.socket.destroyed) h.socket.destroy();
    }
    for (const c of bucket.waitingClients) {
      c.socket.removeListener('timeout', c.onTimeout);
      if (!c.socket.destroyed) c.socket.destroy();
    }
    for (const s of bucket.activeSockets) {
      if (!s.destroyed) s.destroy();
    }
    bucket.waitingHosts.length = 0;
    bucket.waitingClients.length = 0;
    bucket.activeSockets.clear();
  }

  private handleIncomingConnection(socket: net.Socket): void {
    if (this.closed) {
      socket.destroy();
      return;
    }

    socket.setNoDelay(true);
    socket.setKeepAlive(true, 30_000);
    socket.setTimeout(this.handshakeTimeoutMs);
    this.pendingSockets.add(socket);

    let preambleBuf: Buffer = Buffer.alloc(0);
    let promoted = false;

    const cleanupPending = (): void => {
      socket.removeListener('data', onPreambleData);
      socket.removeListener('timeout', onHandshakeTimeout);
      socket.removeListener('error', onPendingError);
      socket.removeListener('close', onPendingClose);
      this.pendingSockets.delete(socket);
    };

    const onHandshakeTimeout = (): void => {
      cleanupPending();
      if (!socket.destroyed) socket.destroy();
    };

    const onPendingError = (): void => {
      cleanupPending();
      if (!socket.destroyed) socket.destroy();
    };

    const onPendingClose = (): void => {
      if (!promoted) {
        cleanupPending();
      }
    };

    const onPreambleData = (chunk: Buffer): void => {
      preambleBuf =
        preambleBuf.length === 0 ? chunk : Buffer.concat([preambleBuf, chunk]);
      const nlIdx = preambleBuf.indexOf(0x0a);
      if (nlIdx === -1) {
        if (preambleBuf.length > MAX_MUX_PREAMBLE_BYTES) {
          cleanupPending();
          socket.destroy();
        }
        return;
      }

      promoted = true;
      cleanupPending();
      socket.pause();

      const line = preambleBuf.subarray(0, nlIdx).toString('utf-8').trim();
      const trailing = Buffer.from(preambleBuf.subarray(nlIdx + 1));
      const parts = line.split(/\s+/);
      if (parts.length !== 3 || parts[0].toUpperCase() !== 'MUX') {
        socket.destroy();
        return;
      }

      const roomToken = parts[1];
      const role = parts[2].toUpperCase();
      const bucket = this.rooms.get(roomToken);
      if (!bucket || this.closed || (role !== 'HOST' && role !== 'CLIENT')) {
        socket.destroy();
        return;
      }

      if (role === 'HOST') {
        this.enqueueWaitingHost(bucket, socket);
      } else {
        this.enqueueWaitingClient(bucket, socket, trailing);
      }
    };

    socket.on('data', onPreambleData);
    socket.once('timeout', onHandshakeTimeout);
    socket.on('error', onPendingError);
    socket.on('close', onPendingClose);
  }

  private enqueueWaitingHost(bucket: MuxRoomBucket, socket: net.Socket): void {
    this.pruneWaitingList(bucket.waitingHosts, bucket.activeSockets);
    while (bucket.waitingHosts.length >= this.maxWaitingPerSide) {
      const evicted = bucket.waitingHosts.shift();
      if (evicted) {
        evicted.socket.removeListener('timeout', evicted.onTimeout);
        bucket.activeSockets.delete(evicted.socket);
        evicted.socket.destroy();
      }
    }

    socket.setTimeout(this.idleTimeoutMs);
    bucket.activeSockets.add(socket);

    const onTimeout = (): void => {
      this.removeFromWaiting(bucket.waitingHosts, socket);
      bucket.activeSockets.delete(socket);
      socket.destroy();
    };

    socket.once('timeout', onTimeout);
    socket.on('error', () => {
      socket.removeListener('timeout', onTimeout);
      this.removeFromWaiting(bucket.waitingHosts, socket);
      bucket.activeSockets.delete(socket);
      if (!socket.destroyed) socket.destroy();
    });
    socket.on('close', () => {
      socket.removeListener('timeout', onTimeout);
      this.removeFromWaiting(bucket.waitingHosts, socket);
      bucket.activeSockets.delete(socket);
    });

    bucket.waitingHosts.push({ socket, onTimeout });
    this.tryPairRoom(bucket);
  }

  private enqueueWaitingClient(
    bucket: MuxRoomBucket,
    socket: net.Socket,
    trailing: Buffer
  ): void {
    this.pruneWaitingList(bucket.waitingClients, bucket.activeSockets);
    while (bucket.waitingClients.length >= this.maxWaitingPerSide) {
      const evicted = bucket.waitingClients.shift();
      if (evicted) {
        evicted.socket.removeListener('timeout', evicted.onTimeout);
        bucket.activeSockets.delete(evicted.socket);
        evicted.socket.destroy();
      }
    }

    socket.setTimeout(this.idleTimeoutMs);
    bucket.activeSockets.add(socket);

    const onTimeout = (): void => {
      this.removeFromWaiting(bucket.waitingClients, socket);
      bucket.activeSockets.delete(socket);
      socket.destroy();
    };

    socket.once('timeout', onTimeout);
    socket.on('error', () => {
      socket.removeListener('timeout', onTimeout);
      this.removeFromWaiting(bucket.waitingClients, socket);
      bucket.activeSockets.delete(socket);
      if (!socket.destroyed) socket.destroy();
    });
    socket.on('close', () => {
      socket.removeListener('timeout', onTimeout);
      this.removeFromWaiting(bucket.waitingClients, socket);
      bucket.activeSockets.delete(socket);
    });

    bucket.waitingClients.push({ socket, trailing, onTimeout });
    this.tryPairRoom(bucket);
  }

  private pruneWaitingList<
    T extends { socket: net.Socket; onTimeout: () => void },
  >(queue: T[], activeSockets: Set<net.Socket>): void {
    for (let i = queue.length - 1; i >= 0; i--) {
      const item = queue[i];
      if (item.socket.destroyed || !item.socket.writable) {
        item.socket.removeListener('timeout', item.onTimeout);
        activeSockets.delete(item.socket);
        if (!item.socket.destroyed) item.socket.destroy();
        queue.splice(i, 1);
      }
    }
  }

  private removeFromWaiting<
    T extends { socket: net.Socket; onTimeout: () => void },
  >(queue: T[], target: net.Socket): void {
    const idx = queue.findIndex((item) => item.socket === target);
    if (idx !== -1) {
      const [removed] = queue.splice(idx, 1);
      removed.socket.removeListener('timeout', removed.onTimeout);
    }
  }

  private tryPairRoom(bucket: MuxRoomBucket): void {
    this.pruneWaitingList(bucket.waitingHosts, bucket.activeSockets);
    this.pruneWaitingList(bucket.waitingClients, bucket.activeSockets);

    while (bucket.waitingClients.length > 0 && bucket.waitingHosts.length > 0) {
      const hostItem = bucket.waitingHosts.shift()!;
      const hostSocket = hostItem.socket;
      hostSocket.removeListener('timeout', hostItem.onTimeout);
      if (hostSocket.destroyed || !hostSocket.writable) {
        bucket.activeSockets.delete(hostSocket);
        continue;
      }

      const clientItem = bucket.waitingClients.shift()!;
      const clientSocket = clientItem.socket;
      clientSocket.removeListener('timeout', clientItem.onTimeout);
      if (clientSocket.destroyed || !clientSocket.writable) {
        bucket.activeSockets.delete(clientSocket);
        hostSocket.setTimeout(this.idleTimeoutMs);
        hostSocket.once('timeout', hostItem.onTimeout);
        bucket.waitingHosts.unshift(hostItem);
        continue;
      }

      hostSocket.setTimeout(0);
      clientSocket.setTimeout(0);

      const teardownBoth = (): void => {
        if (!clientSocket.destroyed) clientSocket.destroy();
        if (!hostSocket.destroyed) hostSocket.destroy();
        bucket.activeSockets.delete(clientSocket);
        bucket.activeSockets.delete(hostSocket);
      };

      clientSocket.once('close', teardownBoth);
      clientSocket.once('error', teardownBoth);
      hostSocket.once('close', teardownBoth);
      hostSocket.once('error', teardownBoth);

      const wakeupAndInitial =
        clientItem.trailing.length > 0
          ? Buffer.concat([Buffer.from('R'), clientItem.trailing])
          : Buffer.from('R');

      hostSocket.write(wakeupAndInitial, (err) => {
        if (err) {
          teardownBoth();
          return;
        }
        clientSocket.pipe(hostSocket);
        hostSocket.pipe(clientSocket);
        clientSocket.resume();
        hostSocket.resume();
      });
    }
  }

  public async close(): Promise<void> {
    if (this.closed) return;
    this.closed = true;
    for (const s of this.pendingSockets) {
      if (!s.destroyed) s.destroy();
    }
    this.pendingSockets.clear();
    for (const bucket of this.rooms.values()) {
      this.destroyBucket(bucket);
    }
    this.rooms.clear();
    await closeNetServer(this.server);
  }
}

export interface UdpRendezvousOptions {
  maxRooms?: number;
  maxClientsPerRoom?: number;
  sessionTtlMs?: number;
  allowEphemeralFallback?: boolean;
}

interface UdpPunchPeerRecord {
  peerId: string;
  rawAddress: string;
  normalizedIp: string;
  port: number;
  publicAddr: string;
  localAddr: string | null;
  updatedAtMs: number;
}

interface UdpPunchRoomBucket {
  host: UdpPunchPeerRecord | null;
  clients: Map<string, UdpPunchPeerRecord>;
  pairTokens: Map<string, string>;
  updatedAtMs: number;
}

const SAFE_ROOM_TOKEN_RE = /^[a-zA-Z0-9._:-]{1,128}$/;
const SAFE_SOCKET_ADDR_RE = /^[a-zA-Z0-9.:[\]-]{3,96}$/;

function normalizeUdpAddress(rawIp: string): string {
  const trimmed = rawIp.trim();
  if (trimmed.startsWith('::ffff:')) {
    return trimmed.slice('::ffff:'.length);
  }
  if (trimmed === '::1' || trimmed === '0.0.0.0') {
    return '127.0.0.1';
  }
  return trimmed || '127.0.0.1';
}

function formatSocketAddr(ip: string, port: number): string {
  return ip.includes(':') ? `[${ip}]:${port}` : `${ip}:${port}`;
}

function sanitizeOptionalSocketAddr(raw: unknown): string | null {
  if (typeof raw !== 'string') return null;
  const trimmed = raw.trim();
  if (!trimmed || !SAFE_SOCKET_ADDR_RE.test(trimmed)) return null;
  return trimmed;
}

/**
 * Lightweight UDP rendezvous and endpoint-reflection server (`DEFAULT_UDP_RENDEZVOUS_PORT = 30098`).
 * Reflects observed public `ip:port` (`whoami`) and pairs host/joiner UDP endpoints (`register` -> `punch_peer`)
 * to coordinate simultaneous bidirectional P2P UDP hole-punching before falling back to `#mux:` TCP relay.
 */
export class UdpRendezvousServer {
  public readonly port: number;
  public readonly sessionTtlMs: number;
  private readonly maxRooms: number;
  private readonly maxClientsPerRoom: number;
  private readonly socket: dgram.Socket;
  private readonly rooms = new Map<string, UdpPunchRoomBucket>();
  private closed = false;

  private constructor(
    socket: dgram.Socket,
    port: number,
    options: UdpRendezvousOptions = {}
  ) {
    this.socket = socket;
    this.port = port;
    this.sessionTtlMs = Math.max(
      2_000,
      options.sessionTtlMs ?? DEFAULT_UDP_SESSION_TTL_MS
    );
    this.maxRooms = Math.max(1, options.maxRooms ?? DEFAULT_MAX_MUX_ROOMS);
    this.maxClientsPerRoom = Math.max(
      1,
      options.maxClientsPerRoom ?? DEFAULT_MAX_WAITING_PER_SIDE
    );

    this.socket.on('message', (msg, rinfo) => {
      this.handleDatagram(msg, rinfo);
    });
    this.socket.on('error', () => {
      // Ignore transient ICMP/UDP socket errors so the rendezvous listener stays alive
    });
  }

  public static async create(
    preferredPort = 0,
    host = '0.0.0.0',
    options: UdpRendezvousOptions = {}
  ): Promise<UdpRendezvousServer> {
    const allowFallback = options.allowEphemeralFallback ?? true;

    const bindAttempt = (targetPort: number): Promise<dgram.Socket> =>
      new Promise((resolve, reject) => {
        const sock = dgram.createSocket('udp4');
        const onError = (err: NodeJS.ErrnoException): void => {
          sock.removeListener('listening', onListening);
          try {
            sock.close();
          } catch {
            // ignore
          }
          reject(err);
        };
        const onListening = (): void => {
          sock.removeListener('error', onError);
          resolve(sock);
        };
        sock.once('error', onError);
        sock.once('listening', onListening);
        sock.bind(targetPort, host);
      });

    let boundSocket: dgram.Socket;
    const initialPort = Math.max(0, Math.floor(preferredPort));
    try {
      boundSocket = await bindAttempt(initialPort);
    } catch (err) {
      const code = (err as NodeJS.ErrnoException)?.code;
      if (
        allowFallback &&
        initialPort !== 0 &&
        (code === 'EADDRINUSE' || code === 'EACCES')
      ) {
        boundSocket = await bindAttempt(0);
      } else {
        throw err;
      }
    }

    const addr = boundSocket.address();
    if (!addr || typeof addr === 'string') {
      boundSocket.close();
      throw new Error('Failed to bind UdpRendezvousServer socket');
    }

    return new UdpRendezvousServer(boundSocket, addr.port, options);
  }

  public get activeRoomCount(): number {
    this.pruneExpired(Date.now());
    return this.rooms.size;
  }

  public pruneExpired(nowMs = Date.now()): void {
    for (const [token, bucket] of this.rooms.entries()) {
      if (
        bucket.host &&
        nowMs - bucket.host.updatedAtMs > this.sessionTtlMs
      ) {
        bucket.host = null;
      }
      for (const [cid, client] of bucket.clients.entries()) {
        if (nowMs - client.updatedAtMs > this.sessionTtlMs) {
          bucket.clients.delete(cid);
          bucket.pairTokens.delete(cid);
        }
      }
      if (
        !bucket.host &&
        bucket.clients.size === 0 &&
        nowMs - bucket.updatedAtMs > this.sessionTtlMs
      ) {
        this.rooms.delete(token);
      }
    }
  }

  private sendJson(payload: unknown, port: number, address: string): void {
    if (this.closed) return;
    try {
      const buf = Buffer.from(JSON.stringify(payload), 'utf-8');
      this.socket.send(buf, port, address);
    } catch {
      // Ignore send errors on closed/unreachable UDP targets
    }
  }

  private handleDatagram(msg: Buffer, rinfo: dgram.RemoteInfo): void {
    if (
      this.closed ||
      msg.length === 0 ||
      msg.length > MAX_UDP_RENDEZVOUS_BYTES
    ) {
      return;
    }

    let parsed: Record<string, unknown>;
    try {
      const raw = JSON.parse(msg.toString('utf-8')) as unknown;
      if (!raw || typeof raw !== 'object' || Array.isArray(raw)) {
        return;
      }
      parsed = raw as Record<string, unknown>;
    } catch {
      return;
    }

    const nowMs = Date.now();
    const op = String(parsed.op ?? '')
      .trim()
      .toLowerCase();
    const normalizedIp = normalizeUdpAddress(rinfo.address);
    const observedAddr = formatSocketAddr(normalizedIp, rinfo.port);

    if (op === 'whoami' || op === 'reflect') {
      this.sendJson(
        {
          op: 'whoami_ack',
          ip: normalizedIp,
          port: rinfo.port,
          observed_addr: observedAddr,
          nonce:
            typeof parsed.nonce === 'number' || typeof parsed.nonce === 'string'
              ? parsed.nonce
              : null,
        },
        rinfo.port,
        rinfo.address
      );
      return;
    }

    if (op === 'register' || op === 'punch_register') {
      this.pruneExpired(nowMs);
      const roomToken = String(parsed.room_token ?? '').trim();
      const roleRaw = String(parsed.role ?? '')
        .trim()
        .toLowerCase();
      if (
        !SAFE_ROOM_TOKEN_RE.test(roomToken) ||
        (roleRaw !== 'host' && roleRaw !== 'client')
      ) {
        this.sendJson(
          {
            op: 'error',
            error: 'Invalid room_token or role (expected host or client)',
          },
          rinfo.port,
          rinfo.address
        );
        return;
      }

      const peerIdRaw = String(parsed.peer_id ?? '')
        .trim()
        .slice(0, 64);
      const peerId = peerIdRaw || `${roleRaw}:${observedAddr}`;
      const localAddr = sanitizeOptionalSocketAddr(parsed.local_addr);

      let bucket = this.rooms.get(roomToken);
      if (!bucket) {
        if (this.rooms.size >= this.maxRooms) {
          const oldestKey = this.rooms.keys().next().value;
          if (oldestKey !== undefined) {
            this.rooms.delete(oldestKey);
          }
        }
        bucket = {
          host: null,
          clients: new Map<string, UdpPunchPeerRecord>(),
          pairTokens: new Map<string, string>(),
          updatedAtMs: nowMs,
        };
        this.rooms.set(roomToken, bucket);
      }
      bucket.updatedAtMs = nowMs;

      const record: UdpPunchPeerRecord = {
        peerId,
        rawAddress: rinfo.address,
        normalizedIp,
        port: rinfo.port,
        publicAddr: observedAddr,
        localAddr,
        updatedAtMs: nowMs,
      };

      this.sendJson(
        {
          op: 'register_ack',
          room_token: roomToken,
          role: roleRaw,
          peer_id: peerId,
          observed_addr: observedAddr,
        },
        rinfo.port,
        rinfo.address
      );

      if (roleRaw === 'host') {
        bucket.host = record;
        for (const [clientKey, client] of bucket.clients.entries()) {
          this.notifyPunchPair(roomToken, bucket, record, clientKey, client);
        }
      } else {
        if (
          !bucket.clients.has(peerId) &&
          bucket.clients.size >= this.maxClientsPerRoom
        ) {
          const oldestClientKey = bucket.clients.keys().next().value;
          if (oldestClientKey !== undefined) {
            bucket.clients.delete(oldestClientKey);
            bucket.pairTokens.delete(oldestClientKey);
          }
        }
        bucket.clients.set(peerId, record);
        if (bucket.host) {
          this.notifyPunchPair(
            roomToken,
            bucket,
            bucket.host,
            peerId,
            record
          );
        }
      }
    }
  }

  private notifyPunchPair(
    roomToken: string,
    bucket: UdpPunchRoomBucket,
    host: UdpPunchPeerRecord,
    clientKey: string,
    client: UdpPunchPeerRecord
  ): void {
    let punchToken = bucket.pairTokens.get(clientKey);
    if (!punchToken) {
      punchToken = crypto.randomBytes(8).toString('hex');
      bucket.pairTokens.set(clientKey, punchToken);
    }

    this.sendJson(
      {
        op: 'punch_peer',
        room_token: roomToken,
        role: 'host',
        peer_id: client.peerId,
        peer_public_addr: client.publicAddr,
        peer_local_addr: client.localAddr,
        observed_addr: host.publicAddr,
        punch_token: punchToken,
      },
      host.port,
      host.rawAddress
    );

    this.sendJson(
      {
        op: 'punch_peer',
        room_token: roomToken,
        role: 'client',
        peer_id: host.peerId,
        peer_public_addr: host.publicAddr,
        peer_local_addr: host.localAddr,
        observed_addr: client.publicAddr,
        punch_token: punchToken,
      },
      client.port,
      client.rawAddress
    );
  }

  public close(): Promise<void> {
    if (this.closed) {
      return Promise.resolve();
    }
    this.closed = true;
    this.rooms.clear();
    return new Promise((resolve) => {
      try {
        this.socket.close(() => resolve());
      } catch {
        resolve();
      }
    });
  }
}
