import * as fs from 'node:fs';
import * as http from 'node:http';
import * as path from 'node:path';
import { URL } from 'node:url';
import { RelayPortRange } from './relay';
import { CentralMultiplayerRegistry } from './registry';
import {
  CreateMatchRequest,
  LobbyAnnouncement,
  LobbyHeartbeatRequest,
  LobbyStatus,
} from './types';
import {
  scanProjectOverview,
  scanWorkspaceReplays,
} from './workspace-scanner';

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

function sendResponse(
  res: http.ServerResponse,
  statusCode: number,
  contentType: string,
  body: Buffer | string
): void {
  const buf = typeof body === 'string' ? Buffer.from(body, 'utf-8') : body;
  res.writeHead(statusCode, {
    'Content-Type': contentType,
    'Content-Length': String(buf.length),
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type',
    Connection: 'close',
  });
  res.end(buf);
}

function sendJson(
  res: http.ServerResponse,
  statusCode: number,
  payload: unknown
): void {
  const json = JSON.stringify(payload);
  sendResponse(res, statusCode, 'application/json', json);
}

function readJsonBody<T>(req: http.IncomingMessage): Promise<T> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    let totalBytes = 0;
    req.on('data', (chunk: Buffer) => {
      totalBytes += chunk.length;
      if (totalBytes > 1024 * 1024) {
        reject(new Error('Request body too large'));
        req.destroy();
        return;
      }
      chunks.push(chunk);
    });
    req.on('end', () => {
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
    });
    req.on('error', reject);
  });
}

function extractClientIp(req: http.IncomingMessage): string | undefined {
  const xff = req.headers['x-forwarded-for'];
  if (typeof xff === 'string' && xff.trim().length > 0) {
    return xff.split(',')[0].trim();
  }
  const xRealIp = req.headers['x-real-ip'];
  if (typeof xRealIp === 'string' && xRealIp.trim().length > 0) {
    return xRealIp.trim();
  }
  return req.socket.remoteAddress;
}

function extractRelayPublicHost(
  req: http.IncomingMessage,
  configuredPublicHost?: string
): string | undefined {
  if (configuredPublicHost && configuredPublicHost.trim().length > 0) {
    return configuredPublicHost.trim();
  }
  if (process.env.PUBLIC_HOST && process.env.PUBLIC_HOST.trim().length > 0) {
    return process.env.PUBLIC_HOST.trim();
  }
  const xfHost = req.headers['x-forwarded-host'];
  if (typeof xfHost === 'string' && xfHost.trim().length > 0) {
    const firstHost = xfHost.split(',')[0].split(':')[0].trim();
    if (firstHost && firstHost !== 'localhost' && firstHost !== '0.0.0.0') {
      return firstHost;
    }
  }
  const hostHeader = req.headers.host;
  if (typeof hostHeader === 'string' && hostHeader.trim().length > 0) {
    const hostname = hostHeader.split(':')[0].trim();
    if (hostname && hostname !== 'localhost' && hostname !== '0.0.0.0') {
      return hostname;
    }
  }
  return req.socket.localAddress;
}

export interface CentralWebServerOptions {
  ttlSecs?: number;
  publicDir?: string;
  workspaceRoot?: string;
  publicHost?: string;
  relayPortRange?: RelayPortRange;
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

export function createCentralWebServer(options: CentralWebServerOptions = {}): {
  server: http.Server;
  registry: CentralMultiplayerRegistry;
} {
  const initialReplays = scanWorkspaceReplays(options.workspaceRoot);
  const registry = new CentralMultiplayerRegistry(
    options.ttlSecs ?? 45,
    initialReplays,
    Date.now(),
    options.relayPortRange
  );
  const projectOverview = scanProjectOverview(options.workspaceRoot);
  const publicDir = options.publicDir ?? resolveDefaultPublicDir();

  const server = http.createServer(async (req, res) => {
    try {
      const method = (req.method ?? 'GET').toUpperCase();
      if (method === 'OPTIONS') {
        sendResponse(res, 204, 'text/plain', '');
        return;
      }

      const rawUrl = req.url ?? '/';
      const parsedUrl = new URL(rawUrl, 'http://localhost');
      const pathname = parsedUrl.pathname;
      const searchParams = parsedUrl.searchParams;
      const nowMs = Date.now();

      // 1. Health Endpoint (compatible with rts-network)
      if (method === 'GET' && pathname === '/api/health') {
        const stats = registry.getStats(nowMs);
        sendJson(res, 200, {
          status: 'online',
          active_lobbies: stats.active_lobbies,
          players_online: stats.players_online,
          uptime_secs: stats.uptime_secs,
        });
        return;
      }

      // 2. Live Registry Stats Endpoint
      if (method === 'GET' && pathname === '/api/stats') {
        sendJson(res, 200, registry.getStats(nowMs));
        return;
      }

      // 3. Project & Mod Metadata Endpoint
      if (method === 'GET' && pathname === '/api/project-info') {
        sendJson(res, 200, projectOverview);
        return;
      }

      // 3b. Leaderboard Endpoint
      if (method === 'GET' && pathname === '/api/leaderboard') {
        sendJson(res, 200, { leaderboard: registry.getLeaderboard() });
        return;
      }

      // 3c. Game Database Endpoint (Units, Buildings, Weapons, Countries, Maps)
      if (method === 'GET' && pathname === '/api/game-database') {
        const dbPath = path.join(publicDir, 'game-database.json');
        if (fs.existsSync(dbPath)) {
          const content = fs.readFileSync(dbPath);
          sendResponse(res, 200, 'application/json; charset=utf-8', content);
          return;
        }
        sendJson(res, 404, { error: 'Game database not found' });
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
        sendJson(res, 200, { lobbies });
        return;
      }

      // 5. Register or Update Lobby: POST /api/lobbies
      if (method === 'POST' && pathname === '/api/lobbies') {
        try {
          const ann = await readJsonBody<LobbyAnnouncement>(req);
          if (!ann || typeof ann !== 'object' || !ann.lobby_name || !ann.host_addr) {
            sendJson(res, 400, {
              error: 'Invalid JSON: lobby_name and host_addr are required',
            });
            return;
          }
          const peerIp = extractClientIp(req);
          const serverLocalIp = extractRelayPublicHost(req, options.publicHost);
          const entry = await registry.registerOrUpdate(
            ann,
            peerIp,
            serverLocalIp,
            nowMs
          );
          sendJson(res, 201, entry);
        } catch (err) {
          const msg = err instanceof Error ? err.message : String(err);
          sendJson(res, 400, { error: `Invalid JSON: ${msg}` });
        }
        return;
      }

      // 6. Heartbeat or Delete on /api/lobbies/:id[/heartbeat|/close]
      if (pathname.startsWith('/api/lobbies/')) {
        const rest = pathname.slice('/api/lobbies/'.length);
        const parts = rest.split('/');
        const lobbyId = decodeURIComponent(parts[0] ?? '');
        const sub = parts[1] ?? '';

        if (
          (method === 'POST' || method === 'PUT') &&
          (sub === 'heartbeat' || sub === '')
        ) {
          try {
            const hb = await readJsonBody<LobbyHeartbeatRequest>(req);
            const updated = registry.heartbeat(lobbyId, hb, nowMs);
            if (!updated) {
              sendJson(res, 404, { error: 'Lobby not found or expired' });
              return;
            }
            sendJson(res, 200, updated);
          } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            sendJson(res, 400, { error: `Invalid heartbeat JSON: ${msg}` });
          }
          return;
        }

        if (method === 'DELETE' || (method === 'POST' && sub === 'close')) {
          const deleted = registry.deleteLobby(lobbyId, nowMs);
          sendJson(res, 200, { deleted });
          return;
        }
      }

      // 7. Match & Replay Registry Endpoints
      if (method === 'GET' && pathname === '/api/matches') {
        const modId = searchParams.get('mod_id') ?? undefined;
        const search = searchParams.get('search') ?? undefined;
        const matches = registry.listMatches(modId || undefined, search);
        sendJson(res, 200, { matches });
        return;
      }

      if (method === 'POST' && pathname === '/api/matches') {
        try {
          const body = await readJsonBody<CreateMatchRequest>(req);
          if (!body || typeof body !== 'object' || !body.lobby_name) {
            sendJson(res, 400, { error: 'Invalid match payload' });
            return;
          }
          const created = registry.recordMatch(body, nowMs);
          sendJson(res, 201, created);
        } catch (err) {
          const msg = err instanceof Error ? err.message : String(err);
          sendJson(res, 400, { error: `Invalid match JSON: ${msg}` });
        }
        return;
      }

      // 8. Static Files (Homepage & Frontend Assets)
      if (method === 'GET' || method === 'HEAD') {
        const relPath =
          pathname === '/' || pathname === '/index.html'
            ? 'index.html'
            : pathname.replace(/^\/+/, '');
        const resolvedPath = path.resolve(publicDir, relPath);
        if (
          resolvedPath.startsWith(publicDir) &&
          fs.existsSync(resolvedPath) &&
          fs.statSync(resolvedPath).isFile()
        ) {
          const ext = path.extname(resolvedPath).toLowerCase();
          const contentType =
            MIME_TYPES[ext] ?? 'application/octet-stream';
          const content = fs.readFileSync(resolvedPath);
          sendResponse(res, 200, contentType, content);
          return;
        }
      }

      sendJson(res, 404, { error: 'Endpoint not found' });
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      sendJson(res, 500, { error: `Internal server error: ${msg}` });
    }
  });

  server.on('close', () => {
    registry.closeAllRelays();
  });

  return { server, registry };
}
