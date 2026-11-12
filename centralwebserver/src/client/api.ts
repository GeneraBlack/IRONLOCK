import {
  CommanderRankEntry,
  CreateMatchRequest,
  GameDatabase,
  LeaderboardResponse,
  LobbyAnnouncement,
  LobbyEntry,
  LobbyHeartbeatRequest,
  LobbyListResponse,
  LobbyStatus,
  MatchListResponse,
  MatchRecord,
  ProjectOverview,
  RegisteredLobbyResponse,
  RegistryStats,
} from './types.js';

export interface FetchLobbiesParams {
  modId?: string;
  onlyOpen?: boolean;
  status?: LobbyStatus | '';
  search?: string;
}

async function requestJson<T>(url: string, init?: RequestInit): Promise<T> {
  const response = await fetch(url, {
    ...init,
    headers: {
      'Content-Type': 'application/json',
      ...(init?.headers ?? {}),
    },
  });
  if (!response.ok) {
    let errText = `HTTP ${response.status}`;
    try {
      const data = (await response.json()) as { error?: string };
      if (data.error) errText = data.error;
    } catch {
      // ignore json parse failure
    }
    throw new Error(errText);
  }
  return (await response.json()) as T;
}

export async function fetchStats(): Promise<RegistryStats> {
  return requestJson<RegistryStats>('/api/stats');
}

export async function fetchProjectInfo(): Promise<ProjectOverview> {
  return requestJson<ProjectOverview>('/api/project-info');
}

export async function fetchGameDatabase(): Promise<GameDatabase> {
  return requestJson<GameDatabase>('/api/game-database');
}

export async function fetchLeaderboard(): Promise<CommanderRankEntry[]> {
  const res = await requestJson<LeaderboardResponse>('/api/leaderboard');
  return res.leaderboard;
}

export async function fetchLobbies(
  params: FetchLobbiesParams = {}
): Promise<LobbyEntry[]> {
  const query = new URLSearchParams();
  if (params.modId) query.set('mod_id', params.modId);
  if (params.onlyOpen) query.set('only_open', 'true');
  if (params.status) query.set('status', params.status);
  if (params.search) query.set('search', params.search);
  const qs = query.toString();
  const res = await requestJson<LobbyListResponse>(
    `/api/lobbies${qs ? `?${qs}` : ''}`
  );
  return res.lobbies;
}

export async function registerLobby(
  ann: LobbyAnnouncement
): Promise<RegisteredLobbyResponse> {
  return requestJson<RegisteredLobbyResponse>('/api/lobbies', {
    method: 'POST',
    body: JSON.stringify(ann),
  });
}

export async function sendLobbyHeartbeat(
  lobbyId: string,
  hb: LobbyHeartbeatRequest,
  hostToken?: string
): Promise<LobbyEntry> {
  return requestJson<LobbyEntry>(
    `/api/lobbies/${encodeURIComponent(lobbyId)}/heartbeat`,
    {
      method: 'POST',
      headers: hostToken ? { 'X-Host-Token': hostToken } : {},
      body: JSON.stringify(hb),
    }
  );
}

export async function deleteLobby(
  lobbyId: string,
  hostToken?: string
): Promise<boolean> {
  const res = await requestJson<{ deleted: boolean }>(
    `/api/lobbies/${encodeURIComponent(lobbyId)}`,
    {
      method: 'DELETE',
      headers: hostToken ? { 'X-Host-Token': hostToken } : {},
    }
  );
  return res.deleted;
}

export async function fetchMatches(
  modId?: string,
  search?: string
): Promise<MatchRecord[]> {
  const query = new URLSearchParams();
  if (modId) query.set('mod_id', modId);
  if (search) query.set('search', search);
  const qs = query.toString();
  const res = await requestJson<MatchListResponse>(
    `/api/matches${qs ? `?${qs}` : ''}`
  );
  return res.matches;
}

export async function createMatchRecord(
  req: CreateMatchRequest
): Promise<MatchRecord> {
  return requestJson<MatchRecord>('/api/matches', {
    method: 'POST',
    body: JSON.stringify(req),
  });
}
