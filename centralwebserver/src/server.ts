import * as crypto from 'node:crypto';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as path from 'node:path';
import { URL } from 'node:url';
import {
  MultiplexedRelayOptions,
  RelayCapacityError,
  RelayPortRange,
  RelayRoomOptions,
  UdpRendezvousOptions,
} from './relay';
import { DedicatedServerFleetManager } from './dedicated_fleet';
import { MatchmakingQueueManager } from './matchmaking';
import {
  CentralMultiplayerRegistry,
  isCgnatOrPrivateIp,
  LobbyAuthError,
  normalizePeerIp,
  RegistryValidationError,
} from './registry';
import {
  AutomatchCancelRequest,
  AutomatchEnqueueRequest,
  CreateMatchRequest,
  DedicatedServerAllocateRequest,
  DedicatedServerHeartbeatRequest,
  DedicatedServerRegisterRequest,
  LobbyAnnouncement,
  LobbyHeartbeatRequest,
  LobbyStatus,
  PlayerRegistrationRequest,
  SignedMatchReport,
  WorkshopPublishJsonRequest,
  WorkshopPublishMetadataInput,
} from './types';
import {
  WorkshopAuthError,
  WorkshopManager,
  WorkshopPayloadTooLargeError,
  WorkshopValidationError,
} from './workshop';
import {
  scanProjectOverview,
  scanWorkspaceReplays,
} from './workspace-scanner';

export const MAX_JSON_BODY_BYTES = 64 * 1024;

export class PayloadTooLargeError extends Error {
  constructor(message = 'Request body exceeds 64 KB limit') {
    super(message);
    this.name = 'PayloadTooLargeError';
  }
}

const MIME_TYPES: Record<string, string> = {
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.js': 'application/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.webp': 'image/webp',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.glb': 'model/gltf-binary',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.ico': 'image/x-icon',
};

const IMMUTABLE_ASSET_EXTS = new Set([
  '.glb',
  '.webp',
  '.png',
  '.jpg',
  '.jpeg',
  '.woff',
  '.woff2',
  '.ico',
]);

interface CorsPolicyResult {
  forbidden: boolean;
  headers: Record<string, string>;
}

function evaluateCorsPolicy(
  req: http.IncomingMessage,
  method: string,
  configuredPublicHost?: string
): CorsPolicyResult {
  const originHeader = req.headers.origin;
  const isMutating =
    method === 'POST' || method === 'PUT' || method === 'DELETE';
  const requestedMethod = (
    req.headers['access-control-request-method'] ?? ''
  )
    .toString()
    .toUpperCase();
  const isPreflightMutating =
    method === 'OPTIONS' &&
    (requestedMethod === 'POST' ||
      requestedMethod === 'PUT' ||
      requestedMethod === 'DELETE');

  if (!originHeader) {
    return {
      forbidden: false,
      headers: isMutating
        ? {}
        : {
            'Access-Control-Allow-Origin': '*',
            'Access-Control-Allow-Methods': 'GET, HEAD, OPTIONS',
            'Access-Control-Allow-Headers': 'Content-Type',
          },
    };
  }

  let originHost = '';
  try {
    const parsedOrigin = new URL(originHeader);
    originHost = parsedOrigin.host.toLowerCase();
  } catch {
    return { forbidden: true, headers: {} };
  }

  const reqHost = (req.headers.host ?? '').toLowerCase();
  const allowedHosts = new Set<string>();
  if (reqHost) allowedHosts.add(reqHost);
  if (configuredPublicHost) {
    allowedHosts.add(configuredPublicHost.toLowerCase());
  }
  if (process.env.PUBLIC_DOMAIN) {
    allowedHosts.add(process.env.PUBLIC_DOMAIN.toLowerCase());
  }
  if (process.env.PUBLIC_HOST) {
    allowedHosts.add(process.env.PUBLIC_HOST.toLowerCase());
  }

  const allowedExtraOrigins = new Set<string>(
    (process.env.CORS_ALLOWED_ORIGINS ?? '')
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)
  );

  const originHostnameOnly = originHost.split(':')[0];
  const isTrustedOrigin =
    allowedExtraOrigins.has(originHeader) ||
    allowedHosts.has(originHost) ||
    (originHostnameOnly.length > 0 && allowedHosts.has(originHostnameOnly));

  if ((isMutating || isPreflightMutating) && !isTrustedOrigin) {
    return { forbidden: true, headers: {} };
  }

  if (isTrustedOrigin) {
    return {
      forbidden: false,
      headers: {
        'Access-Control-Allow-Origin': originHeader,
        'Access-Control-Allow-Methods':
          'GET, HEAD, POST, PUT, DELETE, OPTIONS',
        'Access-Control-Allow-Headers':
          'Content-Type, Authorization, X-Host-Token, X-Server-Token, X-Fleet-Secret, X-Workshop-Token, X-Workshop-Metadata',
        Vary: 'Origin',
      },
    };
  }

  return {
    forbidden: false,
    headers: {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, HEAD, OPTIONS',
      'Access-Control-Allow-Headers': 'Content-Type',
    },
  };
}

function sendResponse(
  res: http.ServerResponse,
  statusCode: number,
  contentType: string,
  body: Buffer | string,
  extraHeaders: Record<string, string> = {},
  headOnly = false
): void {
  const buf = typeof body === 'string' ? Buffer.from(body, 'utf-8') : body;
  res.writeHead(statusCode, {
    'Content-Type': contentType,
    'Content-Length': String(buf.length),
    'X-Content-Type-Options': 'nosniff',
    Connection: 'close',
    ...extraHeaders,
  });
  if (headOnly || statusCode === 304 || statusCode === 204) {
    res.end();
  } else {
    res.end(buf);
  }
}

function sendJson(
  res: http.ServerResponse,
  statusCode: number,
  payload: unknown,
  extraHeaders: Record<string, string> = {}
): void {
  const json = JSON.stringify(payload);
  sendResponse(res, statusCode, 'application/json', json, extraHeaders);
}

function readJsonBody<T>(req: http.IncomingMessage): Promise<T> {
  return new Promise((resolve, reject) => {
    const contentLengthHeader = req.headers['content-length'];
    if (contentLengthHeader !== undefined) {
      const declaredBytes = Number(contentLengthHeader);
      if (Number.isFinite(declaredBytes) && declaredBytes > MAX_JSON_BODY_BYTES) {
        req.pause();
        reject(new PayloadTooLargeError());
        return;
      }
    }

    const chunks: Buffer[] = [];
    let totalBytes = 0;
    let settled = false;

    const cleanup = (): void => {
      req.removeListener('data', onData);
      req.removeListener('end', onEnd);
      req.removeListener('error', onError);
    };

    const onData = (chunk: Buffer): void => {
      if (settled) return;
      totalBytes += chunk.length;
      if (totalBytes > MAX_JSON_BODY_BYTES) {
        settled = true;
        chunks.length = 0;
        cleanup();
        req.pause();
        reject(new PayloadTooLargeError());
        return;
      }
      chunks.push(chunk);
    };

    const onEnd = (): void => {
      if (settled) return;
      settled = true;
      cleanup();
      try {
        const raw = Buffer.concat(chunks).toString('utf-8');
        if (!raw.trim()) {
          reject(new Error('Empty JSON body'));
          return;
        }
        resolve(JSON.parse(raw) as T);
      } catch (err) {
        reject(err);
      }
    };

    const onError = (err: Error): void => {
      if (settled) return;
      settled = true;
      cleanup();
      reject(err);
    };

    req.on('data', onData);
    req.on('end', onEnd);
    req.on('error', onError);
  });
}

function extractHostAuthToken(req: http.IncomingMessage): string | undefined {
  const xToken = req.headers['x-host-token'];
  if (typeof xToken === 'string' && xToken.trim().length > 0) {
    return xToken.trim();
  }
  const auth = req.headers.authorization;
  if (typeof auth === 'string') {
    const match = auth.match(/^Bearer\s+(.+)$/i);
    if (match && match[1].trim().length > 0) {
      return match[1].trim();
    }
  }
  return undefined;
}

function extractWorkshopAuthToken(
  req: http.IncomingMessage
): string | undefined {
  const xToken = req.headers['x-workshop-token'];
  if (typeof xToken === 'string' && xToken.trim().length > 0) {
    return xToken.trim();
  }
  const auth = req.headers.authorization;
  if (typeof auth === 'string') {
    const match = auth.match(/^Bearer\s+(.+)$/i);
    if (match && match[1].trim().length > 0) {
      return match[1].trim();
    }
  }
  return undefined;
}

function extractServerAuthToken(
  req: http.IncomingMessage
): string | undefined {
  const xToken = req.headers['x-server-token'];
  if (typeof xToken === 'string' && xToken.trim().length > 0) {
    return xToken.trim();
  }
  const auth = req.headers.authorization;
  if (typeof auth === 'string') {
    const match = auth.match(/^Bearer\s+(.+)$/i);
    if (match && match[1].trim().length > 0) {
      return match[1].trim();
    }
  }
  return undefined;
}

function extractFleetSecretHeader(
  req: http.IncomingMessage
): string | undefined {
  const secret = req.headers['x-fleet-secret'];
  if (typeof secret === 'string' && secret.trim().length > 0) {
    return secret.trim();
  }
  return undefined;
}

function readRawBodyBounded(
  req: http.IncomingMessage,
  maxBytes: number
): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    const contentLengthHeader = req.headers['content-length'];
    if (contentLengthHeader !== undefined) {
      const declaredBytes = Number(contentLengthHeader);
      if (Number.isFinite(declaredBytes) && declaredBytes > maxBytes) {
        req.pause();
        reject(
          new WorkshopPayloadTooLargeError(
            `Request body exceeds ${maxBytes} byte workshop limit`
          )
        );
        return;
      }
    }

    const chunks: Buffer[] = [];
    let totalBytes = 0;
    let settled = false;

    const cleanup = (): void => {
      req.removeListener('data', onData);
      req.removeListener('end', onEnd);
      req.removeListener('error', onError);
    };

    const onData = (chunk: Buffer): void => {
      if (settled) return;
      totalBytes += chunk.length;
      if (totalBytes > maxBytes) {
        settled = true;
        chunks.length = 0;
        cleanup();
        req.pause();
        reject(
          new WorkshopPayloadTooLargeError(
            `Request body exceeds ${maxBytes} byte workshop limit`
          )
        );
        return;
      }
      chunks.push(chunk);
    };

    const onEnd = (): void => {
      if (settled) return;
      settled = true;
      cleanup();
      resolve(Buffer.concat(chunks));
    };

    const onError = (err: Error): void => {
      if (settled) return;
      settled = true;
      cleanup();
      reject(err);
    };

    req.on('data', onData);
    req.on('end', onEnd);
    req.on('error', onError);
  });
}

export function isTrustedProxyAddress(
  remoteAddress?: string,
  trustProxyOption?: boolean
): boolean {
  if (trustProxyOption === true) return true;
  if (trustProxyOption === false) return false;
  if (process.env.TRUST_PROXY === 'true') return true;
  if (process.env.TRUST_PROXY === 'false') return false;
  if (!remoteAddress) return false;
  const addr = remoteAddress.trim();
  return (
    addr === '127.0.0.1' ||
    addr === '::1' ||
    addr === '::ffff:127.0.0.1'
  );
}

const SAFE_IP_RE = /^[0-9a-fA-F:.]{2,64}$/;
const SAFE_HOSTNAME_RE = /^[a-zA-Z0-9](?:[a-zA-Z0-9.-]{0,251}[a-zA-Z0-9])?$/;

function extractClientIp(
  req: http.IncomingMessage,
  trustProxyOption?: boolean
): string | undefined {
  if (isTrustedProxyAddress(req.socket.remoteAddress, trustProxyOption)) {
    const xff = req.headers['x-forwarded-for'];
    if (typeof xff === 'string' && xff.trim().length > 0) {
      const candidate = xff.split(',')[0].trim();
      if (SAFE_IP_RE.test(candidate)) {
        return candidate;
      }
    }
    const xRealIp = req.headers['x-real-ip'];
    if (typeof xRealIp === 'string' && xRealIp.trim().length > 0) {
      const candidate = xRealIp.trim();
      if (SAFE_IP_RE.test(candidate)) {
        return candidate;
      }
    }
  }
  return req.socket.remoteAddress;
}

function sanitizeHostname(raw: string): string | undefined {
  const trimmed = raw.trim();
  if (!trimmed || trimmed.includes('..')) return undefined;
  // Handle bracketed IPv6 if present, otherwise hostname/IPv4
  if (trimmed.startsWith('[')) {
    const closeIdx = trimmed.indexOf(']');
    if (closeIdx > 1) {
      const ipv6 = trimmed.slice(1, closeIdx);
      if (SAFE_IP_RE.test(ipv6)) return ipv6;
    }
    return undefined;
  }
  const hostOnly = trimmed.split(':')[0].trim();
  if (
    hostOnly &&
    hostOnly !== 'localhost' &&
    hostOnly !== '0.0.0.0' &&
    SAFE_HOSTNAME_RE.test(hostOnly)
  ) {
    return hostOnly;
  }
  return undefined;
}

function extractRelayPublicHost(
  req: http.IncomingMessage,
  configuredPublicHost?: string,
  trustProxyOption?: boolean
): string | undefined {
  if (configuredPublicHost && configuredPublicHost.trim().length > 0) {
    return configuredPublicHost.trim();
  }
  if (process.env.PUBLIC_HOST && process.env.PUBLIC_HOST.trim().length > 0) {
    return process.env.PUBLIC_HOST.trim();
  }
  if (isTrustedProxyAddress(req.socket.remoteAddress, trustProxyOption)) {
    const xfHost = req.headers['x-forwarded-host'];
    if (typeof xfHost === 'string' && xfHost.trim().length > 0) {
      const safeXf = sanitizeHostname(xfHost.split(',')[0]);
      if (safeXf) return safeXf;
    }
    const hostHeader = req.headers.host;
    if (typeof hostHeader === 'string' && hostHeader.trim().length > 0) {
      const safeHost = sanitizeHostname(hostHeader);
      if (safeHost) return safeHost;
    }
  }
  return req.socket.localAddress;
}

class SlidingWindowRateLimiter {
  private readonly buckets = new Map<
    string,
    { count: number; resetAtMs: number }
  >();
  private readonly maxRequests: number;
  private readonly windowMs: number;

  constructor(maxRequests: number, windowMs: number) {
    this.maxRequests = Math.max(1, maxRequests);
    this.windowMs = Math.max(100, windowMs);
  }

  public check(
    ipKey: string,
    nowMs: number
  ): { allowed: boolean; retryAfterSecs: number } {
    if (this.buckets.size > 4096) {
      for (const [key, entry] of this.buckets.entries()) {
        if (nowMs >= entry.resetAtMs) {
          this.buckets.delete(key);
        }
      }
    }

    const existing = this.buckets.get(ipKey);
    if (!existing || nowMs >= existing.resetAtMs) {
      this.buckets.set(ipKey, {
        count: 1,
        resetAtMs: nowMs + this.windowMs,
      });
      return { allowed: true, retryAfterSecs: 0 };
    }

    if (existing.count >= this.maxRequests) {
      const retryAfterSecs = Math.max(
        1,
        Math.ceil((existing.resetAtMs - nowMs) / 1000)
      );
      return { allowed: false, retryAfterSecs };
    }

    existing.count += 1;
    return { allowed: true, retryAfterSecs: 0 };
  }
}

export interface CentralWebServerOptions {
  ttlSecs?: number;
  publicDir?: string;
  workspaceRoot?: string;
  publicHost?: string;
  relayPortRange?: RelayPortRange;
  relayRoomOptions?: RelayRoomOptions;
  relayMuxPort?: number;
  muxRelayOptions?: MultiplexedRelayOptions;
  udpRendezvousPort?: number;
  udpRendezvousOptions?: UdpRendezvousOptions;
  preferMuxRelay?: boolean;
  dataDir?: string;
  workshopDir?: string;
  maxWorkshopUploadBytes?: number;
  workshopApiToken?: string;
  dedicatedTtlMs?: number;
  dedicatedFleetSecret?: string;
  seedDemoMatches?: boolean;
  trustProxy?: boolean;
  rateLimitMax?: number;
  rateLimitWindowMs?: number;
}

function resolveDefaultPublicDir(): string {
  const candidates = [
    path.resolve(__dirname, '..', 'public'),
    path.resolve(__dirname, '..', '..', 'public'),
    path.resolve(process.cwd(), 'public'),
    path.resolve(process.cwd(), 'centralwebserver', 'public'),
  ];
  for (const c of candidates) {
    if (fs.existsSync(path.join(c, 'index.html'))) {
      return c;
    }
  }
  return path.resolve(__dirname, '..', '..', 'public');
}

interface CachedGameDb {
  buffer: Buffer;
  etag: string;
  mtimeMs: number;
}

export function createCentralWebServer(options: CentralWebServerOptions = {}): {
  server: http.Server;
  registry: CentralMultiplayerRegistry;
  matchmaking: MatchmakingQueueManager;
  workshop: WorkshopManager;
  dedicatedFleet: DedicatedServerFleetManager;
} {
  const initialReplays = scanWorkspaceReplays(options.workspaceRoot);
  const registry = new CentralMultiplayerRegistry(
    options.ttlSecs ?? 45,
    initialReplays,
    Date.now(),
    options.relayPortRange,
    {
      seedDemoMatches: options.seedDemoMatches,
      relayRoomOptions: options.relayRoomOptions,
      relayMuxPort: options.relayMuxPort,
      muxRelayOptions: options.muxRelayOptions,
      udpRendezvousPort: options.udpRendezvousPort,
      udpRendezvousOptions: options.udpRendezvousOptions,
      preferMuxRelay: options.preferMuxRelay,
      dataDir: options.dataDir,
    }
  );
  const dedicatedFleet = new DedicatedServerFleetManager(registry, {
    ttlMs:
      options.dedicatedTtlMs ??
      (options.ttlSecs !== undefined
        ? Math.max(1, options.ttlSecs) * 1000
        : 45_000),
    fleetSecret: options.dedicatedFleetSecret,
  });
  const matchmaking = new MatchmakingQueueManager(registry, dedicatedFleet);
  const workshop = new WorkshopManager({
    workshopDir: options.workshopDir,
    dataDir: options.dataDir,
    maxUploadBytes: options.maxWorkshopUploadBytes,
    apiToken: options.workshopApiToken,
  });
  const projectOverview = scanProjectOverview(options.workspaceRoot);
  const resolvedPublicRoot = path.resolve(
    options.publicDir ?? resolveDefaultPublicDir()
  );
  const publicRootWithSep = resolvedPublicRoot.endsWith(path.sep)
    ? resolvedPublicRoot
    : resolvedPublicRoot + path.sep;

  const rateLimiter = new SlidingWindowRateLimiter(
    options.rateLimitMax ?? (Number(process.env.RATE_LIMIT_MAX) || 60),
    options.rateLimitWindowMs ??
      (Number(process.env.RATE_LIMIT_WINDOW_MS) || 60_000)
  );

  let cachedGameDb: CachedGameDb | null = null;
  const dbPath = path.join(resolvedPublicRoot, 'game-database.json');

  async function loadCachedGameDatabase(): Promise<CachedGameDb | null> {
    try {
      const stat = await fs.promises.stat(dbPath);
      if (!stat.isFile()) return null;
      if (cachedGameDb && cachedGameDb.mtimeMs === stat.mtimeMs) {
        return cachedGameDb;
      }
      const buffer = await fs.promises.readFile(dbPath);
      const digest = crypto
        .createHash('sha256')
        .update(buffer)
        .digest('hex')
        .slice(0, 24);
      cachedGameDb = {
        buffer,
        etag: `"db-${digest}"`,
        mtimeMs: stat.mtimeMs,
      };
      return cachedGameDb;
    } catch {
      return null;
    }
  }

  function handlePayloadTooLarge(
    req: http.IncomingMessage,
    res: http.ServerResponse,
    corsHeaders: Record<string, string>,
    message: string
  ): void {
    let drainedBytes = 0;
    const maxGraceDrainBytes = 256 * 1024;
    const onDrainChunk = (chunk: Buffer): void => {
      drainedBytes += chunk.length;
      if (drainedBytes > maxGraceDrainBytes && !req.destroyed) {
        req.removeListener('data', onDrainChunk);
        req.destroy();
      }
    };
    req.on('data', onDrainChunk);
    req.resume();

    const destroyTimer = setTimeout(() => {
      req.removeListener('data', onDrainChunk);
      if (!req.destroyed) req.destroy();
    }, 200);
    destroyTimer.unref();

    req.once('end', () => {
      clearTimeout(destroyTimer);
      req.removeListener('data', onDrainChunk);
    });
    req.once('close', () => {
      clearTimeout(destroyTimer);
      req.removeListener('data', onDrainChunk);
    });

    sendJson(res, 413, { error: message }, {
      ...corsHeaders,
      Connection: 'close',
    });
  }

  const server = http.createServer(async (req, res) => {
    const method = (req.method ?? 'GET').toUpperCase();
    const cors = evaluateCorsPolicy(req, method, options.publicHost);
    if (cors.forbidden) {
      sendJson(res, 403, { error: 'Cross-origin mutating request forbidden' });
      return;
    }

    try {
      if (method === 'OPTIONS') {
        sendResponse(res, 204, 'text/plain', '', cors.headers, true);
        return;
      }

      const rawUrl = req.url ?? '/';
      const parsedUrl = new URL(rawUrl, 'http://localhost');
      const pathname = parsedUrl.pathname;
      const searchParams = parsedUrl.searchParams;
      const nowMs = Date.now();

      const isMutatingApi =
        (method === 'POST' || method === 'PUT' || method === 'DELETE') &&
        (pathname === '/api/lobbies' ||
          pathname.startsWith('/api/lobbies/') ||
          pathname === '/api/matches' ||
          pathname === '/api/v1/players/register' ||
          pathname === '/api/v1/matches' ||
          pathname === '/api/v1/matches/signed' ||
          pathname.startsWith('/api/v1/servers/') ||
          pathname.startsWith('/api/v1/matchmaking/') ||
          pathname.startsWith('/api/v1/workshop/'));

      if (isMutatingApi) {
        const clientIpKey =
          extractClientIp(req, options.trustProxy) ?? 'unknown';
        const rl = rateLimiter.check(clientIpKey, nowMs);
        if (!rl.allowed) {
          sendJson(
            res,
            429,
            { error: 'Too many requests, please retry later' },
            {
              ...cors.headers,
              'Retry-After': String(rl.retryAfterSecs),
            }
          );
          return;
        }
      }

      // 1. Health Endpoint (compatible with rts-network)
      if (method === 'GET' && pathname === '/api/health') {
        const stats = registry.getStats(nowMs);
        sendJson(
          res,
          200,
          {
            status: 'online',
            active_lobbies: stats.active_lobbies,
            players_online: stats.players_online,
            uptime_secs: stats.uptime_secs,
          },
          cors.headers
        );
        return;
      }

      // 1b. STUN Whoami Endpoint (returns apparent WAN IP, TCP port, and CGNAT/private classification)
      if (
        method === 'GET' &&
        (pathname === '/api/v1/stun-whoami' || pathname === '/api/stun-whoami')
      ) {
        const rawIp = extractClientIp(req, options.trustProxy);
        const ip = normalizePeerIp(rawIp) ?? '127.0.0.1';
        let port = req.socket.remotePort ?? 0;
        if (isTrustedProxyAddress(req.socket.remoteAddress, options.trustProxy)) {
          const xfPort = req.headers['x-forwarded-port'];
          if (typeof xfPort === 'string' && xfPort.trim().length > 0) {
            const parsedPort = Number(xfPort.split(',')[0].trim());
            if (
              Number.isInteger(parsedPort) &&
              parsedPort > 0 &&
              parsedPort <= 65535
            ) {
              port = parsedPort;
            }
          }
        }
        sendJson(
          res,
          200,
          {
            ip,
            port,
            is_cgnat: isCgnatOrPrivateIp(ip),
          },
          cors.headers
        );
        return;
      }

      // 2. Live Registry Stats Endpoint
      if (method === 'GET' && pathname === '/api/stats') {
        sendJson(
          res,
          200,
          {
            ...registry.getStats(nowMs),
            workshop_items_total: workshop.totalItems,
          },
          cors.headers
        );
        return;
      }

      // 3. Project & Mod Metadata Endpoint
      if (method === 'GET' && pathname === '/api/project-info') {
        sendJson(res, 200, projectOverview, cors.headers);
        return;
      }

      // 3b. Leaderboard Endpoint (Glicko-2 / ELO MMR + commander stats)
      if (
        method === 'GET' &&
        (pathname === '/api/leaderboard' || pathname === '/api/v1/leaderboard')
      ) {
        sendJson(
          res,
          200,
          { leaderboard: registry.getLeaderboard() },
          cors.headers
        );
        return;
      }

      // 3c. Game Database Endpoint (in-memory cached + async I/O + ETag/Cache-Control)
      if ((method === 'GET' || method === 'HEAD') && pathname === '/api/game-database') {
        const db = await loadCachedGameDatabase();
        if (db) {
          const cacheHeaders = {
            ...cors.headers,
            'Cache-Control': 'public, max-age=300',
            ETag: db.etag,
          };
          if (req.headers['if-none-match'] === db.etag) {
            sendResponse(
              res,
              304,
              'application/json; charset=utf-8',
              '',
              cacheHeaders,
              true
            );
            return;
          }
          sendResponse(
            res,
            200,
            'application/json; charset=utf-8',
            db.buffer,
            cacheHeaders,
            method === 'HEAD'
          );
          return;
        }
        sendJson(res, 404, { error: 'Game database not found' }, cors.headers);
        return;
      }

      // 4. List Lobbies: GET /api/lobbies
      if (method === 'GET' && pathname === '/api/lobbies') {
        const modId = searchParams.get('mod_id') ?? undefined;
        const onlyOpenRaw = searchParams.get('only_open');
        const onlyOpen = onlyOpenRaw === 'true' || onlyOpenRaw === '1';
        const statusParam = searchParams.get('status') as LobbyStatus | null;
        const statusFilter =
          statusParam === 'open' ||
          statusParam === 'full' ||
          statusParam === 'in_game'
            ? statusParam
            : undefined;
        const search = searchParams.get('search') ?? undefined;

        const lobbies = registry.listLobbies(
          {
            modId: modId || undefined,
            onlyOpen,
            status: statusFilter,
            search,
          },
          nowMs
        );
        sendJson(res, 200, { lobbies }, cors.headers);
        return;
      }

      // 5. Register or Update Lobby: POST /api/lobbies
      if (method === 'POST' && pathname === '/api/lobbies') {
        try {
          const ann = await readJsonBody<LobbyAnnouncement>(req);
          if (
            !ann ||
            typeof ann !== 'object' ||
            Array.isArray(ann) ||
            !ann.lobby_name ||
            !ann.host_addr
          ) {
            sendJson(
              res,
              400,
              {
                error: 'Invalid JSON: lobby_name and host_addr are required',
              },
              cors.headers
            );
            return;
          }
          const peerIp = extractClientIp(req, options.trustProxy);
          const serverLocalIp = extractRelayPublicHost(
            req,
            options.publicHost,
            options.trustProxy
          );
          const tokenHeader = extractHostAuthToken(req);
          const entry = await registry.registerOrUpdate(
            ann,
            peerIp,
            serverLocalIp,
            nowMs,
            tokenHeader
          );
          sendJson(res, 201, entry, cors.headers);
        } catch (err) {
          if (err instanceof PayloadTooLargeError) {
            handlePayloadTooLarge(req, res, cors.headers, err.message);
            return;
          }
          if (err instanceof LobbyAuthError) {
            sendJson(
              res,
              err.statusCode,
              { error: err.message },
              cors.headers
            );
            return;
          }
          if (err instanceof RelayCapacityError) {
            sendJson(
              res,
              503,
              { error: err.message, code: err.code },
              cors.headers
            );
            return;
          }
          const msg = err instanceof Error ? err.message : String(err);
          sendJson(
            res,
            400,
            { error: `Invalid JSON: ${msg}` },
            cors.headers
          );
        }
        return;
      }

      // 6. Heartbeat, Password Verification, or Delete on /api/lobbies/:id[/heartbeat|/close|/verify-password]
      if (pathname.startsWith('/api/lobbies/')) {
        const rest = pathname.slice('/api/lobbies/'.length);
        const parts = rest.split('/');
        let lobbyId = '';
        try {
          lobbyId = decodeURIComponent(parts[0] ?? '');
        } catch {
          sendJson(res, 400, { error: 'Malformed lobby ID' }, cors.headers);
          return;
        }
        const sub = parts[1] ?? '';

        if (method === 'POST' && sub === 'verify-password') {
          try {
            const body = await readJsonBody<{ password?: string | null }>(req);
            const candidate =
              body?.password ??
              (typeof req.headers['x-lobby-password'] === 'string'
                ? req.headers['x-lobby-password']
                : undefined);
            const verified = registry.verifyLobbyPassword(
              lobbyId,
              candidate,
              nowMs
            );
            if (!verified.found) {
              sendJson(
                res,
                404,
                { error: 'Lobby not found or expired' },
                cors.headers
              );
              return;
            }
            if (!verified.valid) {
              sendJson(
                res,
                403,
                { valid: false, error: 'Invalid lobby password' },
                cors.headers
              );
              return;
            }
            sendJson(
              res,
              200,
              { valid: true, lobby: verified.lobby },
              cors.headers
            );
          } catch (err) {
            if (err instanceof PayloadTooLargeError) {
              handlePayloadTooLarge(req, res, cors.headers, err.message);
              return;
            }
            const msg = err instanceof Error ? err.message : String(err);
            sendJson(
              res,
              400,
              { error: `Invalid password verification payload: ${msg}` },
              cors.headers
            );
          }
          return;
        }

        if (
          (method === 'POST' || method === 'PUT') &&
          (sub === 'heartbeat' || sub === '')
        ) {
          try {
            const hb = await readJsonBody<LobbyHeartbeatRequest>(req);
            const tokenHeader = extractHostAuthToken(req);
            const updated = registry.heartbeat(
              lobbyId,
              hb,
              nowMs,
              tokenHeader
            );
            if (!updated) {
              sendJson(
                res,
                404,
                { error: 'Lobby not found or expired' },
                cors.headers
              );
              return;
            }
            sendJson(res, 200, updated, cors.headers);
          } catch (err) {
            if (err instanceof PayloadTooLargeError) {
              handlePayloadTooLarge(req, res, cors.headers, err.message);
              return;
            }
            if (err instanceof LobbyAuthError) {
              sendJson(
                res,
                err.statusCode,
                { error: err.message },
                cors.headers
              );
              return;
            }
            const msg = err instanceof Error ? err.message : String(err);
            sendJson(
              res,
              400,
              { error: `Invalid heartbeat JSON: ${msg}` },
              cors.headers
            );
          }
          return;
        }

        if (method === 'DELETE' || (method === 'POST' && sub === 'close')) {
          try {
            const tokenHeader = extractHostAuthToken(req);
            const deleted = registry.deleteLobby(lobbyId, nowMs, tokenHeader);
            sendJson(res, 200, { deleted }, cors.headers);
          } catch (err) {
            if (err instanceof LobbyAuthError) {
              sendJson(
                res,
                err.statusCode,
                { error: err.message },
                cors.headers
              );
              return;
            }
            const msg = err instanceof Error ? err.message : String(err);
            sendJson(res, 400, { error: msg }, cors.headers);
          }
          return;
        }
      }

      // 7. Match & Replay Registry Endpoints
      if (method === 'GET' && pathname === '/api/matches') {
        const modId = searchParams.get('mod_id') ?? undefined;
        const search = searchParams.get('search') ?? undefined;
        const matches = registry.listMatches(modId || undefined, search);
        sendJson(res, 200, { matches }, cors.headers);
        return;
      }

      if (method === 'POST' && pathname === '/api/matches') {
        try {
          const body = await readJsonBody<CreateMatchRequest>(req);
          if (!body || typeof body !== 'object' || Array.isArray(body)) {
            sendJson(
              res,
              400,
              { error: 'Invalid match payload' },
              cors.headers
            );
            return;
          }
          const created = registry.recordMatch(body, nowMs);
          sendJson(res, 201, created, cors.headers);
        } catch (err) {
          if (err instanceof PayloadTooLargeError) {
            handlePayloadTooLarge(req, res, cors.headers, err.message);
            return;
          }
          if (err instanceof LobbyAuthError) {
            sendJson(
              res,
              err.statusCode,
              { error: err.message },
              cors.headers
            );
            return;
          }
          if (err instanceof RegistryValidationError) {
            sendJson(res, 400, { error: err.message }, cors.headers);
            return;
          }
          const msg = err instanceof Error ? err.message : String(err);
          sendJson(
            res,
            400,
            { error: `Invalid match JSON: ${msg}` },
            cors.headers
          );
        }
        return;
      }

      // 7b. Cryptographic Player Profile Registration: POST /api/v1/players/register
      if (method === 'POST' && pathname === '/api/v1/players/register') {
        try {
          const body = await readJsonBody<PlayerRegistrationRequest>(req);
          if (!body || typeof body !== 'object' || Array.isArray(body)) {
            sendJson(
              res,
              400,
              { error: 'Invalid player registration payload' },
              cors.headers
            );
            return;
          }
          const bound = registry.registerPlayerBinding(
            body.player_id,
            body.display_name,
            body.public_token
          );
          sendJson(
            res,
            201,
            {
              registered: true,
              player_id: bound.player_id,
              display_name: bound.display_name,
            },
            cors.headers
          );
        } catch (err) {
          if (err instanceof PayloadTooLargeError) {
            handlePayloadTooLarge(req, res, cors.headers, err.message);
            return;
          }
          if (err instanceof LobbyAuthError) {
            sendJson(
              res,
              err.statusCode,
              { error: err.message },
              cors.headers
            );
            return;
          }
          if (err instanceof RegistryValidationError) {
            sendJson(res, 400, { error: err.message }, cors.headers);
            return;
          }
          const msg = err instanceof Error ? err.message : String(err);
          sendJson(
            res,
            400,
            { error: `Invalid player registration JSON: ${msg}` },
            cors.headers
          );
        }
        return;
      }

      // 7c. Authenticated HMAC-SHA256 Match Report: POST /api/v1/matches & POST /api/v1/matches/signed
      if (
        method === 'POST' &&
        (pathname === '/api/v1/matches' || pathname === '/api/v1/matches/signed')
      ) {
        try {
          const body = await readJsonBody<SignedMatchReport>(req);
          if (!body || typeof body !== 'object' || Array.isArray(body)) {
            sendJson(
              res,
              401,
              { error: 'SignedMatchReport JSON object required' },
              cors.headers
            );
            return;
          }
          const serverTokenHeader = extractServerAuthToken(req);
          const created = registry.recordSignedMatch(
            body,
            nowMs,
            serverTokenHeader
          );
          sendJson(res, 201, created, cors.headers);
        } catch (err) {
          if (err instanceof PayloadTooLargeError) {
            handlePayloadTooLarge(req, res, cors.headers, err.message);
            return;
          }
          if (err instanceof LobbyAuthError) {
            sendJson(
              res,
              err.statusCode,
              { error: err.message },
              cors.headers
            );
            return;
          }
          if (err instanceof RegistryValidationError) {
            sendJson(res, 400, { error: err.message }, cors.headers);
            return;
          }
          const msg = err instanceof Error ? err.message : String(err);
          sendJson(
            res,
            401,
            { error: `Invalid signed match report JSON: ${msg}` },
            cors.headers
          );
        }
        return;
      }

      // 7c-ii. Dedicated Server Fleet Orchestration Endpoints: /api/v1/servers[/register|/heartbeat|/allocate]
      if (method === 'GET' && pathname === '/api/v1/servers') {
        const modId =
          searchParams.get('mod_id') ?? searchParams.get('modId') ?? undefined;
        const status = searchParams.get('status') ?? undefined;
        const region = searchParams.get('region') ?? undefined;
        const listed = dedicatedFleet.listServers(
          {
            modId: modId || undefined,
            status: status || undefined,
            region: region || undefined,
          },
          nowMs
        );
        sendJson(res, 200, listed, cors.headers);
        return;
      }

      if (method === 'POST' && pathname === '/api/v1/servers/register') {
        try {
          const body = await readJsonBody<DedicatedServerRegisterRequest>(req);
          if (!body || typeof body !== 'object' || Array.isArray(body)) {
            sendJson(
              res,
              400,
              { error: 'Invalid dedicated server registration payload' },
              cors.headers
            );
            return;
          }
          const peerIp = extractClientIp(req, options.trustProxy);
          const serverLocalIp = extractRelayPublicHost(
            req,
            options.publicHost,
            options.trustProxy
          );
          const serverTokenHeader = extractServerAuthToken(req);
          const fleetSecretHeader = extractFleetSecretHeader(req);
          const registered = await dedicatedFleet.registerServer(
            body,
            serverLocalIp,
            nowMs,
            serverTokenHeader,
            peerIp,
            fleetSecretHeader
          );
          sendJson(res, 201, registered, cors.headers);
        } catch (err) {
          if (err instanceof PayloadTooLargeError) {
            handlePayloadTooLarge(req, res, cors.headers, err.message);
            return;
          }
          if (err instanceof LobbyAuthError) {
            sendJson(
              res,
              err.statusCode,
              { error: err.message },
              cors.headers
            );
            return;
          }
          if (err instanceof RegistryValidationError) {
            sendJson(res, 400, { error: err.message }, cors.headers);
            return;
          }
          if (err instanceof RelayCapacityError) {
            sendJson(
              res,
              503,
              { error: err.message, code: err.code },
              cors.headers
            );
            return;
          }
          const msg = err instanceof Error ? err.message : String(err);
          sendJson(
            res,
            400,
            { error: `Invalid dedicated server register JSON: ${msg}` },
            cors.headers
          );
        }
        return;
      }

      if (
        (method === 'POST' || method === 'PUT') &&
        pathname === '/api/v1/servers/heartbeat'
      ) {
        try {
          const body = await readJsonBody<DedicatedServerHeartbeatRequest>(req);
          if (!body || typeof body !== 'object' || Array.isArray(body)) {
            sendJson(
              res,
              400,
              { error: 'Invalid dedicated server heartbeat payload' },
              cors.headers
            );
            return;
          }
          const serverTokenHeader = extractServerAuthToken(req);
          const updated = dedicatedFleet.heartbeat(
            body,
            nowMs,
            serverTokenHeader
          );
          if (!updated) {
            sendJson(
              res,
              404,
              { error: 'Dedicated server not found or expired' },
              cors.headers
            );
            return;
          }
          sendJson(res, 200, updated, cors.headers);
        } catch (err) {
          if (err instanceof PayloadTooLargeError) {
            handlePayloadTooLarge(req, res, cors.headers, err.message);
            return;
          }
          if (err instanceof LobbyAuthError) {
            sendJson(
              res,
              err.statusCode,
              { error: err.message },
              cors.headers
            );
            return;
          }
          if (err instanceof RegistryValidationError) {
            sendJson(res, 400, { error: err.message }, cors.headers);
            return;
          }
          const msg = err instanceof Error ? err.message : String(err);
          sendJson(
            res,
            400,
            { error: `Invalid dedicated server heartbeat JSON: ${msg}` },
            cors.headers
          );
        }
        return;
      }

      if (method === 'POST' && pathname === '/api/v1/servers/allocate') {
        try {
          const body = await readJsonBody<DedicatedServerAllocateRequest>(req);
          if (!body || typeof body !== 'object' || Array.isArray(body)) {
            sendJson(
              res,
              400,
              { error: 'Invalid dedicated server allocation payload' },
              cors.headers
            );
            return;
          }
          const allocated = dedicatedFleet.allocateServer(body, nowMs);
          if (!allocated) {
            sendJson(
              res,
              404,
              {
                allocated: false,
                error:
                  'No idle dedicated server available matching requested criteria',
              },
              cors.headers
            );
            return;
          }
          sendJson(
            res,
            200,
            {
              allocated: true,
              server: {
                ...allocated,
                relay_uplink_addr: null,
              },
            },
            cors.headers
          );
        } catch (err) {
          if (err instanceof PayloadTooLargeError) {
            handlePayloadTooLarge(req, res, cors.headers, err.message);
            return;
          }
          if (err instanceof RegistryValidationError) {
            sendJson(res, 400, { error: err.message }, cors.headers);
            return;
          }
          const msg = err instanceof Error ? err.message : String(err);
          sendJson(
            res,
            400,
            { error: `Invalid dedicated server allocation JSON: ${msg}` },
            cors.headers
          );
        }
        return;
      }

      // 7d. 1v1 / 2v2 ELO Automatch Queue Endpoints: /api/v1/matchmaking/{enqueue,poll,cancel}
      if (method === 'POST' && pathname === '/api/v1/matchmaking/enqueue') {
        try {
          const body = await readJsonBody<AutomatchEnqueueRequest>(req);
          if (!body || typeof body !== 'object' || Array.isArray(body)) {
            sendJson(
              res,
              400,
              { error: 'Invalid automatch enqueue payload' },
              cors.headers
            );
            return;
          }
          const serverLocalIp = extractRelayPublicHost(
            req,
            options.publicHost,
            options.trustProxy
          );
          const ticketResp = await matchmaking.enqueue(
            body,
            serverLocalIp,
            nowMs
          );
          sendJson(res, 201, ticketResp, cors.headers);
        } catch (err) {
          if (err instanceof PayloadTooLargeError) {
            handlePayloadTooLarge(req, res, cors.headers, err.message);
            return;
          }
          if (err instanceof LobbyAuthError) {
            sendJson(
              res,
              err.statusCode,
              { error: err.message },
              cors.headers
            );
            return;
          }
          if (err instanceof RegistryValidationError) {
            sendJson(res, 400, { error: err.message }, cors.headers);
            return;
          }
          if (err instanceof RelayCapacityError) {
            sendJson(
              res,
              503,
              { error: err.message, code: err.code },
              cors.headers
            );
            return;
          }
          const msg = err instanceof Error ? err.message : String(err);
          sendJson(
            res,
            400,
            { error: `Invalid automatch enqueue JSON: ${msg}` },
            cors.headers
          );
        }
        return;
      }

      if (
        (method === 'GET' || method === 'POST') &&
        pathname === '/api/v1/matchmaking/poll'
      ) {
        try {
          let ticketId = searchParams.get('ticket_id')?.trim() ?? '';
          if (!ticketId && method === 'POST') {
            const body = await readJsonBody<{ ticket_id?: string }>(req);
            ticketId = String(body?.ticket_id ?? '').trim();
          }
          if (!ticketId) {
            sendJson(
              res,
              400,
              { error: 'Missing ticket_id parameter' },
              cors.headers
            );
            return;
          }
          const serverLocalIp = extractRelayPublicHost(
            req,
            options.publicHost,
            options.trustProxy
          );
          const polled = await matchmaking.poll(
            ticketId,
            serverLocalIp,
            nowMs
          );
          if (!polled) {
            sendJson(
              res,
              404,
              { error: 'Automatch ticket not found' },
              cors.headers
            );
            return;
          }
          sendJson(res, 200, polled, cors.headers);
        } catch (err) {
          if (err instanceof PayloadTooLargeError) {
            handlePayloadTooLarge(req, res, cors.headers, err.message);
            return;
          }
          const msg = err instanceof Error ? err.message : String(err);
          sendJson(
            res,
            400,
            { error: `Invalid automatch poll request: ${msg}` },
            cors.headers
          );
        }
        return;
      }

      if (method === 'POST' && pathname === '/api/v1/matchmaking/cancel') {
        try {
          const body = await readJsonBody<AutomatchCancelRequest>(req);
          if (
            !body ||
            typeof body !== 'object' ||
            Array.isArray(body) ||
            !body.ticket_id ||
            !body.player_id
          ) {
            sendJson(
              res,
              400,
              { error: 'ticket_id and player_id are required' },
              cors.headers
            );
            return;
          }
          const cancelled = matchmaking.cancel(
            body.ticket_id,
            body.player_id,
            nowMs
          );
          sendJson(res, 200, { cancelled }, cors.headers);
        } catch (err) {
          if (err instanceof PayloadTooLargeError) {
            handlePayloadTooLarge(req, res, cors.headers, err.message);
            return;
          }
          const msg = err instanceof Error ? err.message : String(err);
          sendJson(
            res,
            400,
            { error: `Invalid automatch cancel JSON: ${msg}` },
            cors.headers
          );
        }
        return;
      }

      // 7e. Online Mod & Map Workshop Endpoints: /api/v1/workshop/{items,publish,download/:id}
      if (method === 'GET' && pathname === '/api/v1/workshop/items') {
        const result = workshop.listItems({
          search:
            searchParams.get('search') ?? searchParams.get('q') ?? undefined,
          tag: searchParams.get('tag') ?? undefined,
          kind: searchParams.get('kind') ?? undefined,
          sort: searchParams.get('sort') ?? undefined,
        });
        sendJson(res, 200, result, cors.headers);
        return;
      }

      if (method === 'POST' && pathname === '/api/v1/workshop/publish') {
        try {
          const contentType = (req.headers['content-type'] ?? '')
            .toString()
            .toLowerCase();
          const workshopToken = extractWorkshopAuthToken(req);

          let meta: WorkshopPublishMetadataInput;
          let blob: Buffer;

          if (contentType.includes('application/octet-stream')) {
            const rawMetaHeader = req.headers['x-workshop-metadata'];
            const metaHeaderStr = Array.isArray(rawMetaHeader)
              ? rawMetaHeader[0]
              : rawMetaHeader;
            if (!metaHeaderStr || !metaHeaderStr.trim()) {
              sendJson(
                res,
                400,
                {
                  error:
                    'X-Workshop-Metadata header required for octet-stream publish',
                },
                cors.headers
              );
              return;
            }
            const trimmedMeta = metaHeaderStr.trim();
            const decodedMetaJson = trimmedMeta.startsWith('{')
              ? trimmedMeta
              : Buffer.from(trimmedMeta, 'base64').toString('utf-8');
            meta = JSON.parse(decodedMetaJson) as WorkshopPublishMetadataInput;
            blob = await readRawBodyBounded(req, workshop.maxUploadBytes);
          } else {
            const maxJsonBytes =
              Math.ceil((workshop.maxUploadBytes * 4) / 3) + 64 * 1024;
            const rawBody = await readRawBodyBounded(req, maxJsonBytes);
            const rawText = rawBody.toString('utf-8');
            if (!rawText.trim()) {
              sendJson(
                res,
                400,
                { error: 'Empty workshop publish JSON payload' },
                cors.headers
              );
              return;
            }
            const parsed = JSON.parse(rawText) as WorkshopPublishJsonRequest;
            if (
              !parsed ||
              typeof parsed !== 'object' ||
              Array.isArray(parsed) ||
              typeof parsed.payload_base64 !== 'string'
            ) {
              sendJson(
                res,
                400,
                {
                  error:
                    'Workshop JSON publish payload requires payload_base64 string',
                },
                cors.headers
              );
              return;
            }
            const cleanB64 = parsed.payload_base64.trim();
            if (
              cleanB64.length === 0 ||
              !/^[A-Za-z0-9+/=_-]+$/.test(cleanB64)
            ) {
              sendJson(
                res,
                400,
                { error: 'Invalid base64 workshop payload_base64 encoding' },
                cors.headers
              );
              return;
            }
            blob = Buffer.from(cleanB64, 'base64');
            meta = parsed;
          }

          const created = workshop.publishItem(
            meta,
            blob,
            nowMs,
            workshopToken
          );
          sendJson(res, 201, created, cors.headers);
        } catch (err) {
          if (err instanceof WorkshopPayloadTooLargeError) {
            handlePayloadTooLarge(req, res, cors.headers, err.message);
            return;
          }
          if (err instanceof WorkshopAuthError) {
            sendJson(
              res,
              err.statusCode,
              { error: err.message },
              cors.headers
            );
            return;
          }
          if (err instanceof WorkshopValidationError) {
            sendJson(res, 400, { error: err.message }, cors.headers);
            return;
          }
          const msg = err instanceof Error ? err.message : String(err);
          sendJson(
            res,
            400,
            { error: `Invalid workshop publish payload: ${msg}` },
            cors.headers
          );
        }
        return;
      }

      if (
        method === 'GET' &&
        pathname.startsWith('/api/v1/workshop/download/')
      ) {
        const rawId = pathname.slice('/api/v1/workshop/download/'.length);
        let itemId = '';
        try {
          itemId = decodeURIComponent(rawId);
        } catch {
          sendJson(res, 400, { error: 'Malformed workshop item id' }, cors.headers);
          return;
        }
        if (itemId.includes('/') || itemId.includes('\\') || itemId.includes('..')) {
          sendJson(res, 400, { error: 'Invalid workshop item id' }, cors.headers);
          return;
        }
        const dl = workshop.openDownload(itemId);
        if (!dl) {
          sendJson(
            res,
            404,
            { error: 'Workshop item not found' },
            cors.headers
          );
          return;
        }

        const downloadHeaders: Record<string, string> = {
          ...cors.headers,
          'Content-Disposition': `attachment; filename="${dl.metadata.file_name}"`,
          'X-Workshop-Id': dl.metadata.id,
          'X-Workshop-Sha256': dl.metadata.sha256,
          'X-Workshop-Content-Hash': dl.metadata.mod_content_hash,
          'X-Workshop-Downloads': String(dl.metadata.downloads),
        };

        if (dl.stream) {
          res.writeHead(200, {
            'Content-Type': 'application/octet-stream',
            'Content-Length': String(dl.sizeBytes),
            'X-Content-Type-Options': 'nosniff',
            Connection: 'close',
            ...downloadHeaders,
          });
          dl.stream.on('error', () => {
            if (!res.destroyed) res.destroy();
          });
          dl.stream.pipe(res);
          return;
        }

        sendResponse(
          res,
          200,
          'application/octet-stream',
          dl.buffer ?? Buffer.alloc(0),
          downloadHeaders
        );
        return;
      }

      // 8. Static Files (Homepage & Frontend Assets — path.sep traversal protection + async I/O + ETag/Cache-Control)
      if (method === 'GET' || method === 'HEAD') {
        let decodedPath = pathname;
        try {
          decodedPath = decodeURIComponent(pathname);
        } catch {
          sendJson(res, 400, { error: 'Malformed URL path' }, cors.headers);
          return;
        }
        if (decodedPath.includes('\0')) {
          sendJson(res, 400, { error: 'Invalid URL path' }, cors.headers);
          return;
        }

        const relPath =
          decodedPath === '/' || decodedPath === '/index.html'
            ? 'index.html'
            : decodedPath.replace(/^\/+/, '');
        const resolvedPath = path.resolve(resolvedPublicRoot, relPath);
        if (resolvedPath.startsWith(publicRootWithSep)) {
          try {
            const stat = await fs.promises.stat(resolvedPath);
            if (stat.isFile()) {
              const ext = path.extname(resolvedPath).toLowerCase();
              const contentType =
                MIME_TYPES[ext] ?? 'application/octet-stream';
              const etag = `"${stat.size.toString(16)}-${Math.floor(
                stat.mtimeMs
              ).toString(16)}"`;
              const cacheControl = IMMUTABLE_ASSET_EXTS.has(ext)
                ? 'public, max-age=3600'
                : 'public, max-age=60';
              const staticHeaders = {
                ...cors.headers,
                'Cache-Control': cacheControl,
                ETag: etag,
              };
              if (req.headers['if-none-match'] === etag) {
                sendResponse(res, 304, contentType, '', staticHeaders, true);
                return;
              }
              if (method === 'HEAD') {
                res.writeHead(200, {
                  'Content-Type': contentType,
                  'Content-Length': String(stat.size),
                  'X-Content-Type-Options': 'nosniff',
                  Connection: 'close',
                  ...staticHeaders,
                });
                res.end();
                return;
              }
              const content = await fs.promises.readFile(resolvedPath);
              sendResponse(res, 200, contentType, content, staticHeaders);
              return;
            }
          } catch {
            // File does not exist; fall through to 404
          }
        }
      }

      sendJson(res, 404, { error: 'Endpoint not found' }, cors.headers);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      sendJson(
        res,
        500,
        { error: `Internal server error: ${msg}` },
        cors.headers
      );
    }
  });

  server.on('close', () => {
    registry.closeAllRelays();
  });

  return { server, registry, matchmaking, workshop, dedicatedFleet };
}

export const createAppServer = createCentralWebServer;
