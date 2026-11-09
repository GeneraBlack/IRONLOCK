import * as net from 'node:net';

export interface RelayPortRange {
  start: number;
  end: number;
}

function listenOnRangeOrEphemeral(
  srv: net.Server,
  range?: RelayPortRange
): Promise<void> {
  if (!range || range.start <= 0 || range.end < range.start) {
    return new Promise<void>((resolve, reject) => {
      srv.once('error', reject);
      srv.listen(0, '0.0.0.0', () => resolve());
    });
  }

  return new Promise<void>((resolve, reject) => {
    let port = range.start;
    const tryNext = (): void => {
      if (port > range.end) {
        srv.once('error', reject);
        srv.listen(0, '0.0.0.0', () => resolve());
        return;
      }
      const currentPort = port++;
      const onError = (err: NodeJS.ErrnoException): void => {
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

export class RelayTunnelRoom {
  public readonly clientPort: number;
  public readonly uplinkPort: number;
  private readonly clientServer: net.Server;
  private readonly uplinkServer: net.Server;
  private readonly waitingHostUplinks: net.Socket[] = [];
  private readonly waitingClients: net.Socket[] = [];
  private readonly activeSockets = new Set<net.Socket>();
  private closed = false;

  private constructor(
    clientServer: net.Server,
    uplinkServer: net.Server,
    clientPort: number,
    uplinkPort: number
  ) {
    this.clientServer = clientServer;
    this.uplinkServer = uplinkServer;
    this.clientPort = clientPort;
    this.uplinkPort = uplinkPort;

    this.uplinkServer.on('connection', (socket) => {
      if (this.closed) {
        socket.destroy();
        return;
      }
      socket.setNoDelay(true);
      this.activeSockets.add(socket);
      socket.on('error', () => {
        this.removeWaiting(this.waitingHostUplinks, socket);
        this.activeSockets.delete(socket);
      });
      socket.on('close', () => {
        this.removeWaiting(this.waitingHostUplinks, socket);
        this.activeSockets.delete(socket);
      });
      this.waitingHostUplinks.push(socket);
      this.tryPair();
    });

    this.clientServer.on('connection', (socket) => {
      if (this.closed) {
        socket.destroy();
        return;
      }
      socket.setNoDelay(true);
      this.activeSockets.add(socket);
      socket.on('error', () => {
        this.removeWaiting(this.waitingClients, socket);
        this.activeSockets.delete(socket);
      });
      socket.on('close', () => {
        this.removeWaiting(this.waitingClients, socket);
        this.activeSockets.delete(socket);
      });
      this.waitingClients.push(socket);
      this.tryPair();
    });
  }

  public static async create(range?: RelayPortRange): Promise<RelayTunnelRoom> {
    const clientServer = net.createServer();
    const uplinkServer = net.createServer();

    const envStart = Number(process.env.RELAY_PORT_START) || 0;
    const envEnd = Number(process.env.RELAY_PORT_END) || 0;
    const effectiveRange =
      range ??
      (envStart > 0 && envEnd >= envStart
        ? { start: envStart, end: envEnd }
        : undefined);

    await listenOnRangeOrEphemeral(clientServer, effectiveRange);
    await listenOnRangeOrEphemeral(uplinkServer, effectiveRange);

    const cAddr = clientServer.address();
    const uAddr = uplinkServer.address();
    if (!cAddr || typeof cAddr === 'string' || !uAddr || typeof uAddr === 'string') {
      clientServer.close();
      uplinkServer.close();
      throw new Error('Failed to bind RelayTunnelRoom TCP ports');
    }

    return new RelayTunnelRoom(clientServer, uplinkServer, cAddr.port, uAddr.port);
  }

  private removeWaiting(list: net.Socket[], target: net.Socket): void {
    const idx = list.indexOf(target);
    if (idx !== -1) {
      list.splice(idx, 1);
    }
  }

  private tryPair(): void {
    while (this.waitingClients.length > 0 && this.waitingHostUplinks.length > 0) {
      const hostSocket = this.waitingHostUplinks.shift()!;
      if (hostSocket.destroyed || !hostSocket.writable) {
        continue;
      }
      const clientSocket = this.waitingClients.shift()!;
      if (clientSocket.destroyed || !clientSocket.writable) {
        this.waitingHostUplinks.unshift(hostSocket);
        continue;
      }

      // Send 1-byte wakeup marker 'R' so the Host's LockstepServer promotes this standby uplink to an active peer
      hostSocket.write(Buffer.from('R'), (err) => {
        if (err) {
          clientSocket.destroy();
          hostSocket.destroy();
          return;
        }
        clientSocket.pipe(hostSocket);
        hostSocket.pipe(clientSocket);
      });
    }
  }

  public close(): void {
    if (this.closed) return;
    this.closed = true;
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
