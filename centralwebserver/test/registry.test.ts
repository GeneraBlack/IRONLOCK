import * as assert from 'node:assert/strict';
import * as http from 'node:http';
import * as net from 'node:net';
import { after, before, describe, it } from 'node:test';
import { createCentralWebServer } from '../src/server';
import {
  LeaderboardResponse,
  LobbyEntry,
  LobbyListResponse,
  MatchListResponse,
  MatchRecord,
  ProjectOverview,
  RegistryStats,
} from '../src/types';

function httpRequestJson<T>(
  baseUrl: string,
  method: string,
  path: string,
  body?: unknown,
  extraHeaders?: Record<string, string>
): Promise<{ statusCode: number; headers: http.IncomingHttpHeaders; data: T }> {
  return new Promise((resolve, reject) => {
    const url = new URL(path, baseUrl);
    const payload = body !== undefined ? JSON.stringify(body) : undefined;
    const req = http.request(
      url,
      {
        method,
        headers: {
          'Content-Type': 'application/json',
          ...(payload
            ? { 'Content-Length': String(Buffer.byteLength(payload)) }
            : {}),
          ...(extraHeaders ?? {}),
        },
      },
      (res) => {
        const chunks: Buffer[] = [];
        res.on('data', (c: Buffer) => chunks.push(c));
        res.on('end', () => {
          const raw = Buffer.concat(chunks).toString('utf-8');
          try {
            const parsed = JSON.parse(raw) as T;
            resolve({
              statusCode: res.statusCode ?? 500,
              headers: res.headers,
              data: parsed,
            });
          } catch (err) {
            reject(
              new Error(`Failed to parse JSON response: ${raw} (${String(err)})`)
            );
          }
        });
      }
    );
    req.on('error', reject);
    if (payload) req.write(payload);
    req.end();
  });
}

function httpGetText(
  baseUrl: string,
  path: string
): Promise<{ statusCode: number; headers: http.IncomingHttpHeaders; text: string }> {
  return new Promise((resolve, reject) => {
    const url = new URL(path, baseUrl);
    http
      .get(url, (res) => {
        const chunks: Buffer[] = [];
        res.on('data', (c: Buffer) => chunks.push(c));
        res.on('end', () => {
          resolve({
            statusCode: res.statusCode ?? 500,
            headers: res.headers,
            text: Buffer.concat(chunks).toString('utf-8'),
          });
        });
      })
      .on('error', reject);
  });
}

describe('Central Webserver & Multiplayer Lobby Registry', () => {
  let server: http.Server;
  let baseUrl = '';

  before(async () => {
    const created = createCentralWebServer({ ttlSecs: 30 });
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

  it('serves the project homepage and static assets with explicit Content-Length', async () => {
    const home = await httpGetText(baseUrl, '/');
    assert.equal(home.statusCode, 200);
    assert.ok(home.headers['content-length'], 'Content-Length header must be set');
    assert.ok(
      home.text.includes('rust-rts.bls-isp.net'),
      'Homepage HTML should include rust-rts.bls-isp.net branding'
    );
    assert.ok(
      home.text.includes('IRONLOCK'),
      'Homepage HTML should include IRONLOCK // RUST-RTS brand identity'
    );

    const favicon = await httpGetText(baseUrl, '/favicon.svg');
    assert.equal(favicon.statusCode, 200);
    assert.ok(favicon.text.includes('<svg'));

    const css = await httpGetText(baseUrl, '/styles.css');
    assert.equal(css.statusCode, 200);
    assert.ok(css.text.includes('--accent-allied'));

    const js = await httpGetText(baseUrl, '/js/app.js');
    assert.equal(js.statusCode, 200);
    assert.ok(js.text.includes('refreshAllData'));

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

    // Ensure every data-i18n key in index.html and every t(state.locale, 'key') in app.js exists in i18n.js
    const htmlKeys = [...home.text.matchAll(/data-i18n="([^"]+)"/g)].map((m) => m[1]);
    const jsKeys = [...js.text.matchAll(/t\(state\.locale,\s*'([^']+)'\)/g)].map((m) => m[1]);
    for (const key of [...new Set([...htmlKeys, ...jsKeys])]) {
      const occurrences = i18nJs.text.split(`${key}:`).length - 1;
      assert.ok(
        occurrences >= 2,
        `Key '${key}' must be defined in both DE and EN in i18n.js (found ${occurrences})`
      );
    }

    const cameo = await httpGetText(baseUrl, '/assets/cameos/gtnkicon.webp');
    assert.equal(cameo.statusCode, 200);
    assert.equal(cameo.headers['content-type'], 'image/webp');
    assert.ok(Number(cameo.headers['content-length']) > 500);

    const screen = await httpGetText(baseUrl, '/assets/screens/country_usa.webp');
    assert.equal(screen.statusCode, 200);
    assert.equal(screen.headers['content-type'], 'image/webp');
    assert.ok(Number(screen.headers['content-length']) > 5000);

    const glbModel = await httpGetText(baseUrl, '/assets/models/grizzly_tank.glb');
    assert.equal(glbModel.statusCode, 200);
    assert.equal(glbModel.headers['content-type'], 'model/gltf-binary');
    assert.ok(Number(glbModel.headers['content-length']) > 10000);
    assert.ok(glbModel.text.startsWith('glTF'), 'Binary GLB must start with glTF magic header');

    const mapWebp = await httpGetText(baseUrl, '/assets/maps/mp_01_offense_defense.webp');
    assert.equal(mapWebp.statusCode, 200);
    assert.equal(mapWebp.headers['content-type'], 'image/webp');
    assert.ok(Number(mapWebp.headers['content-length']) > 10000);
  });

  it('serves the full RA2 & Yuri Revenge game database and commander leaderboard', async () => {
    const dbRes = await httpRequestJson<{
      factions: unknown[];
      countries: unknown[];
      units: Array<{ id: string; mesh_vertices?: number; mesh_triangles?: number; visual: { model: string } }>;
      buildings: Array<{ id: string; mesh_vertices?: number; mesh_triangles?: number; visual: { model: string } }>;
      weapons: unknown[];
      maps: Array<{ id: string; theater?: string; preview_webp?: string; spawns: unknown[]; ores: unknown[] }>;
    }>(baseUrl, 'GET', '/api/game-database');
    assert.equal(dbRes.statusCode, 200);
    assert.equal(dbRes.data.factions.length, 3);
    assert.equal(dbRes.data.countries.length, 10);
    assert.ok(dbRes.data.units.length >= 70);
    assert.equal(dbRes.data.buildings.length, 76);
    assert.ok(dbRes.data.weapons.length >= 58);
    assert.ok(dbRes.data.maps.length >= 50);
    assert.ok(
      dbRes.data.units.every((u) => (u.mesh_vertices ?? 0) > 0 && (u.mesh_triangles ?? 0) > 0),
      'Every unit must have real 3D GLB mesh vertex and triangle counts'
    );
    assert.ok(
      dbRes.data.buildings.every((b) => (b.mesh_vertices ?? 0) > 0 && (b.mesh_triangles ?? 0) > 0),
      'Every building must have real 3D GLB mesh vertex and triangle counts'
    );
    assert.ok(
      dbRes.data.maps.every((m) => Boolean(m.preview_webp) && Boolean(m.theater)),
      'Every map must have a rendered isometric WebP preview and theater metadata'
    );

    const lbRes = await httpRequestJson<LeaderboardResponse>(
      baseUrl,
      'GET',
      '/api/leaderboard'
    );
    assert.equal(lbRes.statusCode, 200);
    assert.ok(lbRes.data.leaderboard.length >= 3);
    assert.equal(lbRes.data.leaderboard[0].rank, 1);
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

  it('registers, lists, filters, heartbeats, and deletes lobbies compatibly with rts-network', async () => {
    // 1. Register Lobby with 0.0.0.0:7878 -> should substitute peer IP 127.0.0.1:7878
    const createRes = await httpRequestJson<LobbyEntry>(
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

    // 2. List open lobbies for rust-ra2
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

    // 3. Send heartbeat updating players to 2/2 -> becomes full
    const hbRes = await httpRequestJson<LobbyEntry>(
      baseUrl,
      'POST',
      `/api/lobbies/${createRes.data.lobby_id}/heartbeat`,
      {
        current_players: 2,
        max_players: 2,
      }
    );
    assert.equal(hbRes.statusCode, 200);
    assert.equal(hbRes.data.status, 'full');
    assert.equal(hbRes.data.current_players, 2);

    // 4. Filter only_open=true should now return 0
    const openOnlyRes = await httpRequestJson<LobbyListResponse>(
      baseUrl,
      'GET',
      '/api/lobbies?only_open=true'
    );
    assert.equal(openOnlyRes.data.lobbies.length, 0);

    // 5. Check /api/stats
    const statsRes = await httpRequestJson<RegistryStats>(
      baseUrl,
      'GET',
      '/api/stats'
    );
    assert.equal(statsRes.statusCode, 200);
    assert.equal(statsRes.data.public_domain, 'rust-rts.bls-isp.net');
    assert.equal(statsRes.data.active_lobbies, 1);
    assert.equal(statsRes.data.full_lobbies, 1);
    assert.equal(statsRes.data.players_online, 2);

    // 6. Delete Lobby
    const delRes = await httpRequestJson<{ deleted: boolean }>(
      baseUrl,
      'DELETE',
      `/api/lobbies/${createRes.data.lobby_id}`
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

  it('lists and records deterministic lockstep matches in /api/matches', async () => {
    const initial = await httpRequestJson<MatchListResponse>(
      baseUrl,
      'GET',
      '/api/matches'
    );
    assert.equal(initial.statusCode, 200);
    const beforeCount = initial.data.matches.length;
    assert.ok(beforeCount >= 3);

    const postMatch = await httpRequestJson<MatchRecord>(
      baseUrl,
      'POST',
      '/api/matches',
      {
        lobby_name: 'Championship Finals [Allied vs Soviet]',
        mod_id: 'rust-ra2',
        map_id: 'river_crossing',
        map_name: 'River Crossing',
        winner_player: 'CommanderGener',
        winner_faction: 'Allied',
        final_tick: 19200,
        final_state_hash: '0x1122334455667788',
      }
    );
    assert.equal(postMatch.statusCode, 201);
    assert.equal(postMatch.data.winner_player, 'CommanderGener');
    assert.equal(postMatch.data.final_state_hash, '0x1122334455667788');

    const filtered = await httpRequestJson<MatchListResponse>(
      baseUrl,
      'GET',
      '/api/matches?search=0x1122334455667788'
    );
    assert.equal(filtered.data.matches.length, 1);
  });

  it('allocates a Reverse-TCP NAT Relay Tunnel Room when allocate_relay is true', async () => {
    const createRes = await httpRequestJson<LobbyEntry>(
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
    assert.ok(createRes.data.relay_uplink_addr, 'relay_uplink_addr must be allocated');
    assert.equal(createRes.data.nat_mode, 'Relay-Tunnel (NAT-Safe)');

    const uplinkPort = Number(createRes.data.relay_uplink_addr!.split(':')[1]);
    const clientPort = Number(createRes.data.relay_addr!.split(':')[1]);

    // Connect host standby uplink and WAN client, verify 'R' wakeup marker + bidirectional payload
    const exchangeResult = await new Promise<string>((resolve, reject) => {
      const hostUplink = net.createConnection({ host: '127.0.0.1', port: uplinkPort });
      let receivedByHost = '';

      hostUplink.on('data', (chunk) => {
        receivedByHost += chunk.toString('utf-8');
        if (receivedByHost === 'RHELLO_LOCKSTEP') {
          hostUplink.write('WELCOME_PEER');
        }
      });
      hostUplink.on('error', reject);

      hostUplink.on('connect', () => {
        const wanClient = net.createConnection({ host: '127.0.0.1', port: clientPort });
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
      `/api/lobbies/${createRes.data.lobby_id}`
    );
  });

  it('handles production domain rust-rts.bls-isp.net and reverse-proxy X-Forwarded-For without NAT loopback collisions', async () => {
    const hostA = await httpRequestJson<LobbyEntry>(
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

    const hostB = await httpRequestJson<LobbyEntry>(
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
      `/api/lobbies/${hostA.data.lobby_id}`
    );
    await httpRequestJson<{ deleted: boolean }>(
      baseUrl,
      'DELETE',
      `/api/lobbies/${hostB.data.lobby_id}`
    );
  });
});
