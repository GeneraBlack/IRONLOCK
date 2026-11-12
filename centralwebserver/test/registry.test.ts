import * as assert from 'node:assert/strict';
import * as crypto from 'node:crypto';
import * as dgram from 'node:dgram';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as net from 'node:net';
import * as path from 'node:path';
import * as zlib from 'node:zlib';
import { pathToFileURL } from 'node:url';
import { after, before, describe, it } from 'node:test';
import {
  DEFAULT_RELAY_IDLE_TIMEOUT_MS,
  maxRelayRoomsForRange,
  UdpRendezvousServer,
} from '../src/relay';
import {
  CentralMultiplayerRegistry,
  createPlayerIdentityProfile,
  signMatchReport,
} from '../src/registry';
import { createAppServer, createCentralWebServer } from '../src/server';
import {
  AutomatchPollResponse,
  DedicatedServerEntry,
  DedicatedServerListResponse,
  DedicatedServerRegisterResponse,
  LeaderboardResponse,
  LobbyEntry,
  LobbyListResponse,
  MatchListResponse,
  MatchRecord,
  ProjectOverview,
  RegisteredLobbyResponse,
  RegistryStats,
  StunWhoamiResponse,
  WorkshopItemListResponse,
  WorkshopItemMetadata,
} from '../src/types';
import { signWorkshopPublish, WorkshopManager } from '../src/workshop';

function httpRequestRaw(
  baseUrl: string,
  method: string,
  reqPath: string,
  rawBody?: string | Buffer,
  extraHeaders?: Record<string, string>
): Promise<{
  statusCode: number;
  headers: http.IncomingHttpHeaders;
  bodyText: string;
}> {
  return new Promise((resolve, reject) => {
    const url = new URL(reqPath, baseUrl);
    const payloadBuf =
      rawBody !== undefined
        ? Buffer.isBuffer(rawBody)
          ? rawBody
          : Buffer.from(rawBody, 'utf-8')
        : undefined;

    const req = http.request(
      url,
      {
        method,
        headers: {
          ...(payloadBuf
            ? {
                'Content-Type': 'application/json',
                'Content-Length': String(payloadBuf.byteLength),
              }
            : {}),
          ...(extraHeaders ?? {}),
        },
      },
      (res) => {
        const chunks: Buffer[] = [];
        res.on('data', (c: Buffer) => chunks.push(c));
        res.on('end', () => {
          resolve({
            statusCode: res.statusCode ?? 500,
            headers: res.headers,
            bodyText: Buffer.concat(chunks).toString('utf-8'),
          });
        });
      }
    );
    req.on('error', reject);
    if (payloadBuf) req.write(payloadBuf);
    req.end();
  });
}

async function httpRequestJson<T>(
  baseUrl: string,
  method: string,
  reqPath: string,
  body?: unknown,
  extraHeaders?: Record<string, string>
): Promise<{ statusCode: number; headers: http.IncomingHttpHeaders; data: T }> {
  const payload = body !== undefined ? JSON.stringify(body) : undefined;
  const res = await httpRequestRaw(
    baseUrl,
    method,
    reqPath,
    payload,
    extraHeaders
  );
  try {
    const parsed = (res.bodyText ? JSON.parse(res.bodyText) : {}) as T;
    return {
      statusCode: res.statusCode,
      headers: res.headers,
      data: parsed,
    };
  } catch (err) {
    throw new Error(
      `Failed to parse JSON response (${res.statusCode}): ${res.bodyText} (${String(err)})`
    );
  }
}

function httpGetText(
  baseUrl: string,
  reqPath: string,
  extraHeaders?: Record<string, string>
): Promise<{
  statusCode: number;
  headers: http.IncomingHttpHeaders;
  text: string;
}> {
  return httpRequestRaw(baseUrl, 'GET', reqPath, undefined, extraHeaders).then(
    (r) => ({
      statusCode: r.statusCode,
      headers: r.headers,
      text: r.bodyText,
    })
  );
}

describe('Central Webserver & Multiplayer Lobby Registry', () => {
  let server: http.Server;
  let baseUrl = '';

  before(async () => {
    const created = createCentralWebServer({
      ttlSecs: 30,
      relayPortRange: { start: 30100, end: 30139 },
      rateLimitMax: 200,
    });
    server = created.server;
    await new Promise<void>((resolve) => {
      server.listen(0, '127.0.0.1', () => resolve());
    });
    const addr = server.address();
    if (!addr || typeof addr === 'string') {
      throw new Error('Unexpected server address');
    }
    baseUrl = `http://127.0.0.1:${addr.port}`;
  });

  after(async () => {
    await new Promise<void>((resolve) => server.close(() => resolve()));
  });

  it('serves the project homepage, valid CLI commands, relay pool labels, and verified i18n keys', async () => {
    const home = await httpGetText(baseUrl, '/');
    assert.equal(home.statusCode, 200);
    assert.ok(
      home.headers['content-length'],
      'Content-Length header must be set'
    );
    assert.ok(
      home.text.includes('rust-rts.bls-isp.net'),
      'Homepage HTML should include rust-rts.bls-isp.net branding'
    );
    assert.ok(
      home.text.includes('IRONLOCK'),
      'Homepage HTML should include IRONLOCK // RUST-RTS brand identity'
    );
    assert.ok(
      home.text.includes('20 Relay Rooms (40 Ports: 30100–30139)'),
      'Homepage must accurately state 20 Relay Rooms (40 Ports: 30100–30139)'
    );
    assert.ok(
      !home.text.includes('40 Rooms'),
      'Homepage must not overclaim 40 relay rooms'
    );

    const favicon = await httpGetText(baseUrl, '/favicon.svg');
    assert.equal(favicon.statusCode, 200);
    assert.ok(favicon.text.includes('<svg'));

    const css = await httpGetText(baseUrl, '/styles.css');
    assert.equal(css.statusCode, 200);
    assert.ok(css.text.includes('--accent-allied'));

    const appJs = await httpGetText(baseUrl, '/js/app.js');
    assert.equal(appJs.statusCode, 200);
    assert.ok(appJs.text.includes('refreshAllData'));
    assert.ok(
      !appJs.text.includes('--map '),
      'app.js must not emit nonexistent --map flag for rts-client'
    );

    const renderJs = await httpGetText(baseUrl, '/js/render.js');
    assert.equal(renderJs.statusCode, 200);
    assert.ok(renderJs.text.includes('benchmark-balance'));
    assert.ok(renderJs.text.includes('generate-models'));
    assert.ok(renderJs.text.includes('import ./legacy-mod'));
    assert.ok(renderJs.text.includes('scaffold-unit'));
    assert.ok(!renderJs.text.includes('generate-3d-assets'));
    assert.ok(!renderJs.text.includes('import-mod'));
    assert.ok(!renderJs.text.includes('new-unit'));

    assert.ok(home.text.includes('btn-lang-de'));
    assert.ok(home.text.includes('btn-lang-en'));
    assert.ok(home.text.includes('hero-crate-detail'));
    assert.ok(home.text.includes('pipeline-spec-stage'));

    const i18nJs = await httpGetText(baseUrl, '/js/i18n.js');
    assert.equal(i18nJs.statusCode, 200);
    assert.ok(i18nJs.text.includes('UI_STRINGS'));
    assert.ok(i18nJs.text.includes('ENGINE_CRATES_SPEC'));
    assert.ok(i18nJs.text.includes('Deterministic 3D RTS Engine'));
    assert.ok(i18nJs.text.includes('Deterministische 3D-RTS-Engine'));

    // Non-vacuous i18n check across index.html, app.js, and render.js
    const htmlKeys = [
      ...home.text.matchAll(
        /data-i18n(?:-html|-placeholder)?="([^"]+)"/g
      ),
    ].map((m) => m[1]);
    const appJsKeys = [
      ...appJs.text.matchAll(/\bt\(\s*(?:state\.lang|lang)\s*,\s*'([^']+)'\s*\)/g),
    ].map((m) => m[1]);
    const renderJsKeys = [
      ...renderJs.text.matchAll(
        /\bt\(\s*(?:state\.lang|lang)\s*,\s*'([^']+)'\s*\)/g
      ),
    ].map((m) => m[1]);
    const jsKeys = [...appJsKeys, ...renderJsKeys];

    assert.ok(
      htmlKeys.length >= 30,
      `Expected >= 30 HTML data-i18n keys, found ${htmlKeys.length}`
    );
    assert.ok(
      jsKeys.length >= 25,
      `Expected >= 25 JS t(state.lang|lang, ...) keys, found ${jsKeys.length}`
    );

    for (const key of [...new Set([...htmlKeys, ...jsKeys])]) {
      const escapedKey = key.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      const keyRegex = new RegExp(`\\b${escapedKey}\\s*:`, 'g');
      const occurrences = [...i18nJs.text.matchAll(keyRegex)].length;
      assert.ok(
        occurrences >= 2,
        `Key '${key}' must be defined in both DE and EN in i18n.js (found ${occurrences})`
      );
    }

    const cameo = await httpGetText(baseUrl, '/assets/cameos/gtnkicon.webp');
    assert.equal(cameo.statusCode, 200);
    assert.equal(cameo.headers['content-type'], 'image/webp');
    assert.ok(Number(cameo.headers['content-length']) > 500);

    const screen = await httpGetText(
      baseUrl,
      '/assets/screens/country_usa.webp'
    );
    assert.equal(screen.statusCode, 200);
    assert.equal(screen.headers['content-type'], 'image/webp');
    assert.ok(Number(screen.headers['content-length']) > 5000);

    const glbModel = await httpGetText(
      baseUrl,
      '/assets/models/grizzly_tank.glb'
    );
    assert.equal(glbModel.statusCode, 200);
    assert.equal(glbModel.headers['content-type'], 'model/gltf-binary');
    assert.ok(Number(glbModel.headers['content-length']) > 10000);
    assert.ok(
      glbModel.text.startsWith('glTF'),
      'Binary GLB must start with glTF magic header'
    );

    const mapWebp = await httpGetText(
      baseUrl,
      '/assets/maps/mp_01_offense_defense.webp'
    );
    assert.equal(mapWebp.statusCode, 200);
    assert.equal(mapWebp.headers['content-type'], 'image/webp');
    assert.ok(Number(mapWebp.headers['content-length']) > 10000);
  });

  it('serves cached game-database with ETag/304 and computes live commander leaderboard without fake demo seeds', async () => {
    const dbRes = await httpRequestJson<{
      factions: unknown[];
      countries: unknown[];
      units: Array<{
        id: string;
        mesh_vertices?: number;
        mesh_triangles?: number;
        visual: { model: string };
      }>;
      buildings: Array<{
        id: string;
        mesh_vertices?: number;
        mesh_triangles?: number;
        visual: { model: string };
      }>;
      weapons: unknown[];
      maps: Array<{
        id: string;
        theater?: string;
        preview_webp?: string;
        spawns: unknown[];
        ores: unknown[];
      }>;
    }>(baseUrl, 'GET', '/api/game-database');
    assert.equal(dbRes.statusCode, 200);
    assert.ok(dbRes.headers.etag, 'ETag header must be set on /api/game-database');
    assert.ok(
      dbRes.headers['cache-control'],
      'Cache-Control header must be set on /api/game-database'
    );
    assert.equal(dbRes.data.factions.length, 3);
    assert.equal(dbRes.data.countries.length, 10);
    assert.ok(dbRes.data.units.length >= 70);
    assert.ok(dbRes.data.buildings.length >= 76);
    assert.ok(dbRes.data.weapons.length >= 58);
    assert.ok(dbRes.data.maps.length >= 50);
    assert.ok(
      dbRes.data.units.every(
        (u) => (u.mesh_vertices ?? 0) > 0 && (u.mesh_triangles ?? 0) > 0
      ),
      'Every unit must have real 3D GLB mesh vertex and triangle counts'
    );
    assert.ok(
      dbRes.data.buildings.every(
        (b) => (b.mesh_vertices ?? 0) > 0 && (b.mesh_triangles ?? 0) > 0
      ),
      'Every building must have real 3D GLB mesh vertex and triangle counts'
    );
    assert.ok(
      dbRes.data.maps.every(
        (m) => Boolean(m.preview_webp) && Boolean(m.theater)
      ),
      'Every map must have a rendered isometric WebP preview and theater metadata'
    );

    // Conditional request with matching ETag returns 304 Not Modified
    const cachedRes = await httpGetText(baseUrl, '/api/game-database', {
      'If-None-Match': String(dbRes.headers.etag),
    });
    assert.equal(cachedRes.statusCode, 304);
    assert.equal(cachedRes.text, '');

    // Ensure hardcoded fake demo matches are NOT seeded unless SEED_DEMO_MATCHES === 'true'
    const initialMatches = await httpRequestJson<MatchListResponse>(
      baseUrl,
      'GET',
      '/api/matches'
    );
    assert.ok(
      !initialMatches.data.matches.some(
        (m) => m.final_state_hash === '0xcbf29ce484222325'
      ),
      'Demo matches must not pollute production ledger by default'
    );

    // Opt-in SEED_DEMO_MATCHES check on an isolated registry instance
    const demoRegistry = new CentralMultiplayerRegistry(
      45,
      [],
      Date.now(),
      { start: 30100, end: 30139 },
      { seedDemoMatches: true }
    );
    assert.equal(demoRegistry.listMatches().length, 5);

    // Record real matches and verify leaderboard computation
    await httpRequestJson<MatchRecord>(baseUrl, 'POST', '/api/matches', {
      lobby_id: 'lobby-lb-1',
      lobby_name: '',
      mod_id: 'rust-ra2',
      map_id: 'skirmish_valley',
      players: [
        { name: 'AlphaCmdr', faction: 'Allied', team: 1, is_winner: true },
        { name: 'BravoCmdr', faction: 'Soviet', team: 2, is_winner: false },
      ],
      winner_player: 'AlphaCmdr',
      winner_faction: 'Allied',
      final_tick: 12000,
      final_state_hash: '0xaaaa111122223333',
    });
    await httpRequestJson<MatchRecord>(baseUrl, 'POST', '/api/matches', {
      lobby_id: 'lobby-lb-2',
      lobby_name: 'EU Ranked #2',
      mod_id: 'rust-ra2',
      map_id: 'river_crossing',
      players: [
        { name: 'AlphaCmdr', faction: 'Allied', team: 1, is_winner: true },
        { name: 'CharlieCmdr', faction: 'Yuri', team: 2, is_winner: false },
      ],
      winner_player: 'AlphaCmdr',
      winner_faction: 'Allied',
      final_tick: 14400,
      final_state_hash: '0xbbbb444455556666',
    });

    const lbRes = await httpRequestJson<LeaderboardResponse>(
      baseUrl,
      'GET',
      '/api/leaderboard'
    );
    assert.equal(lbRes.statusCode, 200);
    assert.ok(lbRes.data.leaderboard.length >= 3);
    assert.equal(lbRes.data.leaderboard[0].rank, 1);
    assert.equal(lbRes.data.leaderboard[0].player_name, 'AlphaCmdr');
  });

  it('exposes workspace project info with real mods from rust-ra2 and mods/', async () => {
    const res = await httpRequestJson<ProjectOverview>(
      baseUrl,
      'GET',
      '/api/project-info'
    );
    assert.equal(res.statusCode, 200);
    assert.equal(res.data.tick_rate_hz, 20);
    assert.ok(res.data.crates.length >= 5);
    assert.ok(res.data.mods.some((m) => m.id === 'rust-ra2'));
  });

  it('registers, lists, filters, heartbeats, and deletes lobbies with hostToken authentication', async () => {
    const createRes = await httpRequestJson<RegisteredLobbyResponse>(
      baseUrl,
      'POST',
      '/api/lobbies',
      {
        lobby_name: 'Operation Red Dawn [Allied vs Soviet]',
        host_player: 'CommanderGener',
        host_addr: '0.0.0.0:7878',
        mod_id: 'rust-ra2',
        mod_name: 'Red Alert 2: Rust Total Conversion',
        mod_version: '0.1.0',
        map_id: 'skirmish_valley',
        map_name: 'Skirmish Valley',
        current_players: 1,
        max_players: 2,
        game_mode: 'Standard',
        password_protected: false,
      }
    );

    assert.equal(createRes.statusCode, 201);
    assert.ok(
      createRes.headers['content-length'],
      'Rust LobbyRegistryClient requires Content-Length'
    );
    assert.equal(createRes.data.lobby_id, 'lobby-1');
    assert.equal(createRes.data.host_addr, '127.0.0.1:7878');
    assert.equal(createRes.data.status, 'open');
    assert.equal(typeof createRes.data.hostToken, 'string');
    assert.equal(createRes.data.hostToken.length, 64);
    assert.equal(createRes.data.host_token, createRes.data.hostToken);

    const hostToken = createRes.data.hostToken;

    // Public GET /api/lobbies MUST omit hostToken / host_token
    const listRes = await httpRequestJson<LobbyListResponse>(
      baseUrl,
      'GET',
      '/api/lobbies?mod_id=rust-ra2&only_open=true'
    );
    assert.equal(listRes.statusCode, 200);
    assert.equal(listRes.data.lobbies.length, 1);
    assert.equal(
      listRes.data.lobbies[0].lobby_name,
      'Operation Red Dawn [Allied vs Soviet]'
    );
    assert.equal(
      (listRes.data.lobbies[0] as unknown as Record<string, unknown>).hostToken,
      undefined,
      'Public lobby listing must never leak hostToken'
    );
    assert.equal(
      (listRes.data.lobbies[0] as unknown as Record<string, unknown>).host_token,
      undefined,
      'Public lobby listing must never leak host_token'
    );

    // Unauthenticated or forged heartbeat & DELETE must be rejected (401 / 403)
    const unauthHb = await httpRequestJson<{ error: string }>(
      baseUrl,
      'POST',
      `/api/lobbies/${createRes.data.lobby_id}/heartbeat`,
      { current_players: 2 }
    );
    assert.equal(unauthHb.statusCode, 401);

    const wrongTokenHb = await httpRequestJson<{ error: string }>(
      baseUrl,
      'POST',
      `/api/lobbies/${createRes.data.lobby_id}/heartbeat`,
      { current_players: 2 },
      { 'X-Host-Token': '0'.repeat(64) }
    );
    assert.equal(wrongTokenHb.statusCode, 403);

    const unauthDel = await httpRequestJson<{ error: string }>(
      baseUrl,
      'DELETE',
      `/api/lobbies/${createRes.data.lobby_id}`
    );
    assert.equal(unauthDel.statusCode, 401);

    const wrongTokenDel = await httpRequestJson<{ error: string }>(
      baseUrl,
      'DELETE',
      `/api/lobbies/${createRes.data.lobby_id}`,
      undefined,
      { Authorization: `Bearer ${'f'.repeat(64)}` }
    );
    assert.equal(wrongTokenDel.statusCode, 403);

    // Authenticated heartbeat via X-Host-Token updates 2/2 -> full
    const hbRes = await httpRequestJson<LobbyEntry>(
      baseUrl,
      'POST',
      `/api/lobbies/${createRes.data.lobby_id}/heartbeat`,
      {
        current_players: 2,
        max_players: 2,
      },
      { 'X-Host-Token': hostToken }
    );
    assert.equal(hbRes.statusCode, 200);
    assert.equal(hbRes.data.status, 'full');
    assert.equal(hbRes.data.current_players, 2);

    // Authenticated heartbeat via Authorization: Bearer also succeeds
    const bearerHb = await httpRequestJson<LobbyEntry>(
      baseUrl,
      'POST',
      `/api/lobbies/${createRes.data.lobby_id}/heartbeat`,
      { status: 'in_game' },
      { Authorization: `Bearer ${hostToken}` }
    );
    assert.equal(bearerHb.statusCode, 200);
    assert.equal(bearerHb.data.status, 'in_game');

    const openOnlyRes = await httpRequestJson<LobbyListResponse>(
      baseUrl,
      'GET',
      '/api/lobbies?only_open=true'
    );
    assert.equal(openOnlyRes.statusCode, 200);
    assert.equal(openOnlyRes.data.lobbies.length, 0);

    const statsRes = await httpRequestJson<RegistryStats>(
      baseUrl,
      'GET',
      '/api/stats'
    );
    assert.equal(statsRes.statusCode, 200);
    assert.equal(statsRes.data.public_domain, 'rust-rts.bls-isp.net');
    assert.equal(statsRes.data.active_lobbies, 1);
    assert.equal(statsRes.data.in_game_lobbies, 1);
    assert.equal(statsRes.data.players_online, 2);

    // Authenticated DELETE succeeds
    const delRes = await httpRequestJson<{ deleted: boolean }>(
      baseUrl,
      'DELETE',
      `/api/lobbies/${createRes.data.lobby_id}`,
      undefined,
      { 'X-Host-Token': hostToken }
    );
    assert.equal(delRes.statusCode, 200);
    assert.equal(delRes.data.deleted, true);

    const afterDel = await httpRequestJson<LobbyListResponse>(
      baseUrl,
      'GET',
      '/api/lobbies'
    );
    assert.equal(afterDel.data.lobbies.length, 0);
  });

  it('verifies optional lobby passwords without exposing secrets', async () => {
    const createRes = await httpRequestJson<RegisteredLobbyResponse>(
      baseUrl,
      'POST',
      '/api/lobbies',
      {
        lobby_name: 'Private Clan Scrim',
        host_player: 'ClanLeader',
        host_addr: '127.0.0.1:7891',
        mod_id: 'rust-ra2',
        password: 'iron-curtain-secret',
      }
    );
    assert.equal(createRes.statusCode, 201);
    assert.equal(createRes.data.password_protected, true);

    const badPw = await httpRequestJson<{ valid: boolean }>(
      baseUrl,
      'POST',
      `/api/lobbies/${createRes.data.lobby_id}/verify-password`,
      { password: 'wrong-password' }
    );
    assert.equal(badPw.statusCode, 403);
    assert.equal(badPw.data.valid, false);

    const okPw = await httpRequestJson<{ valid: boolean }>(
      baseUrl,
      'POST',
      `/api/lobbies/${createRes.data.lobby_id}/verify-password`,
      { password: 'iron-curtain-secret' }
    );
    assert.equal(okPw.statusCode, 200);
    assert.equal(okPw.data.valid, true);

    await httpRequestJson<{ deleted: boolean }>(
      baseUrl,
      'DELETE',
      `/api/lobbies/${createRes.data.lobby_id}`,
      undefined,
      { 'X-Host-Token': createRes.data.hostToken }
    );
  });

  it('lists and records deterministic lockstep matches compatibly with rts-network', async () => {
    const initial = await httpRequestJson<MatchListResponse>(
      baseUrl,
      'GET',
      '/api/matches'
    );
    assert.equal(initial.statusCode, 200);
    const beforeCount = initial.data.matches.length;

    // Empty lobby_name defaults to "Skirmish Match" and preserves lobby_id + team
    const postMatch = await httpRequestJson<MatchRecord>(
      baseUrl,
      'POST',
      '/api/matches',
      {
        lobby_id: 'lobby-42',
        lobby_name: '   ',
        mod_id: 'rust-ra2',
        map_id: 'river_crossing',
        map_name: 'River Crossing',
        players: [
          {
            name: 'CommanderGener',
            faction: 'Allied',
            team: 1,
            is_winner: true,
          },
          {
            name: 'SovietAI',
            faction: 'Soviet',
            team: 2,
            is_winner: false,
          },
        ],
        winner_player: 'CommanderGener',
        winner_faction: 'Allied',
        final_tick: 19200,
        final_state_hash: '0x1122334455667788',
        replayUrl: '/replays/river_crossing_42.replay.json',
      }
    );
    assert.equal(postMatch.statusCode, 201);
    assert.equal(postMatch.data.lobby_name, 'Skirmish Match');
    assert.equal(postMatch.data.lobby_id, 'lobby-42');
    assert.equal(postMatch.data.players[0].team, 1);
    assert.equal(postMatch.data.players[1].team, 2);
    assert.equal(postMatch.data.winner_player, 'CommanderGener');
    assert.equal(postMatch.data.final_state_hash, '0x1122334455667788');
    assert.equal(
      postMatch.data.replayUrl,
      '/replays/river_crossing_42.replay.json'
    );

    const after = await httpRequestJson<MatchListResponse>(
      baseUrl,
      'GET',
      '/api/matches'
    );
    assert.equal(after.data.matches.length, beforeCount + 1);

    const filtered = await httpRequestJson<MatchListResponse>(
      baseUrl,
      'GET',
      '/api/matches?search=0x1122334455667788'
    );
    assert.equal(filtered.data.matches.length, 1);
  });

  it('rejects Stored XSS replayUrl schemes in POST /api/matches and client HTML renderer', async () => {
    const xssJs = await httpRequestJson<{ error: string }>(
      baseUrl,
      'POST',
      '/api/matches',
      {
        lobby_name: 'XSS Probe 1',
        winner_player: 'Attacker',
        final_state_hash: '0xdeadbeef00000001',
        replayUrl: 'javascript:alert(document.cookie)',
      }
    );
    assert.equal(xssJs.statusCode, 400);

    const xssData = await httpRequestJson<{ error: string }>(
      baseUrl,
      'POST',
      '/api/matches',
      {
        lobby_name: 'XSS Probe 2',
        winner_player: 'Attacker',
        final_state_hash: '0xdeadbeef00000002',
        replay_url: 'data:text/html,<script>alert(1)</script>',
      }
    );
    assert.equal(xssData.statusCode, 400);

    // Dynamically import actual compiled ES2022 browser bundle public/js/render.js
    const renderModuleUrl = pathToFileURL(
      path.resolve(__dirname, '..', '..', 'public', 'js', 'render.js')
    ).href;
    const dynamicEsmImport = new Function('u', 'return import(u)') as <T>(
      u: string
    ) => Promise<T>;
    const renderMod = await dynamicEsmImport<{
      sanitizeSafeUrl: (raw: string | undefined | null) => string | null;
      renderMatchesRowsHtml: (
        matches: MatchRecord[],
        nowMs?: number,
        lang?: 'de' | 'en'
      ) => string;
    }>(renderModuleUrl);

    assert.equal(renderMod.sanitizeSafeUrl('javascript:alert(1)'), null);
    assert.equal(renderMod.sanitizeSafeUrl('JaVaScRiPt:alert(1)'), null);
    assert.equal(
      renderMod.sanitizeSafeUrl('data:text/html,<svg onload=alert(1)>'),
      null
    );
    assert.equal(renderMod.sanitizeSafeUrl('//evil.example/payload'), null);
    assert.equal(
      renderMod.sanitizeSafeUrl('https://rust-rts.bls-isp.net/replays/ok.json'),
      'https://rust-rts.bls-isp.net/replays/ok.json'
    );
    assert.equal(
      renderMod.sanitizeSafeUrl('/replays/ok.replay.json'),
      '/replays/ok.replay.json'
    );

    const baseMatch: MatchRecord = {
      match_id: 'match-xss-check',
      lobby_id: null,
      lobby_name: 'Test Match',
      mod_id: 'rust-ra2',
      mod_name: 'RA2',
      mod_version: '1.0.0',
      map_id: 'skirmish_valley',
      map_name: 'Skirmish Valley',
      game_mode: 'Standard',
      players: [{ name: 'P1', faction: 'Allied', team: 1, is_winner: true }],
      winner_player: 'P1',
      winner_faction: 'Allied',
      duration_secs: 120,
      final_tick: 2400,
      final_state_hash: '0x1234',
      total_commands: 12,
      desync_detected: false,
      replay_file: null,
      completed_at_unix_ms: Date.now(),
    };

    const unsafeHtml = renderMod.renderMatchesRowsHtml(
      [
        { ...baseMatch, replayUrl: 'javascript:alert(1)' },
        {
          ...baseMatch,
          match_id: 'match-xss-2',
          replay_url: 'data:text/html,<script>alert(1)</script>',
        },
      ],
      Date.now(),
      'en'
    );
    assert.ok(!unsafeHtml.includes('javascript:'));
    assert.ok(!unsafeHtml.includes('data:text/html'));
    assert.ok(!unsafeHtml.includes('<a '));

    const safeHtml = renderMod.renderMatchesRowsHtml(
      [
        {
          ...baseMatch,
          replayUrl: 'https://rust-rts.bls-isp.net/replays/m1.replay.json',
        },
      ],
      Date.now(),
      'en'
    );
    assert.ok(
      safeHtml.includes(
        'href="https://rust-rts.bls-isp.net/replays/m1.replay.json"'
      )
    );
  });

  it('enforces a strict 64 KB request body cap with HTTP 413', async () => {
    const oversizedPadding = 'A'.repeat(70 * 1024);
    const oversizedBody = JSON.stringify({
      lobby_name: oversizedPadding,
      host_player: 'FloodHost',
    });

    const res = await httpRequestRaw(
      baseUrl,
      'POST',
      '/api/lobbies',
      oversizedBody
    );
    assert.equal(res.statusCode, 413);

    // Server remains healthy immediately after rejecting oversized payload
    const health = await httpRequestJson<{ status: string }>(
      baseUrl,
      'GET',
      '/api/health'
    );
    assert.equal(health.statusCode, 200);
    assert.equal(health.data.status, 'online');
  });

  it('allocates a Reverse-TCP NAT Relay Tunnel Room within configured range and cleans up sockets', async () => {
    assert.equal(DEFAULT_RELAY_IDLE_TIMEOUT_MS, 120_000);
    assert.equal(maxRelayRoomsForRange({ start: 30100, end: 30139 }), 20);

    const createRes = await httpRequestJson<RegisteredLobbyResponse>(
      baseUrl,
      'POST',
      '/api/lobbies',
      {
        lobby_name: 'CGNAT Relay Test Lobby',
        host_player: 'HostBehindNat',
        host_addr: '127.0.0.1:7899',
        mod_id: 'rust-ra2',
        mod_name: 'Red Alert 2: Rust Total Conversion',
        mod_version: '0.1.0',
        map_id: 'skirmish_valley',
        map_name: 'Skirmish Valley',
        current_players: 1,
        max_players: 2,
        game_mode: 'Standard',
        password_protected: false,
        allocate_relay: true,
      }
    );

    assert.equal(createRes.statusCode, 201);
    assert.ok(createRes.data.relay_addr, 'relay_addr must be allocated');
    assert.ok(
      createRes.data.relay_uplink_addr,
      'relay_uplink_addr must be allocated to host'
    );
    assert.equal(createRes.data.nat_mode, 'Relay-Tunnel (NAT-Safe)');

    const uplinkPort = Number(createRes.data.relay_uplink_addr!.split(':')[1]);
    const clientPort = Number(createRes.data.relay_addr!.split(':')[1]);
    assert.ok(uplinkPort >= 30100 && uplinkPort <= 30139);
    assert.ok(clientPort >= 30100 && clientPort <= 30139);

    // Public lobby list omits private relay_uplink_addr
    const listed = await httpRequestJson<LobbyListResponse>(
      baseUrl,
      'GET',
      '/api/lobbies'
    );
    const pubLobby = listed.data.lobbies.find(
      (l) => l.lobby_id === createRes.data.lobby_id
    );
    assert.ok(pubLobby);
    assert.equal(pubLobby.relay_uplink_addr, null);

    // Connect host standby uplink and WAN client, verify 'R' wakeup marker + bidirectional payload
    const exchangeResult = await new Promise<string>((resolve, reject) => {
      const hostUplink = net.createConnection({
        host: '127.0.0.1',
        port: uplinkPort,
      });
      let receivedByHost = '';

      hostUplink.on('data', (chunk) => {
        receivedByHost += chunk.toString('utf-8');
        if (receivedByHost === 'RHELLO_LOCKSTEP') {
          hostUplink.write('WELCOME_PEER');
        }
      });
      hostUplink.on('error', reject);

      hostUplink.on('connect', () => {
        const wanClient = net.createConnection({
          host: '127.0.0.1',
          port: clientPort,
        });
        wanClient.on('error', reject);
        wanClient.on('connect', () => {
          wanClient.write('HELLO_LOCKSTEP');
        });
        wanClient.on('data', (chunk) => {
          const msg = chunk.toString('utf-8');
          wanClient.destroy();
          hostUplink.destroy();
          resolve(msg);
        });
      });
    });

    assert.equal(exchangeResult, 'WELCOME_PEER');

    await httpRequestJson<{ deleted: boolean }>(
      baseUrl,
      'DELETE',
      `/api/lobbies/${createRes.data.lobby_id}`,
      undefined,
      { 'X-Host-Token': createRes.data.hostToken }
    );
  });

  it('enforces relay port pool exhaustion (503) without falling back to ephemeral port 0', async () => {
    const capped = createCentralWebServer({
      ttlSecs: 30,
      relayPortRange: { start: 39400, end: 39403 }, // 4 ports -> max 2 relay rooms
      rateLimitMax: 50,
    });
    await new Promise<void>((resolve) =>
      capped.server.listen(0, '127.0.0.1', () => resolve())
    );
    const cappedAddr = capped.server.address();
    if (!cappedAddr || typeof cappedAddr === 'string') {
      throw new Error('Unexpected capped server address');
    }
    const cappedUrl = `http://127.0.0.1:${cappedAddr.port}`;

    try {
      const r1 = await httpRequestJson<RegisteredLobbyResponse>(
        cappedUrl,
        'POST',
        '/api/lobbies',
        {
          lobby_name: 'Relay Room 1',
          host_player: 'Host1',
          host_addr: '127.0.0.1:7801',
          allocate_relay: true,
        }
      );
      assert.equal(r1.statusCode, 201);

      const r2 = await httpRequestJson<RegisteredLobbyResponse>(
        cappedUrl,
        'POST',
        '/api/lobbies',
        {
          lobby_name: 'Relay Room 2',
          host_player: 'Host2',
          host_addr: '127.0.0.1:7802',
          allocate_relay: true,
        }
      );
      assert.equal(r2.statusCode, 201);

      // 3rd room exceeds (39403 - 39400 + 1) / 2 = 2 rooms -> must return 503 capacity error!
      const r3 = await httpRequestJson<{ error: string; code?: string }>(
        cappedUrl,
        'POST',
        '/api/lobbies',
        {
          lobby_name: 'Relay Room 3 (Overflow)',
          host_player: 'Host3',
          host_addr: '127.0.0.1:7803',
          allocate_relay: true,
        }
      );
      assert.equal(r3.statusCode, 503);
      assert.ok(r3.data.error.toLowerCase().includes('exhausted'));

      // Releasing Room 1 frees its two ports so Room 3 can now be allocated inside 39400-39403
      const del1 = await httpRequestJson<{ deleted: boolean }>(
        cappedUrl,
        'DELETE',
        `/api/lobbies/${r1.data.lobby_id}`,
        undefined,
        { 'X-Host-Token': r1.data.hostToken }
      );
      assert.equal(del1.statusCode, 200);

      const r4 = await httpRequestJson<RegisteredLobbyResponse>(
        cappedUrl,
        'POST',
        '/api/lobbies',
        {
          lobby_name: 'Relay Room 4 (Reused Slot)',
          host_player: 'Host4',
          host_addr: '127.0.0.1:7804',
          allocate_relay: true,
        }
      );
      assert.equal(r4.statusCode, 201);
      const clientPort = Number(r4.data.relay_addr!.split(':')[1]);
      assert.ok(clientPort >= 39400 && clientPort <= 39403);
    } finally {
      await new Promise<void>((resolve) =>
        capped.server.close(() => resolve())
      );
    }
  });

  it('handles production domain rust-rts.bls-isp.net on loopback proxy and blocks untrusted headers/CORS/traversal/floods', async () => {
    const hostA = await httpRequestJson<RegisteredLobbyResponse>(
      baseUrl,
      'POST',
      '/api/lobbies',
      {
        lobby_name: 'Berlin Allied Host',
        host_player: 'HostBerlin',
        host_addr: '127.0.0.1:7878',
        mod_id: 'rust-ra2',
        mod_name: 'Red Alert 2: Rust Total Conversion',
        mod_version: '0.1.0',
        map_id: 'skirmish_valley',
        map_name: 'Skirmish Valley',
        current_players: 1,
        max_players: 2,
        game_mode: 'Standard',
        allocate_relay: true,
      },
      {
        Host: 'rust-rts.bls-isp.net',
        'X-Forwarded-For': '84.150.11.11, 10.0.0.1',
      }
    );

    const hostB = await httpRequestJson<RegisteredLobbyResponse>(
      baseUrl,
      'POST',
      '/api/lobbies',
      {
        lobby_name: 'Munich Soviet Host',
        host_player: 'HostMunich',
        host_addr: '127.0.0.1:7878',
        mod_id: 'rust-ra2',
        mod_name: 'Red Alert 2: Rust Total Conversion',
        mod_version: '0.1.0',
        map_id: 'river_crossing',
        map_name: 'River Crossing',
        current_players: 1,
        max_players: 2,
        game_mode: 'Standard',
        allocate_relay: true,
      },
      {
        Host: 'rust-rts.bls-isp.net',
        'X-Forwarded-For': '91.64.22.22',
      }
    );

    assert.notEqual(
      hostA.data.lobby_id,
      hostB.data.lobby_id,
      'Two hosts behind NAT using default 127.0.0.1:7878 must not overwrite each other'
    );
    assert.equal(hostA.data.host_addr, '84.150.11.11:7878');
    assert.equal(hostB.data.host_addr, '91.64.22.22:7878');
    assert.ok(
      hostA.data.relay_addr?.startsWith('rust-rts.bls-isp.net:'),
      `Expected relay_addr to use rust-rts.bls-isp.net, got ${hostA.data.relay_addr}`
    );
    assert.ok(
      hostA.data.relay_uplink_addr?.startsWith('rust-rts.bls-isp.net:'),
      `Expected relay_uplink_addr to use rust-rts.bls-isp.net, got ${hostA.data.relay_uplink_addr}`
    );

    await httpRequestJson<{ deleted: boolean }>(
      baseUrl,
      'DELETE',
      `/api/lobbies/${hostA.data.lobby_id}`,
      undefined,
      { 'X-Host-Token': hostA.data.hostToken }
    );
    await httpRequestJson<{ deleted: boolean }>(
      baseUrl,
      'DELETE',
      `/api/lobbies/${hostB.data.lobby_id}`,
      undefined,
      { 'X-Host-Token': hostB.data.hostToken }
    );

    // Cross-origin mutating request from untrusted Origin must be blocked (403)
    const crossOriginPost = await httpRequestJson<{ error: string }>(
      baseUrl,
      'POST',
      '/api/lobbies',
      { lobby_name: 'CSRF Attempt', host_player: 'Attacker' },
      { Origin: 'https://evil.example' }
    );
    assert.equal(crossOriginPost.statusCode, 403);

    // Directory traversal attempts must never escape publicDir
    const traversal = await httpGetText(baseUrl, '/..%2fpackage.json');
    assert.equal(traversal.statusCode, 404);

    // Untrusted proxy mode (trustProxy: false) + rate limiting (429)
    const strict = createCentralWebServer({
      ttlSecs: 30,
      trustProxy: false,
      rateLimitMax: 2,
      rateLimitWindowMs: 60_000,
    });
    await new Promise<void>((resolve) =>
      strict.server.listen(0, '127.0.0.1', () => resolve())
    );
    const strictAddr = strict.server.address();
    if (!strictAddr || typeof strictAddr === 'string') {
      throw new Error('Unexpected strict server address');
    }
    const strictUrl = `http://127.0.0.1:${strictAddr.port}`;

    try {
      const spoofed = await httpRequestJson<RegisteredLobbyResponse>(
        strictUrl,
        'POST',
        '/api/lobbies',
        {
          lobby_name: 'Spoofed Header Lobby',
          host_player: 'Peer1',
          host_addr: '0.0.0.0:7878',
        },
        { 'X-Forwarded-For': '203.0.113.99' }
      );
      assert.equal(spoofed.statusCode, 201);
      assert.equal(
        spoofed.data.host_addr,
        '127.0.0.1:7878',
        'Untrusted proxy headers must be ignored when trustProxy is false'
      );

      const req2 = await httpRequestJson<RegisteredLobbyResponse>(
        strictUrl,
        'POST',
        '/api/lobbies',
        {
          lobby_name: 'Second Allowed Request',
          host_player: 'Peer2',
          host_addr: '127.0.0.1:7879',
        }
      );
      assert.equal(req2.statusCode, 201);

      const req3 = await httpRequestJson<{ error: string }>(
        strictUrl,
        'POST',
        '/api/lobbies',
        {
          lobby_name: 'Rate Limited Third Request',
          host_player: 'Peer3',
          host_addr: '127.0.0.1:7880',
        }
      );
      assert.equal(req3.statusCode, 429);
      assert.ok(req3.headers['retry-after']);
    } finally {
      await new Promise<void>((resolve) =>
        strict.server.close(() => resolve())
      );
    }
  });

  it('serves GET /api/v1/stun-whoami with accurate RFC 6598 CGNAT, RFC 1918 LAN, and public WAN classification', async () => {
    const loopbackRes = await httpRequestJson<StunWhoamiResponse>(
      baseUrl,
      'GET',
      '/api/v1/stun-whoami'
    );
    assert.equal(loopbackRes.statusCode, 200);
    assert.equal(loopbackRes.data.ip, '127.0.0.1');
    assert.ok(loopbackRes.data.port > 0);
    assert.equal(loopbackRes.data.is_cgnat, true);

    const cgnatRes = await httpRequestJson<StunWhoamiResponse>(
      baseUrl,
      'GET',
      '/api/v1/stun-whoami',
      undefined,
      {
        'X-Forwarded-For': '100.64.88.19',
        'X-Forwarded-Port': '49152',
      }
    );
    assert.equal(cgnatRes.statusCode, 200);
    assert.equal(cgnatRes.data.ip, '100.64.88.19');
    assert.equal(cgnatRes.data.port, 49152);
    assert.equal(cgnatRes.data.is_cgnat, true);

    const lanRes = await httpRequestJson<StunWhoamiResponse>(
      baseUrl,
      'GET',
      '/api/v1/stun-whoami',
      undefined,
      { 'X-Forwarded-For': '192.168.178.42' }
    );
    assert.equal(lanRes.statusCode, 200);
    assert.equal(lanRes.data.is_cgnat, true);

    const wanRes = await httpRequestJson<StunWhoamiResponse>(
      baseUrl,
      'GET',
      '/api/v1/stun-whoami',
      undefined,
      {
        'X-Forwarded-For': '84.150.11.11',
        'X-Forwarded-Port': '51422',
      }
    );
    assert.equal(wanRes.statusCode, 200);
    assert.equal(wanRes.data.ip, '84.150.11.11');
    assert.equal(wanRes.data.port, 51422);
    assert.equal(wanRes.data.is_cgnat, false);
  });

  it('multiplexes 510+ concurrent relay rooms on 1 single TCP port with zero 503 exhaustion and clean MUX framing', async () => {
    const muxSrv = createCentralWebServer({
      ttlSecs: 60,
      relayMuxPort: 0,
      rateLimitMax: 2000,
    });
    await new Promise<void>((resolve) =>
      muxSrv.server.listen(0, '127.0.0.1', () => resolve())
    );
    const addr = muxSrv.server.address();
    if (!addr || typeof addr === 'string') {
      throw new Error('Unexpected mux server address');
    }
    const muxUrl = `http://127.0.0.1:${addr.port}`;

    try {
      const createdRooms: RegisteredLobbyResponse[] = [];
      for (let i = 0; i < 510; i++) {
        const res = await httpRequestJson<RegisteredLobbyResponse>(
          muxUrl,
          'POST',
          '/api/lobbies',
          {
            lobby_name: `Mux Room #${i + 1}`,
            host_player: `MuxHost${i + 1}`,
            host_addr: `10.0.${Math.floor(i / 250)}.${(i % 250) + 1}:7878`,
            mod_id: 'rust-ra2',
            allocate_relay: true,
          }
        );
        assert.equal(
          res.statusCode,
          201,
          `Room ${i + 1} must succeed without 503 RELAY_POOL_EXHAUSTED`
        );
        createdRooms.push(res.data);
      }

      const sharedMuxPort = createdRooms[0].relay_mux_port!;
      assert.ok(sharedMuxPort > 0, 'Single mux TCP port must be assigned');
      const uniqueTokens = new Set<string>();
      for (const r of createdRooms) {
        assert.equal(
          r.relay_mux_port,
          sharedMuxPort,
          'All 510 relay rooms must share 1 single TCP mux listener port'
        );
        assert.ok(r.relay_room_token?.startsWith('rm-'));
        assert.ok(
          r.relay_addr?.endsWith(`:${sharedMuxPort}#mux:${r.relay_room_token}`)
        );
        uniqueTokens.add(r.relay_room_token!);
      }
      assert.equal(uniqueTokens.size, 510);

      // Verify concurrent isolated bidirectional MUX tunnels on the single port (including pipelined client bytes)
      const runMuxTunnelExchange = (
        roomToken: string,
        clientPayload: string,
        hostReply: string
      ): Promise<{ hostSaw: string; clientSaw: string }> =>
        new Promise((resolve, reject) => {
          const hostSock = net.createConnection({
            host: '127.0.0.1',
            port: sharedMuxPort,
          });
          let hostSaw = '';
          hostSock.on('error', reject);
          hostSock.on('data', (chunk) => {
            hostSaw += chunk.toString('utf-8');
            if (hostSaw === `R${clientPayload}`) {
              hostSock.write(hostReply);
            }
          });
          hostSock.on('connect', () => {
            hostSock.write(`MUX ${roomToken} HOST\n`, () => {
              const clientSock = net.createConnection({
                host: '127.0.0.1',
                port: sharedMuxPort,
              });
              clientSock.on('error', reject);
              clientSock.on('connect', () => {
                // Send preamble + raw lockstep payload pipelined in a single write!
                clientSock.write(`MUX ${roomToken} CLIENT\n${clientPayload}`);
              });
              clientSock.on('data', (chunk) => {
                const clientSaw = chunk.toString('utf-8');
                clientSock.destroy();
                hostSock.destroy();
                resolve({ hostSaw, clientSaw });
              });
            });
          });
        });

      const [exA, exB] = await Promise.all([
        runMuxTunnelExchange(
          createdRooms[0].relay_room_token!,
          'HELLO_ROOM_000',
          'WELCOME_ROOM_000'
        ),
        runMuxTunnelExchange(
          createdRooms[509].relay_room_token!,
          'HELLO_ROOM_509',
          'WELCOME_ROOM_509'
        ),
      ]);

      assert.equal(exA.hostSaw, 'RHELLO_ROOM_000');
      assert.equal(exA.clientSaw, 'WELCOME_ROOM_000');
      assert.equal(exB.hostSaw, 'RHELLO_ROOM_509');
      assert.equal(exB.clientSaw, 'WELCOME_ROOM_509');

      // Forged/unknown room token must be immediately closed by mux server
      await new Promise<void>((resolve, reject) => {
        const badSock = net.createConnection({
          host: '127.0.0.1',
          port: sharedMuxPort,
        });
        badSock.on('error', () => resolve());
        badSock.on('connect', () => {
          badSock.write('MUX rm-nonexistent-forged CLIENT\n');
        });
        badSock.on('close', () => resolve());
        setTimeout(() => reject(new Error('Forged mux token did not close')), 2000);
      });
    } finally {
      await muxSrv.registry.closeAllRelayRooms();
      await new Promise<void>((resolve) => muxSrv.server.close(() => resolve()));
    }
  });

  it('verifies HMAC-SHA256 signed match reports, blocks spoofing & replays, computes Glicko-2 MMR, and persists atomically', async () => {
    const tmpDataDir = path.resolve(
      __dirname,
      `.tmp_state_${process.pid}_${Date.now()}`
    );
    fs.rmSync(tmpDataDir, { recursive: true, force: true });

    const ranked = createCentralWebServer({
      ttlSecs: 30,
      relayMuxPort: 0,
      dataDir: tmpDataDir,
      rateLimitMax: 500,
    });
    await new Promise<void>((resolve) =>
      ranked.server.listen(0, '127.0.0.1', () => resolve())
    );
    const addr = ranked.server.address();
    if (!addr || typeof addr === 'string') {
      throw new Error('Unexpected ranked server address');
    }
    const rankedUrl = `http://127.0.0.1:${addr.port}`;

    const kirov = createPlayerIdentityProfile(
      'CmdrKirov',
      '11'.repeat(32)
    );
    const prism = createPlayerIdentityProfile(
      'CmdrPrism',
      '22'.repeat(32)
    );
    const imposter = createPlayerIdentityProfile(
      'CmdrKirov',
      '33'.repeat(32)
    );

    try {
      const reg1 = await httpRequestJson<{ registered: boolean; player_id: string }>(
        rankedUrl,
        'POST',
        '/api/v1/players/register',
        {
          player_id: kirov.player_id,
          display_name: kirov.display_name,
          public_token: kirov.public_token,
        }
      );
      assert.equal(reg1.statusCode, 201);
      assert.equal(reg1.data.registered, true);

      const reg2 = await httpRequestJson<{ registered: boolean }>(
        rankedUrl,
        'POST',
        '/api/v1/players/register',
        {
          player_id: prism.player_id,
          display_name: prism.display_name,
          public_token: prism.public_token,
        }
      );
      assert.equal(reg2.statusCode, 201);

      // Different cryptographic keypair attempting to claim 'CmdrKirov' must be rejected (403)
      const spoofNameRes = await httpRequestJson<{ error: string }>(
        rankedUrl,
        'POST',
        '/api/v1/players/register',
        {
          player_id: imposter.player_id,
          display_name: 'CmdrKirov',
          public_token: imposter.public_token,
        }
      );
      assert.equal(spoofNameRes.statusCode, 403);

      // Mismatched player_id vs public_token must be rejected (401)
      const badBindingRes = await httpRequestJson<{ error: string }>(
        rankedUrl,
        'POST',
        '/api/v1/players/register',
        {
          player_id: 'plr-0000000000000000',
          display_name: 'BadKeyCmdr',
          public_token: imposter.public_token,
        }
      );
      assert.equal(badBindingRes.statusCode, 401);

      // Unauthenticated legacy POST /api/matches attempting to spoof registered 'CmdrKirov' must be rejected (403)
      const unauthSpoof = await httpRequestJson<{ error: string }>(
        rankedUrl,
        'POST',
        '/api/matches',
        {
          lobby_name: 'Forged Match',
          players: [
            { name: 'AnonAttacker', faction: 'Soviet', team: 1, is_winner: true },
            { name: 'CmdrKirov', faction: 'Allied', team: 2, is_winner: false },
          ],
          winner_player: 'AnonAttacker',
          winner_faction: 'Soviet',
        }
      );
      assert.equal(unauthSpoof.statusCode, 403);

      // Unsigned or tampered report to POST /api/v1/matches must be rejected (401)
      const matchReq = {
        lobby_id: 'lobby-ranked-1',
        lobby_name: '1v1 Ladder Finals',
        mod_id: 'rust-ra2',
        map_id: 'skirmish_valley',
        game_mode: '1v1',
        players: [
          { name: 'CmdrKirov', faction: 'Soviet', team: 1, is_winner: true },
          { name: 'CmdrPrism', faction: 'Allied', team: 2, is_winner: false },
        ],
        winner_player: 'CmdrKirov',
        winner_faction: 'Soviet',
        final_tick: 9600,
        final_state_hash: '0xabcdef0123456789',
        total_commands: 410,
        desync_detected: false,
      };

      const signed = signMatchReport(matchReq, kirov, 1_700_000_000_000);
      const tampered = { ...signed, winner_player: 'CmdrPrism' };
      const tamperedRes = await httpRequestJson<{ error: string }>(
        rankedUrl,
        'POST',
        '/api/v1/matches',
        tampered
      );
      assert.equal(tamperedRes.statusCode, 401);

      // Authentic signed match report succeeds (201)
      const okRes = await httpRequestJson<MatchRecord>(
        rankedUrl,
        'POST',
        '/api/v1/matches',
        signed
      );
      assert.equal(okRes.statusCode, 201);
      assert.equal(okRes.data.winner_player, 'CmdrKirov');

      // Replaying the identical signed match report is rejected (409 Conflict)
      const replayRes = await httpRequestJson<{ error: string }>(
        rankedUrl,
        'POST',
        '/api/v1/matches',
        signed
      );
      assert.equal(replayRes.statusCode, 409);

      // Verify Glicko-2 rating, RD reduction, and win/loss streaks on GET /api/v1/leaderboard
      const lbRes = await httpRequestJson<LeaderboardResponse>(
        rankedUrl,
        'GET',
        '/api/v1/leaderboard'
      );
      assert.equal(lbRes.statusCode, 200);
      const kirovRow = lbRes.data.leaderboard.find(
        (r) => r.player_name === 'CmdrKirov'
      );
      const prismRow = lbRes.data.leaderboard.find(
        (r) => r.player_name === 'CmdrPrism'
      );
      assert.ok(kirovRow && prismRow);
      assert.ok(
        kirovRow.rating > 1600,
        `Expected winner Glicko-2 rating > 1600, got ${kirovRow.rating}`
      );
      assert.ok(
        kirovRow.rd < 300,
        `Expected winner Glicko-2 RD < 300, got ${kirovRow.rd}`
      );
      assert.equal(kirovRow.streak, 1);
      assert.equal(kirovRow.player_id, kirov.player_id);

      assert.ok(
        prismRow.rating < 1400,
        `Expected loser Glicko-2 rating < 1400, got ${prismRow.rating}`
      );
      assert.ok(prismRow.rd < 300);
      assert.equal(prismRow.streak, -1);
      assert.equal(prismRow.player_id, prism.player_id);

      // Verify atomic state persistence (zero leftover .tmp files + full state restored on restart)
      const dirFiles = fs.readdirSync(tmpDataDir);
      assert.deepEqual(dirFiles, ['registry-state.json']);

      const reloaded = new CentralMultiplayerRegistry(
        30,
        [],
        Date.now(),
        undefined,
        { dataDir: tmpDataDir }
      );
      const reloadedLb = reloaded.getLeaderboard();
      const reloadedKirov = reloadedLb.find((r) => r.player_name === 'CmdrKirov');
      assert.ok(reloadedKirov);
      assert.equal(reloadedKirov.rating, kirovRow.rating);
      assert.equal(reloadedKirov.rd, kirovRow.rd);
      assert.equal(reloadedKirov.streak, 1);
      assert.throws(
        () => reloaded.recordSignedMatch(signed),
        (err: unknown) =>
          err instanceof Error && err.message.toLowerCase().includes('replay')
      );
    } finally {
      await ranked.registry.closeAllRelayRooms();
      await new Promise<void>((resolve) =>
        ranked.server.close(() => resolve())
      );
      fs.rmSync(tmpDataDir, { recursive: true, force: true });
    }
  });

  it('pairs 1v1 and 2v2 players via expanding ELO automatch queue and provisions mux relay rooms', async () => {
    const mm = createCentralWebServer({
      ttlSecs: 30,
      relayMuxPort: 0,
      rateLimitMax: 500,
    });
    await new Promise<void>((resolve) =>
      mm.server.listen(0, '127.0.0.1', () => resolve())
    );
    const addr = mm.server.address();
    if (!addr || typeof addr === 'string') {
      throw new Error('Unexpected automatch server address');
    }
    const mmUrl = `http://127.0.0.1:${addr.port}`;

    try {
      const p1 = createPlayerIdentityProfile('DuelAlpha', 'a1'.repeat(32));
      const p2 = createPlayerIdentityProfile('DuelBravo', 'b2'.repeat(32));

      // Enqueue P1 -> searching, then cancel -> cancelled
      const enq1a = await httpRequestJson<AutomatchPollResponse>(
        mmUrl,
        'POST',
        '/api/v1/matchmaking/enqueue',
        {
          player_id: p1.player_id,
          display_name: p1.display_name,
          public_token: p1.public_token,
          mod_id: 'rust-ra2',
          mode: '1v1',
          preferred_faction: 'Allied',
          host_addr: '127.0.0.1:7901',
        }
      );
      assert.equal(enq1a.statusCode, 201);
      assert.equal(enq1a.data.status, 'searching');

      const cancelRes = await httpRequestJson<{ cancelled: boolean }>(
        mmUrl,
        'POST',
        '/api/v1/matchmaking/cancel',
        {
          ticket_id: enq1a.data.ticket_id,
          player_id: p1.player_id,
        }
      );
      assert.equal(cancelRes.statusCode, 200);
      assert.equal(cancelRes.data.cancelled, true);

      const pollCancelled = await httpRequestJson<AutomatchPollResponse>(
        mmUrl,
        'GET',
        `/api/v1/matchmaking/poll?ticket_id=${enq1a.data.ticket_id}`
      );
      assert.equal(pollCancelled.data.status, 'cancelled');

      // Re-enqueue P1 + enqueue P2 -> instant 1v1 match + auto-allocated #mux relay room
      const enq1b = await httpRequestJson<AutomatchPollResponse>(
        mmUrl,
        'POST',
        '/api/v1/matchmaking/enqueue',
        {
          player_id: p1.player_id,
          display_name: p1.display_name,
          public_token: p1.public_token,
          mod_id: 'rust-ra2',
          mode: '1v1',
          preferred_faction: 'Allied',
          host_addr: '127.0.0.1:7901',
        }
      );
      assert.equal(enq1b.data.status, 'searching');

      const enq2 = await httpRequestJson<AutomatchPollResponse>(
        mmUrl,
        'POST',
        '/api/v1/matchmaking/enqueue',
        {
          player_id: p2.player_id,
          display_name: p2.display_name,
          public_token: p2.public_token,
          mod_id: 'rust-ra2',
          mode: '1v1',
          preferred_faction: 'Soviet',
        }
      );
      assert.equal(enq2.statusCode, 201);
      assert.equal(enq2.data.status, 'matched');
      assert.ok(enq2.data.assignment);

      const poll1b = await httpRequestJson<AutomatchPollResponse>(
        mmUrl,
        'GET',
        `/api/v1/matchmaking/poll?ticket_id=${enq1b.data.ticket_id}`
      );
      assert.equal(poll1b.data.status, 'matched');
      assert.ok(poll1b.data.assignment);
      assert.equal(
        poll1b.data.assignment.match_token,
        enq2.data.assignment.match_token
      );
      assert.equal(
        poll1b.data.assignment.random_seed,
        enq2.data.assignment.random_seed
      );
      assert.equal(
        poll1b.data.assignment.is_host !== enq2.data.assignment.is_host,
        true
      );
      assert.ok(poll1b.data.assignment.relay_addr.includes('#mux:rm-'));

      // Now test 2v2 with divergent ratings requiring ELO window expansion before pairing
      const t1 = createPlayerIdentityProfile('QuadOne', 'c1'.repeat(32));
      const t2 = createPlayerIdentityProfile('QuadTwo', 'c2'.repeat(32));
      const t3 = createPlayerIdentityProfile('QuadThree', 'c3'.repeat(32));
      const t4 = createPlayerIdentityProfile('QuadFour', 'c4'.repeat(32));

      // Give QuadOne two wins over QuadFour so ratings spread apart (> 100 ELO gap)
      for (let g = 0; g < 2; g++) {
        const rep = signMatchReport(
          {
            mod_id: 'rust-ra2',
            map_id: 'skirmish_valley',
            game_mode: '1v1',
            players: [
              { name: t1.display_name, faction: 'Allied', team: 1, is_winner: true },
              { name: t4.display_name, faction: 'Soviet', team: 2, is_winner: false },
            ],
            winner_player: t1.display_name,
            winner_faction: 'Allied',
            final_tick: 4000 + g * 100,
            final_state_hash: `0x9999000${g}`,
          },
          t1,
          1_700_000_100_000 + g
        );
        mm.registry.recordSignedMatch(rep);
      }

      const baseT0 = Date.now();
      const q1 = await mm.matchmaking.enqueue(
        {
          player_id: t1.player_id,
          display_name: t1.display_name,
          public_token: t1.public_token,
          mod_id: 'rust-ra2',
          mode: '2v2',
          preferred_faction: 'Allied',
        },
        '127.0.0.1',
        baseT0
      );
      const q2 = await mm.matchmaking.enqueue(
        {
          player_id: t2.player_id,
          display_name: t2.display_name,
          public_token: t2.public_token,
          mod_id: 'rust-ra2',
          mode: '2v2',
          preferred_faction: 'Soviet',
        },
        '127.0.0.1',
        baseT0
      );
      const q3 = await mm.matchmaking.enqueue(
        {
          player_id: t3.player_id,
          display_name: t3.display_name,
          public_token: t3.public_token,
          mod_id: 'rust-ra2',
          mode: '2v2',
          preferred_faction: 'Yuri',
        },
        '127.0.0.1',
        baseT0
      );
      const q4 = await mm.matchmaking.enqueue(
        {
          player_id: t4.player_id,
          display_name: t4.display_name,
          public_token: t4.public_token,
          mod_id: 'rust-ra2',
          mode: '2v2',
          preferred_faction: 'Allied',
        },
        '127.0.0.1',
        baseT0
      );

      // At t = baseT0 (window = 100), spread between QuadOne (~1748) and QuadFour (~1252) is ~496 > 100 -> still searching!
      assert.equal(q4.status, 'searching');

      // Advance 20 seconds (window = 100 + 20*25 = 600 >= 496) -> all 4 pair with balanced snake-draft [1, 2, 2, 1]!
      const matched2v2 = await mm.matchmaking.poll(
        q1.ticket_id,
        '127.0.0.1',
        baseT0 + 20_000
      );
      assert.ok(matched2v2);
      assert.equal(matched2v2.status, 'matched');
      assert.ok(matched2v2.assignment);
      assert.equal(matched2v2.assignment.mode, '2v2');
      assert.equal(matched2v2.assignment.players.length, 4);
      assert.deepEqual(
        matched2v2.assignment.players.map((p) => p.team_id),
        [1, 2, 2, 1]
      );
      assert.ok(q2.ticket_id && q3.ticket_id);
    } finally {
      await mm.registry.closeAllRelayRooms();
      await new Promise<void>((resolve) => mm.server.close(() => resolve()));
    }
  });

  it('reflects public UDP endpoints and pairs host/joiner P2P hole-punch sessions via UdpRendezvousServer', async () => {
    const rendezvous = await UdpRendezvousServer.create(0, '127.0.0.1');
    const hostSock = dgram.createSocket('udp4');
    const clientSock = dgram.createSocket('udp4');

    const bindUdp = (sock: dgram.Socket): Promise<number> =>
      new Promise((resolve, reject) => {
        sock.once('error', reject);
        sock.bind(0, '127.0.0.1', () => {
          const addr = sock.address();
          resolve(typeof addr === 'string' ? 0 : addr.port);
        });
      });

    try {
      const hostLocalPort = await bindUdp(hostSock);
      const clientLocalPort = await bindUdp(clientSock);

      const hostMessages: Array<Record<string, unknown>> = [];
      const clientMessages: Array<Record<string, unknown>> = [];
      hostSock.on('message', (buf) => {
        hostMessages.push(JSON.parse(buf.toString('utf-8')));
      });
      clientSock.on('message', (buf) => {
        clientMessages.push(JSON.parse(buf.toString('utf-8')));
      });

      const waitForMsg = async (
        arr: Array<Record<string, unknown>>,
        pred: (m: Record<string, unknown>) => boolean
      ): Promise<Record<string, unknown>> => {
        const deadline = Date.now() + 3000;
        while (Date.now() < deadline) {
          const hit = arr.find(pred);
          if (hit) return hit;
          await new Promise((r) => setTimeout(r, 15));
        }
        throw new Error('Timed out waiting for UDP datagram');
      };

      // 1. Endpoint reflection (`whoami`)
      hostSock.send(
        Buffer.from(JSON.stringify({ op: 'whoami', nonce: 4242 }), 'utf-8'),
        rendezvous.port,
        '127.0.0.1'
      );
      const whoamiAck = await waitForMsg(
        hostMessages,
        (m) => m.op === 'whoami_ack'
      );
      assert.equal(whoamiAck.ip, '127.0.0.1');
      assert.equal(whoamiAck.port, hostLocalPort);
      assert.equal(whoamiAck.nonce, 4242);

      // 2. Host + Client registration & bilateral `punch_peer` notification
      hostSock.send(
        Buffer.from(
          JSON.stringify({
            op: 'register',
            room_token: 'rm-punch-test-01',
            role: 'host',
            peer_id: 'host-p1',
            local_addr: `192.168.1.10:${hostLocalPort}`,
          }),
          'utf-8'
        ),
        rendezvous.port,
        '127.0.0.1'
      );
      await waitForMsg(hostMessages, (m) => m.op === 'register_ack');

      clientSock.send(
        Buffer.from(
          JSON.stringify({
            op: 'register',
            room_token: 'rm-punch-test-01',
            role: 'client',
            peer_id: 'client-p2',
            local_addr: `192.168.1.20:${clientLocalPort}`,
          }),
          'utf-8'
        ),
        rendezvous.port,
        '127.0.0.1'
      );

      const hostPeerMsg = await waitForMsg(
        hostMessages,
        (m) => m.op === 'punch_peer'
      );
      const clientPeerMsg = await waitForMsg(
        clientMessages,
        (m) => m.op === 'punch_peer'
      );

      assert.equal(hostPeerMsg.peer_public_addr, `127.0.0.1:${clientLocalPort}`);
      assert.equal(
        hostPeerMsg.peer_local_addr,
        `192.168.1.20:${clientLocalPort}`
      );
      assert.equal(clientPeerMsg.peer_public_addr, `127.0.0.1:${hostLocalPort}`);
      assert.equal(
        clientPeerMsg.peer_local_addr,
        `192.168.1.10:${hostLocalPort}`
      );
      assert.equal(hostPeerMsg.punch_token, clientPeerMsg.punch_token);
      assert.equal(String(hostPeerMsg.punch_token).length, 16);

      // 3. Direct P2P datagram exchange between the paired UDP sockets
      clientSock.send(
        Buffer.from(
          JSON.stringify({
            op: 'p2p_probe',
            token: clientPeerMsg.punch_token,
          }),
          'utf-8'
        ),
        hostLocalPort,
        '127.0.0.1'
      );
      const p2pHit = await waitForMsg(hostMessages, (m) => m.op === 'p2p_probe');
      assert.equal(p2pHit.token, hostPeerMsg.punch_token);
    } finally {
      hostSock.close();
      clientSock.close();
      await rendezvous.close();
    }
  });

  it('publishes, validates, filters, sorts, streams, and persists Workshop .rtsmod and .toml packages', async () => {
    const tmpWorkshopDir = path.resolve(
      __dirname,
      '..',
      '.tmp-workshop-persistence-test'
    );
    fs.rmSync(tmpWorkshopDir, { recursive: true, force: true });

    const buildValidRtsMod03 = (
      modId: string,
      modVersion: string,
      contentHash: bigint
    ): Buffer => {
      const manifestRaw = Buffer.from(
        `id = "${modId}"\nname = "${modId}"\nversion = "${modVersion}"\n`,
        'utf-8'
      );
      const compressed = zlib.deflateSync(manifestRaw);
      const indexJson = Buffer.from(
        `{"mod_id":${JSON.stringify(modId)},"mod_name":${JSON.stringify(
          modId
        )},"mod_version":${JSON.stringify(
          modVersion
        )},"content_hash":${contentHash.toString(10)},"entries":[{"path":"mod.toml","offset":0,"compressed_len":${
          compressed.length
        },"uncompressed_len":${manifestRaw.length},"compressed":true}]}`,
        'utf-8'
      );
      const header = Buffer.alloc(12);
      header.write('RTSMOD03', 0, 8, 'ascii');
      header.writeUInt32LE(indexJson.length, 8);
      return Buffer.concat([header, indexJson, compressed]);
    };

    const httpGetBytes = (
      urlBase: string,
      reqPath: string
    ): Promise<{
      statusCode: number;
      headers: http.IncomingHttpHeaders;
      body: Buffer;
    }> =>
      new Promise((resolve, reject) => {
        http
          .get(new URL(reqPath, urlBase), (res) => {
            const chunks: Buffer[] = [];
            res.on('data', (c: Buffer) => chunks.push(c));
            res.on('end', () =>
              resolve({
                statusCode: res.statusCode ?? 500,
                headers: res.headers,
                body: Buffer.concat(chunks),
              })
            );
          })
          .on('error', reject);
      });

    const ws = createCentralWebServer({
      ttlSecs: 30,
      workshopDir: tmpWorkshopDir,
      maxWorkshopUploadBytes: 8192,
      workshopApiToken: 'secret-workshop-ci-token',
      rateLimitMax: 200,
    });
    await new Promise<void>((resolve) => {
      ws.server.listen(0, '127.0.0.1', () => resolve());
    });
    const addr = ws.server.address();
    if (!addr || typeof addr === 'string') {
      throw new Error('Unexpected workshop test server address');
    }
    const wsUrl = `http://127.0.0.1:${addr.port}`;

    try {
      const authorA = createPlayerIdentityProfile('ModAuthorAlpha');
      const authorB = createPlayerIdentityProfile('ModAuthorBravo');

      const modHashBig = 0xcbf29ce484222325n;
      const modHashHex = `0x${modHashBig.toString(16)}`;
      const modBlob = buildValidRtsMod03('chrono-redux', '1.2.0', modHashBig);
      const modSha256 = crypto
        .createHash('sha256')
        .update(modBlob)
        .digest('hex');

      // 1. Reject unauthenticated publish (401)
      const unauth = await httpRequestJson<{ error: string }>(
        wsUrl,
        'POST',
        '/api/v1/workshop/publish',
        {
          id: 'chrono-redux',
          title: 'Chrono Redux Total Balance',
          author_name: authorA.display_name,
          player_id: authorA.player_id,
          kind: 'mod',
          version: '1.2.0',
          sha256: modSha256,
          mod_content_hash: modHashHex,
          payload_base64: modBlob.toString('base64'),
        }
      );
      assert.equal(unauth.statusCode, 401);

      // 2. Reject SHA-256 mismatch even with valid HMAC (400)
      const wrongSha = '0'.repeat(64);
      const badShaSigned = signWorkshopPublish(
        {
          id: 'chrono-redux',
          title: 'Chrono Redux',
          kind: 'mod',
          version: '1.2.0',
          sha256: wrongSha,
          mod_content_hash: modHashHex,
        },
        authorA
      );
      const badShaResp = await httpRequestJson<{ error: string }>(
        wsUrl,
        'POST',
        '/api/v1/workshop/publish',
        {
          ...badShaSigned,
          payload_base64: modBlob.toString('base64'),
        }
      );
      assert.equal(badShaResp.statusCode, 400);

      // 3. Reject archive content_hash mismatch (400)
      const badContentSigned = signWorkshopPublish(
        {
          id: 'chrono-redux',
          title: 'Chrono Redux',
          kind: 'mod',
          version: '1.2.0',
          sha256: modSha256,
          mod_content_hash: '0x1111222233334444',
        },
        authorA
      );
      const badContentResp = await httpRequestJson<{ error: string }>(
        wsUrl,
        'POST',
        '/api/v1/workshop/publish',
        {
          ...badContentSigned,
          payload_base64: modBlob.toString('base64'),
        }
      );
      assert.equal(badContentResp.statusCode, 400);

      // 4. Reject oversized upload exceeding maxWorkshopUploadBytes = 8192 (413)
      const hugeBlob = Buffer.alloc(9000, 0x41);
      const hugeSha = crypto.createHash('sha256').update(hugeBlob).digest('hex');
      const hugeSigned = signWorkshopPublish(
        {
          id: 'too-big-map',
          title: 'Oversized Map',
          kind: 'map',
          version: '0.1.0',
          sha256: hugeSha,
          mod_content_hash: '0x1234abcd',
        },
        authorA
      );
      const hugeResp = await httpRequestRaw(
        wsUrl,
        'POST',
        '/api/v1/workshop/publish',
        hugeBlob,
        {
          'Content-Type': 'application/octet-stream',
          'X-Workshop-Metadata': Buffer.from(
            JSON.stringify(hugeSigned),
            'utf-8'
          ).toString('base64'),
        }
      );
      assert.equal(hugeResp.statusCode, 413);

      // 5. Publish valid .rtsmod via HMAC-SHA256 JSON envelope (201)
      const validModSigned = signWorkshopPublish(
        {
          id: 'chrono-redux',
          title: 'Chrono Redux Total Balance',
          description: 'Competitive 1v1 balance overhaul',
          kind: 'mod',
          version: '1.2.0',
          tags: ['Balance', 'Competitive', 'RA2'],
          sha256: modSha256,
          mod_content_hash: modHashHex,
        },
        authorA,
        Date.now() - 5000
      );
      const pubMod = await httpRequestJson<WorkshopItemMetadata>(
        wsUrl,
        'POST',
        '/api/v1/workshop/publish',
        {
          ...validModSigned,
          payload_base64: modBlob.toString('base64'),
        }
      );
      assert.equal(pubMod.statusCode, 201);
      assert.equal(pubMod.data.id, 'chrono-redux');
      assert.equal(pubMod.data.file_name, 'chrono-redux.rtsmod');
      assert.equal(pubMod.data.sha256, modSha256);

      // 6. Reject overwrite of 'chrono-redux' by a different publisher (403)
      const hijackSigned = signWorkshopPublish(
        {
          id: 'chrono-redux',
          title: 'Hijacked Mod',
          kind: 'mod',
          version: '9.9.9',
          sha256: modSha256,
          mod_content_hash: modHashHex,
        },
        authorB
      );
      const hijackResp = await httpRequestJson<{ error: string }>(
        wsUrl,
        'POST',
        '/api/v1/workshop/publish',
        {
          ...hijackSigned,
          payload_base64: modBlob.toString('base64'),
        }
      );
      assert.equal(hijackResp.statusCode, 403);

      // 7. Publish valid .toml map via binary octet-stream + Bearer API token (201)
      const mapToml = Buffer.from(
        'id = "arctic_ridge"\nname = "Arctic Ridge (2v2)"\nwidth = 96\nheight = 96\n',
        'utf-8'
      );
      const mapSha256 = crypto
        .createHash('sha256')
        .update(mapToml)
        .digest('hex');
      const pubMapRaw = await httpRequestRaw(
        wsUrl,
        'POST',
        '/api/v1/workshop/publish',
        mapToml,
        {
          'Content-Type': 'application/octet-stream',
          Authorization: 'Bearer secret-workshop-ci-token',
          'X-Workshop-Metadata': JSON.stringify({
            id: 'arctic-ridge-2v2',
            title: 'Arctic Ridge 2v2',
            description: 'Snowy high-ground skirmish map',
            author_name: authorB.display_name,
            player_id: authorB.player_id,
            kind: 'map',
            version: '1.0.0',
            tags: ['snow', '2v2'],
            sha256: mapSha256,
            mod_content_hash: '0x8899aabbccddeeff',
          }),
        }
      );
      assert.equal(pubMapRaw.statusCode, 201);

      // 8. Stream binary download & verify download counter + search/tag/kind/sort filters
      const dl1 = await httpGetBytes(
        wsUrl,
        '/api/v1/workshop/download/chrono-redux'
      );
      assert.equal(dl1.statusCode, 200);
      assert.equal(dl1.headers['x-workshop-sha256'], modSha256);
      assert.equal(dl1.headers['x-workshop-downloads'], '1');
      assert.ok(dl1.body.equals(modBlob));

      const dl2 = await httpGetBytes(
        wsUrl,
        '/api/v1/workshop/download/chrono-redux'
      );
      assert.equal(dl2.statusCode, 200);
      assert.equal(dl2.headers['x-workshop-downloads'], '2');

      const allSortedByDownloads = await httpRequestJson<WorkshopItemListResponse>(
        wsUrl,
        'GET',
        '/api/v1/workshop/items?sort=downloads'
      );
      assert.equal(allSortedByDownloads.statusCode, 200);
      assert.equal(allSortedByDownloads.data.total, 2);
      assert.equal(allSortedByDownloads.data.items[0].id, 'chrono-redux');
      assert.equal(allSortedByDownloads.data.items[0].downloads, 2);

      const onlyMaps = await httpRequestJson<WorkshopItemListResponse>(
        wsUrl,
        'GET',
        '/api/v1/workshop/items?kind=map&tag=2v2&search=arctic'
      );
      assert.equal(onlyMaps.data.total, 1);
      assert.equal(onlyMaps.data.items[0].id, 'arctic-ridge-2v2');

      // 9. Verify atomic disk persistence (no leftover .tmp files + full state reloaded)
      const rootFiles = fs.readdirSync(tmpWorkshopDir).sort();
      assert.deepEqual(rootFiles, ['blobs', 'index.json']);
      const reloadedWorkshop = new WorkshopManager({
        workshopDir: tmpWorkshopDir,
      });
      const reloadedMod = reloadedWorkshop.getItem('chrono-redux');
      assert.ok(reloadedMod);
      assert.equal(reloadedMod.downloads, 2);
      assert.equal(reloadedMod.sha256, modSha256);
    } finally {
      await ws.registry.closeAllRelayRooms();
      await new Promise<void>((resolve) => ws.server.close(() => resolve()));
      fs.rmSync(tmpWorkshopDir, { recursive: true, force: true });
    }
  });

  it('registers, lists, heartbeats, enforces 401/403 token & fleet_secret auth, and evicts stale dedicated servers', async () => {
    const app = createAppServer({
      ttlSecs: 30,
      dedicatedTtlMs: 1_000,
      relayMuxPort: 0,
      rateLimitMax: 500,
    });
    await new Promise<void>((resolve) =>
      app.server.listen(0, '127.0.0.1', () => resolve())
    );
    const addr = app.server.address();
    if (!addr || typeof addr === 'string') {
      throw new Error('Unexpected fleet test server address');
    }
    const fleetUrl = `http://127.0.0.1:${addr.port}`;

    try {
      // Invalid tcp_addr or missing mod_id must return 400
      const badAddr = await httpRequestJson<{ error: string }>(
        fleetUrl,
        'POST',
        '/api/v1/servers/register',
        {
          tcp_addr: 'not-a-host-port',
          mod_id: 'rust-ra2',
        }
      );
      assert.equal(badAddr.statusCode, 400);

      // Register EU dedicated server
      const regEu = await httpRequestJson<DedicatedServerRegisterResponse>(
        fleetUrl,
        'POST',
        '/api/v1/servers/register',
        {
          server_id: 'ds-eu-central-01',
          display_name: 'IRONLOCK EU #1',
          tcp_addr: '0.0.0.0:7910',
          udp_port: 7911,
          region: 'eu-central',
          mod_id: 'rust-ra2',
          mod_version: '0.1.0',
          mod_content_hash: '0xcbf29ce484222325',
          max_players: 4,
        }
      );
      assert.equal(regEu.statusCode, 201);
      assert.equal(regEu.data.server_id, 'ds-eu-central-01');
      assert.equal(regEu.data.tcp_addr, '127.0.0.1:7910');
      assert.equal(regEu.data.udp_port, 7911);
      assert.equal(regEu.data.status, 'idle');
      assert.equal(regEu.data.server_token.length, 64);
      assert.ok(regEu.data.relay_addr?.includes('#mux:rm-'));
      assert.ok(regEu.data.relay_uplink_addr?.includes('#mux:rm-'));

      // Register US dedicated server without relay allocation
      const regUs = await httpRequestJson<DedicatedServerRegisterResponse>(
        fleetUrl,
        'POST',
        '/api/v1/servers/register',
        {
          server_id: 'ds-us-east-01',
          display_name: 'IRONLOCK US #1',
          tcp_addr: '198.51.100.25:7920',
          region: 'us-east',
          mod_id: 'rust-ra2',
          mod_version: '0.1.0',
          allocate_relay: false,
        }
      );
      assert.equal(regUs.statusCode, 201);
      assert.equal(regUs.data.relay_addr, null);

      // Re-registering existing active server_id without or with bad server_token -> 401 / 403
      const reRegNoTok = await httpRequestJson<{ error: string }>(
        fleetUrl,
        'POST',
        '/api/v1/servers/register',
        {
          server_id: 'ds-eu-central-01',
          tcp_addr: '127.0.0.1:7910',
          mod_id: 'rust-ra2',
        }
      );
      assert.equal(reRegNoTok.statusCode, 401);

      const reRegBadTok = await httpRequestJson<{ error: string }>(
        fleetUrl,
        'POST',
        '/api/v1/servers/register',
        {
          server_id: 'ds-eu-central-01',
          tcp_addr: '127.0.0.1:7910',
          mod_id: 'rust-ra2',
          server_token: '0'.repeat(64),
        }
      );
      assert.equal(reRegBadTok.statusCode, 403);

      const reRegOk = await httpRequestJson<DedicatedServerRegisterResponse>(
        fleetUrl,
        'POST',
        '/api/v1/servers/register',
        {
          server_id: 'ds-eu-central-01',
          display_name: 'IRONLOCK EU #1 (Updated)',
          tcp_addr: '127.0.0.1:7910',
          mod_id: 'rust-ra2',
        },
        { 'X-Server-Token': regEu.data.server_token }
      );
      assert.equal(reRegOk.statusCode, 201);
      assert.equal(reRegOk.data.server_token, regEu.data.server_token);
      assert.equal(reRegOk.data.display_name, 'IRONLOCK EU #1 (Updated)');

      // Public GET /api/v1/servers redacts relay_uplink_addr and never exposes server_token
      const listAll = await httpRequestJson<DedicatedServerListResponse>(
        fleetUrl,
        'GET',
        '/api/v1/servers'
      );
      assert.equal(listAll.statusCode, 200);
      assert.equal(listAll.data.total, 2);
      assert.equal(listAll.data.idle, 2);
      assert.equal(listAll.data.active, 0);
      for (const srv of listAll.data.servers) {
        assert.equal(srv.relay_uplink_addr, null);
        assert.equal(
          (srv as unknown as Record<string, unknown>).server_token,
          undefined
        );
      }

      const listEu = await httpRequestJson<DedicatedServerListResponse>(
        fleetUrl,
        'GET',
        '/api/v1/servers?region=eu-central&mod_id=rust-ra2&status=idle'
      );
      assert.equal(listEu.data.total, 1);
      assert.equal(listEu.data.servers[0].server_id, 'ds-eu-central-01');

      // Registry stats reflects online and idle dedicated servers
      const stats = await httpRequestJson<RegistryStats>(
        fleetUrl,
        'GET',
        '/api/stats'
      );
      assert.equal(stats.statusCode, 200);
      assert.equal(stats.data.dedicated_servers_online, 2);
      assert.equal(stats.data.dedicated_servers_idle, 2);

      // Heartbeat auth checks: 404 unknown, 401 missing token, 403 wrong token, 200 valid token
      const hbUnknown = await httpRequestJson<{ error: string }>(
        fleetUrl,
        'POST',
        '/api/v1/servers/heartbeat',
        { server_id: 'ds-does-not-exist', server_token: regEu.data.server_token }
      );
      assert.equal(hbUnknown.statusCode, 404);

      const hbMissingTok = await httpRequestJson<{ error: string }>(
        fleetUrl,
        'POST',
        '/api/v1/servers/heartbeat',
        { server_id: 'ds-eu-central-01', status: 'in_game' }
      );
      assert.equal(hbMissingTok.statusCode, 401);

      const hbWrongTok = await httpRequestJson<{ error: string }>(
        fleetUrl,
        'POST',
        '/api/v1/servers/heartbeat',
        { server_id: 'ds-eu-central-01', status: 'in_game' },
        { Authorization: `Bearer ${'a'.repeat(64)}` }
      );
      assert.equal(hbWrongTok.statusCode, 403);

      const hbValid = await httpRequestJson<DedicatedServerEntry>(
        fleetUrl,
        'POST',
        '/api/v1/servers/heartbeat',
        {
          server_id: 'ds-eu-central-01',
          status: 'in_game',
          current_players: 2,
          active_tick: 640,
        },
        { Authorization: `Bearer ${regEu.data.server_token}` }
      );
      assert.equal(hbValid.statusCode, 200);
      assert.equal(hbValid.data.status, 'in_game');
      assert.equal(hbValid.data.current_players, 2);
      assert.equal(hbValid.data.active_tick, 640);
      assert.ok(
        hbValid.data.relay_uplink_addr?.includes('#mux:rm-'),
        'Authenticated heartbeat must retain relay_uplink_addr for server host'
      );

      // Advance past dedicatedTtlMs (1,000 ms) via fleet pruneExpired / listServers and verify TTL eviction + mux relay release
      assert.equal(app.registry.activeRelayRooms, 1);
      const evictedList = app.dedicatedFleet.listServers(
        {},
        Date.now() + 2_500
      );
      assert.equal(evictedList.total, 0);
      assert.equal(
        app.registry.activeRelayRooms,
        0,
        'Mux relay room must be released when stale dedicated server expires'
      );
    } finally {
      await app.registry.closeAllRelayRooms();
      await new Promise<void>((resolve) => app.server.close(() => resolve()));
    }

    // Also verify optional DEDICATED_FLEET_SECRET enforcement
    const secretApp = createAppServer({
      ttlSecs: 30,
      dedicatedFleetSecret: 'ironlock-fleet-shared-secret',
      relayMuxPort: 0,
    });
    await new Promise<void>((resolve) =>
      secretApp.server.listen(0, '127.0.0.1', () => resolve())
    );
    const secAddr = secretApp.server.address();
    if (!secAddr || typeof secAddr === 'string') {
      throw new Error('Unexpected secret fleet test server address');
    }
    const secUrl = `http://127.0.0.1:${secAddr.port}`;
    try {
      const noSec = await httpRequestJson<{ error: string }>(
        secUrl,
        'POST',
        '/api/v1/servers/register',
        { tcp_addr: '127.0.0.1:7930', mod_id: 'rust-ra2' }
      );
      assert.equal(noSec.statusCode, 401);

      const badSec = await httpRequestJson<{ error: string }>(
        secUrl,
        'POST',
        '/api/v1/servers/register',
        {
          tcp_addr: '127.0.0.1:7930',
          mod_id: 'rust-ra2',
          fleet_secret: 'wrong-secret',
        }
      );
      assert.equal(badSec.statusCode, 403);

      const okSec = await httpRequestJson<DedicatedServerRegisterResponse>(
        secUrl,
        'POST',
        '/api/v1/servers/register',
        {
          tcp_addr: '127.0.0.1:7930',
          mod_id: 'rust-ra2',
          fleet_secret: 'ironlock-fleet-shared-secret',
        }
      );
      assert.equal(okSec.statusCode, 201);
    } finally {
      await secretApp.registry.closeAllRelayRooms();
      await new Promise<void>((resolve) =>
        secretApp.server.close(() => resolve())
      );
    }
  });

  it('allocates dedicated servers via POST /api/v1/servers/allocate and recycles rooms cleanly on completion heartbeat', async () => {
    const app = createAppServer({
      ttlSecs: 30,
      relayMuxPort: 0,
      rateLimitMax: 500,
    });
    await new Promise<void>((resolve) =>
      app.server.listen(0, '127.0.0.1', () => resolve())
    );
    const addr = app.server.address();
    if (!addr || typeof addr === 'string') {
      throw new Error('Unexpected allocate test server address');
    }
    const apiUrl = `http://127.0.0.1:${addr.port}`;

    try {
      const euSrv = await httpRequestJson<DedicatedServerRegisterResponse>(
        apiUrl,
        'POST',
        '/api/v1/servers/register',
        {
          server_id: 'ds-alloc-eu',
          tcp_addr: '198.51.100.10:7940',
          region: 'eu-central',
          mod_id: 'rust-ra2',
          mod_version: '0.1.0',
          max_players: 4,
        }
      );
      const mutSrv = await httpRequestJson<DedicatedServerRegisterResponse>(
        apiUrl,
        'POST',
        '/api/v1/servers/register',
        {
          server_id: 'ds-alloc-mut',
          tcp_addr: '198.51.100.11:7941',
          region: 'eu-central',
          mod_id: 'rust-ra2',
          mod_version: '0.1.0',
          active_mutators: ['instagib', 'fast_build'],
          max_players: 4,
        }
      );
      assert.equal(euSrv.statusCode, 201);
      assert.equal(mutSrv.statusCode, 201);

      // Allocate standard match -> picks vanilla ds-alloc-eu
      const alloc1 = await httpRequestJson<{
        allocated: boolean;
        server: DedicatedServerEntry;
      }>(apiUrl, 'POST', '/api/v1/servers/allocate', {
        mod_id: 'rust-ra2',
        mod_version: '0.1.0',
        required_players: 2,
        region: 'eu-central',
        map_id: 'skirmish_valley',
        mode: '1v1',
        match_token: 'match-tok-001',
      });
      assert.equal(alloc1.statusCode, 200);
      assert.equal(alloc1.data.allocated, true);
      assert.equal(alloc1.data.server.server_id, 'ds-alloc-eu');
      assert.equal(alloc1.data.server.status, 'allocated');
      assert.equal(alloc1.data.server.allocated_match_token, 'match-tok-001');
      assert.equal(alloc1.data.server.allocation?.map_id, 'skirmish_valley');
      assert.equal(alloc1.data.server.relay_uplink_addr, null);

      // Dedicated server polling heartbeat (without overriding status) receives its newly assigned allocation
      const pollAllocHb = await httpRequestJson<DedicatedServerEntry>(
        apiUrl,
        'POST',
        '/api/v1/servers/heartbeat',
        { server_id: 'ds-alloc-eu', server_token: euSrv.data.server_token }
      );
      assert.equal(pollAllocHb.statusCode, 200);
      assert.equal(pollAllocHb.data.status, 'allocated');
      assert.equal(pollAllocHb.data.allocation?.match_token, 'match-tok-001');

      // Allocate mutator match (order-independent mutator set) -> picks ds-alloc-mut
      const allocMut = await httpRequestJson<{
        allocated: boolean;
        server: DedicatedServerEntry;
      }>(apiUrl, 'POST', '/api/v1/servers/allocate', {
        mod_id: 'rust-ra2',
        active_mutators: ['fast_build', 'instagib'],
        required_players: 2,
        match_token: 'match-tok-mut',
      });
      assert.equal(allocMut.statusCode, 200);
      assert.equal(allocMut.data.server.server_id, 'ds-alloc-mut');

      // Both servers are now allocated -> third allocation returns 404 { allocated: false }
      const noIdle = await httpRequestJson<{
        allocated: boolean;
        error: string;
      }>(apiUrl, 'POST', '/api/v1/servers/allocate', {
        mod_id: 'rust-ra2',
        required_players: 2,
      });
      assert.equal(noIdle.statusCode, 404);
      assert.equal(noIdle.data.allocated, false);

      // Recycle ds-alloc-eu via heartbeat with status: 'idle' + completed_match_token
      const recycleHb = await httpRequestJson<DedicatedServerEntry>(
        apiUrl,
        'POST',
        '/api/v1/servers/heartbeat',
        {
          server_id: 'ds-alloc-eu',
          status: 'idle',
          current_players: 0,
          active_tick: 0,
          completed_match_token: 'match-tok-001',
        },
        { 'X-Server-Token': euSrv.data.server_token }
      );
      assert.equal(recycleHb.statusCode, 200);
      assert.equal(recycleHb.data.status, 'idle');
      assert.equal(recycleHb.data.matches_hosted, 1);
      assert.equal(recycleHb.data.allocated_match_token, null);
      assert.equal(recycleHb.data.allocated_lobby_id, null);
      assert.equal(recycleHb.data.allocation, null);

      // Duplicate retry of the same completed_match_token is idempotent
      const dupRecycleHb = await httpRequestJson<DedicatedServerEntry>(
        apiUrl,
        'POST',
        '/api/v1/servers/heartbeat',
        {
          server_id: 'ds-alloc-eu',
          status: 'idle',
          completed_match_token: 'match-tok-001',
        },
        { 'X-Server-Token': euSrv.data.server_token }
      );
      assert.equal(dupRecycleHb.data.matches_hosted, 1);

      // Recycled server is immediately allocatable again!
      const allocAfterRecycle = await httpRequestJson<{
        allocated: boolean;
        server: DedicatedServerEntry;
      }>(apiUrl, 'POST', '/api/v1/servers/allocate', {
        mod_id: 'rust-ra2',
        required_players: 2,
        match_token: 'match-tok-002',
      });
      assert.equal(allocAfterRecycle.statusCode, 200);
      assert.equal(allocAfterRecycle.data.server.server_id, 'ds-alloc-eu');
      assert.equal(allocAfterRecycle.data.server.matches_hosted, 1);
      assert.equal(
        allocAfterRecycle.data.server.allocated_match_token,
        'match-tok-002'
      );
    } finally {
      await app.registry.closeAllRelayRooms();
      await new Promise<void>((resolve) => app.server.close(() => resolve()));
    }
  });

  it('assigns neutral dedicated server in Ranked Automatch when idle and falls back to P2P/Relay when none available', async () => {
    const app = createAppServer({
      ttlSecs: 30,
      relayMuxPort: 0,
      rateLimitMax: 500,
    });
    await new Promise<void>((resolve) =>
      app.server.listen(0, '127.0.0.1', () => resolve())
    );
    const addr = app.server.address();
    if (!addr || typeof addr === 'string') {
      throw new Error('Unexpected dedicated automatch test server address');
    }
    const mmUrl = `http://127.0.0.1:${addr.port}`;

    try {
      // Register 1 idle authoritative dedicated server
      const dsReg = await httpRequestJson<DedicatedServerRegisterResponse>(
        mmUrl,
        'POST',
        '/api/v1/servers/register',
        {
          server_id: 'ds-ranked-eu-01',
          display_name: 'EU Ranked Dedicated #1',
          tcp_addr: '198.51.100.77:7950',
          region: 'eu-central',
          mod_id: 'rust-ra2',
          mod_version: '0.1.0',
          max_players: 4,
        }
      );
      assert.equal(dsReg.statusCode, 201);

      const p1 = createPlayerIdentityProfile('RankedCmdr1', 'd1'.repeat(32));
      const p2 = createPlayerIdentityProfile('RankedCmdr2', 'd2'.repeat(32));

      const enq1 = await httpRequestJson<AutomatchPollResponse>(
        mmUrl,
        'POST',
        '/api/v1/matchmaking/enqueue',
        {
          player_id: p1.player_id,
          display_name: p1.display_name,
          public_token: p1.public_token,
          mod_id: 'rust-ra2',
          mode: '1v1',
          preferred_faction: 'Allied',
          host_addr: '127.0.0.1:7960',
        }
      );
      assert.equal(enq1.data.status, 'searching');

      const enq2 = await httpRequestJson<AutomatchPollResponse>(
        mmUrl,
        'POST',
        '/api/v1/matchmaking/enqueue',
        {
          player_id: p2.player_id,
          display_name: p2.display_name,
          public_token: p2.public_token,
          mod_id: 'rust-ra2',
          mode: '1v1',
          preferred_faction: 'Soviet',
        }
      );
      assert.equal(enq2.data.status, 'matched');

      const poll1 = await httpRequestJson<AutomatchPollResponse>(
        mmUrl,
        'GET',
        `/api/v1/matchmaking/poll?ticket_id=${enq1.data.ticket_id}`
      );
      assert.equal(poll1.data.status, 'matched');
      const a1 = poll1.data.assignment!;
      const a2 = enq2.data.assignment!;

      // Neutral dedicated server assigned: BOTH players have is_host === false!
      assert.equal(a1.authoritative_dedicated, true);
      assert.equal(a2.authoritative_dedicated, true);
      assert.equal(a1.is_host, false);
      assert.equal(a2.is_host, false);
      assert.equal(a1.dedicated_server_id, 'ds-ranked-eu-01');
      assert.equal(a2.dedicated_server_id, 'ds-ranked-eu-01');
      assert.equal(a1.dedicated_addr, '198.51.100.77:7950');
      assert.equal(a1.host_player_id, 'ds-ranked-eu-01');
      assert.equal(a1.candidate_addrs[0], '198.51.100.77:7950');
      assert.equal(a1.candidate_addrs[1], dsReg.data.relay_addr);
      assert.ok(a1.candidate_addrs.length >= 3);

      // Now ds-ranked-eu-01 is busy ('allocated'): next 1v1 pair must gracefully fall back to P2P/Relay!
      const p3 = createPlayerIdentityProfile('FallbackCmdr3', 'd3'.repeat(32));
      const p4 = createPlayerIdentityProfile('FallbackCmdr4', 'd4'.repeat(32));

      const enq3 = await httpRequestJson<AutomatchPollResponse>(
        mmUrl,
        'POST',
        '/api/v1/matchmaking/enqueue',
        {
          player_id: p3.player_id,
          display_name: p3.display_name,
          public_token: p3.public_token,
          mod_id: 'rust-ra2',
          mode: '1v1',
          preferred_faction: 'Allied',
          host_addr: '127.0.0.1:7961',
        }
      );
      const enq4 = await httpRequestJson<AutomatchPollResponse>(
        mmUrl,
        'POST',
        '/api/v1/matchmaking/enqueue',
        {
          player_id: p4.player_id,
          display_name: p4.display_name,
          public_token: p4.public_token,
          mod_id: 'rust-ra2',
          mode: '1v1',
          preferred_faction: 'Yuri',
        }
      );
      const poll3 = await httpRequestJson<AutomatchPollResponse>(
        mmUrl,
        'GET',
        `/api/v1/matchmaking/poll?ticket_id=${enq3.data.ticket_id}`
      );
      const fb3 = poll3.data.assignment!;
      const fb4 = enq4.data.assignment!;

      assert.equal(fb3.authoritative_dedicated, false);
      assert.equal(fb4.authoritative_dedicated, false);
      assert.equal(fb3.dedicated_server_id, null);
      assert.equal(fb3.dedicated_addr, null);
      assert.equal(fb3.is_host, true);
      assert.equal(fb4.is_host, false);
      assert.equal(fb3.host_player_id, p3.player_id);
      assert.equal(fb3.candidate_addrs[0], '127.0.0.1:7961');
      assert.ok(fb3.candidate_addrs[1].includes('#mux:rm-'));
    } finally {
      await app.registry.closeAllRelayRooms();
      await new Promise<void>((resolve) => app.server.close(() => resolve()));
    }
  });

  it('updates commander Glicko-2 ratings from dedicated-server signed match reports via /api/v1/matches and /api/v1/matches/signed', async () => {
    const app = createAppServer({
      ttlSecs: 30,
      relayMuxPort: 0,
      rateLimitMax: 500,
    });
    await new Promise<void>((resolve) =>
      app.server.listen(0, '127.0.0.1', () => resolve())
    );
    const addr = app.server.address();
    if (!addr || typeof addr === 'string') {
      throw new Error('Unexpected dedicated match report test server address');
    }
    const apiUrl = `http://127.0.0.1:${addr.port}`;

    try {
      const cmdrAllied = createPlayerIdentityProfile(
        'CmdrMirage',
        'e1'.repeat(32)
      );
      const cmdrSoviet = createPlayerIdentityProfile(
        'CmdrApoc',
        'e2'.repeat(32)
      );
      // Two distinct dedicated server containers sharing display_name 'rts-dedicated-server'
      const dsIdentityA = createPlayerIdentityProfile(
        'rts-dedicated-server',
        'f1'.repeat(32)
      );
      const dsIdentityB = createPlayerIdentityProfile(
        'rts-dedicated-server',
        'f2'.repeat(32)
      );

      for (const c of [cmdrAllied, cmdrSoviet]) {
        const reg = await httpRequestJson<{ registered: boolean }>(
          apiUrl,
          'POST',
          '/api/v1/players/register',
          {
            player_id: c.player_id,
            display_name: c.display_name,
            public_token: c.public_token,
          }
        );
        assert.equal(reg.statusCode, 201);
      }

      // Dedicated server #1 submits Match 1 via POST /api/v1/matches with real player_id values
      const match1Signed = signMatchReport(
        {
          lobby_id: 'lobby-ds-101',
          lobby_name: 'Authoritative Dedicated 1v1 #1',
          mod_id: 'rust-ra2',
          map_id: 'ra2_mp_01_urban_clash',
          game_mode: '1v1',
          players: [
            {
              name: 'Slot1_Alias',
              player_id: cmdrAllied.player_id,
              faction: 'Allied',
              team: 1,
              is_winner: true,
            },
            {
              name: 'Slot2_Alias',
              player_id: cmdrSoviet.player_id,
              faction: 'Soviet',
              team: 2,
              is_winner: false,
            },
          ],
          winner_player: cmdrAllied.display_name,
          winner_faction: 'Allied',
          final_tick: 8400,
          final_state_hash: '0x123456789abcdef0',
          total_commands: 512,
          desync_detected: false,
        },
        dsIdentityA,
        1_700_000_500_000
      );

      const m1Res = await httpRequestJson<MatchRecord>(
        apiUrl,
        'POST',
        '/api/v1/matches',
        match1Signed
      );
      assert.equal(m1Res.statusCode, 201);
      assert.equal(m1Res.data.players[0].player_id, cmdrAllied.player_id);
      assert.equal(m1Res.data.players[1].player_id, cmdrSoviet.player_id);

      const lb1 = await httpRequestJson<LeaderboardResponse>(
        apiUrl,
        'GET',
        '/api/v1/leaderboard'
      );
      const mirage1 = lb1.data.leaderboard.find(
        (r) => r.player_id === cmdrAllied.player_id
      );
      const apoc1 = lb1.data.leaderboard.find(
        (r) => r.player_id === cmdrSoviet.player_id
      );
      assert.ok(mirage1 && apoc1);
      assert.equal(mirage1.player_name, 'CmdrMirage');
      assert.equal(apoc1.player_name, 'CmdrApoc');
      assert.ok(mirage1.rating > 1600);
      assert.ok(apoc1.rating < 1400);
      assert.ok(
        !lb1.data.leaderboard.some(
          (r) => r.player_name === 'rts-dedicated-server'
        ),
        'Neutral dedicated server submitter must not appear as a player on the leaderboard'
      );

      // Dedicated server #2 submits Match 2 via POST /api/v1/matches/signed
      const match2Signed = signMatchReport(
        {
          lobby_id: 'lobby-ds-102',
          lobby_name: 'Authoritative Dedicated 1v1 #2',
          mod_id: 'rust-ra2',
          map_id: 'ra2_mp_01_urban_clash',
          game_mode: '1v1',
          players: [
            {
              name: cmdrSoviet.display_name,
              player_id: cmdrSoviet.player_id,
              faction: 'Soviet',
              team: 1,
              is_winner: true,
            },
            {
              name: cmdrAllied.display_name,
              player_id: cmdrAllied.player_id,
              faction: 'Allied',
              team: 2,
              is_winner: false,
            },
          ],
          winner_player: cmdrSoviet.display_name,
          winner_faction: 'Soviet',
          final_tick: 9200,
          final_state_hash: '0xfedcba9876543210',
          total_commands: 630,
          desync_detected: false,
        },
        dsIdentityB,
        1_700_000_600_000
      );

      const m2Res = await httpRequestJson<MatchRecord>(
        apiUrl,
        'POST',
        '/api/v1/matches/signed',
        match2Signed
      );
      assert.equal(m2Res.statusCode, 201);

      // Replay protection also enforced on /api/v1/matches/signed
      const replayM2 = await httpRequestJson<{ error: string }>(
        apiUrl,
        'POST',
        '/api/v1/matches/signed',
        match2Signed
      );
      assert.equal(replayM2.statusCode, 409);

      const lb2 = await httpRequestJson<LeaderboardResponse>(
        apiUrl,
        'GET',
        '/api/v1/leaderboard'
      );
      const mirage2 = lb2.data.leaderboard.find(
        (r) => r.player_id === cmdrAllied.player_id
      );
      const apoc2 = lb2.data.leaderboard.find(
        (r) => r.player_id === cmdrSoviet.player_id
      );
      assert.ok(mirage2 && apoc2);
      assert.equal(mirage2.wins, 1);
      assert.equal(mirage2.losses, 1);
      assert.equal(apoc2.wins, 1);
      assert.equal(apoc2.losses, 1);
      assert.ok(apoc2.rating > apoc1.rating);
      assert.ok(mirage2.rating < mirage1.rating);
    } finally {
      await app.registry.closeAllRelayRooms();
      await new Promise<void>((resolve) => app.server.close(() => resolve()));
    }
  });
});
