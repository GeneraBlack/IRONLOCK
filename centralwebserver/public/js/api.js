async function requestJson(url, init) {
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
            const data = (await response.json());
            if (data.error)
                errText = data.error;
        }
        catch {
            // ignore json parse failure
        }
        throw new Error(errText);
    }
    return (await response.json());
}
export async function fetchStats() {
    return requestJson('/api/stats');
}
export async function fetchProjectInfo() {
    return requestJson('/api/project-info');
}
export async function fetchGameDatabase() {
    return requestJson('/api/game-database');
}
export async function fetchLeaderboard() {
    const res = await requestJson('/api/leaderboard');
    return res.leaderboard;
}
export async function fetchLobbies(params = {}) {
    const query = new URLSearchParams();
    if (params.modId)
        query.set('mod_id', params.modId);
    if (params.onlyOpen)
        query.set('only_open', 'true');
    if (params.status)
        query.set('status', params.status);
    if (params.search)
        query.set('search', params.search);
    const qs = query.toString();
    const res = await requestJson(`/api/lobbies${qs ? `?${qs}` : ''}`);
    return res.lobbies;
}
export async function registerLobby(ann) {
    return requestJson('/api/lobbies', {
        method: 'POST',
        body: JSON.stringify(ann),
    });
}
export async function sendLobbyHeartbeat(lobbyId, hb, hostToken) {
    return requestJson(`/api/lobbies/${encodeURIComponent(lobbyId)}/heartbeat`, {
        method: 'POST',
        headers: hostToken ? { 'X-Host-Token': hostToken } : {},
        body: JSON.stringify(hb),
    });
}
export async function deleteLobby(lobbyId, hostToken) {
    const res = await requestJson(`/api/lobbies/${encodeURIComponent(lobbyId)}`, {
        method: 'DELETE',
        headers: hostToken ? { 'X-Host-Token': hostToken } : {},
    });
    return res.deleted;
}
export async function fetchMatches(modId, search) {
    const query = new URLSearchParams();
    if (modId)
        query.set('mod_id', modId);
    if (search)
        query.set('search', search);
    const qs = query.toString();
    const res = await requestJson(`/api/matches${qs ? `?${qs}` : ''}`);
    return res.matches;
}
export async function createMatchRecord(req) {
    return requestJson('/api/matches', {
        method: 'POST',
        body: JSON.stringify(req),
    });
}
