import { RelayPortRange } from './relay';
import { createCentralWebServer } from './server';

function parseCliArgs(argv: string[]): {
  host: string;
  port: number;
  ttlSecs: number;
  publicHost?: string;
  relayPortRange?: RelayPortRange;
} {
  let host = process.env.HOST ?? '0.0.0.0';
  let port = Number(process.env.PORT ?? 8080);
  let ttlSecs = Number(process.env.TTL_SECS ?? 45);
  let publicHost = process.env.PUBLIC_HOST ?? process.env.PUBLIC_DOMAIN;
  let relayPortRange: RelayPortRange | undefined = {
    start: Number(process.env.RELAY_PORT_START) || 27900,
    end: Number(process.env.RELAY_PORT_END) || 27999,
  };
  if (process.env.RELAY_PORT_RANGE) {
    const [s, e] = process.env.RELAY_PORT_RANGE.split('-').map((v) =>
      Number(v.trim())
    );
    if (s > 0 && e >= s) {
      relayPortRange = { start: s, end: e };
    }
  }

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--port' && argv[i + 1]) {
      port = Number(argv[++i]) || 8080;
    } else if (arg === '--host' && argv[i + 1]) {
      host = argv[++i];
    } else if (arg === '--public-host' && argv[i + 1]) {
      publicHost = argv[++i];
    } else if (arg === '--relay-port-range' && argv[i + 1]) {
      const raw = argv[++i];
      const [s, e] = raw.split('-').map((v) => Number(v.trim()));
      if (s > 0 && e >= s) {
        relayPortRange = { start: s, end: e };
      }
    } else if (arg === '--bind' && argv[i + 1]) {
      const bindVal = argv[++i];
      const lastColon = bindVal.lastIndexOf(':');
      if (lastColon !== -1) {
        host = bindVal.slice(0, lastColon);
        port = Number(bindVal.slice(lastColon + 1)) || 8080;
      }
    } else if (arg === '--ttl-secs' && argv[i + 1]) {
      ttlSecs = Number(argv[++i]) || 45;
    }
  }

  return { host, port, ttlSecs, publicHost, relayPortRange };
}

function main(): void {
  const { host, port, ttlSecs, publicHost, relayPortRange } = parseCliArgs(
    process.argv.slice(2)
  );
  const { server } = createCentralWebServer({
    ttlSecs,
    publicHost,
    relayPortRange,
  });

  server.listen(port, host, () => {
    const displayHost = host === '0.0.0.0' ? '127.0.0.1' : host;
    console.log('==============================================================================');
    console.log('  IRONLOCK // RUST-RTS — OFFICIAL PORTAL & CENTRAL MULTIPLAYER REGISTRY');
    console.log('  Live Production Domain     : https://rust-rts.bls-isp.net/');
    console.log(`  Lokaler Endpoint (Browser) : http://${displayHost}:${port}/`);
    console.log(`  Lobby-REST-API (rts-client): http://${displayHost}:${port}/api/lobbies`);
    console.log(`  Match- & Replay-Register   : http://${displayHost}:${port}/api/matches`);
    if (relayPortRange) {
      console.log(`  NAT Relay-Tunnel Ports     : TCP ${relayPortRange.start}-${relayPortRange.end}`);
    }
    console.log(`  Heartbeat-TTL              : ${ttlSecs} Sekunden`);
    console.log('==============================================================================');
  });
}

main();
