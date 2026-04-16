/**
 * Persistent session token storage for P2P games.
 *
 * Tokens are issued by the host on `game_setup` / `reconnect_ack` and consumed
 * by the guest on auto-reconnect. Persisting to IndexedDB (not sessionStorage)
 * means a guest whose tab crashed or was accidentally closed can reopen
 * and still rejoin their original seat — the host recognizes the token and
 * rebinds the PlayerId through `handleReconnect`.
 *
 * Pre-game tokens (issued on lobby join but before `game_setup`) are
 * intentionally NOT persisted — a guest who drops during the lobby must
 * rejoin fresh.
 */

import { createStore, del, get, set } from "idb-keyval";

const STORAGE_PREFIX = "phase-p2p-session:";
/**
 * How long a persisted guest token is valid. Sized for host-resume
 * realism: a host who crashed, reopened the tab, and dialed back in on
 * the same room code within 4 hours can still rejoin guests. Beyond
 * that window, the token is considered stale and the guest rejoins
 * fresh (new seat if lobby, rejected if mid-game).
 */
const SESSION_TTL_MS = 4 * 60 * 60 * 1000;

export interface P2PSessionData {
  hostPeerId: string;
  playerToken: string;
  playerId: number;
  timestamp: number;
}

let _store: ReturnType<typeof createStore> | undefined;

function getSessionStore(): ReturnType<typeof createStore> {
  if (!_store) {
    _store = createStore("phase-p2p-session", "phase-p2p-session");
  }
  return _store;
}

function storageKey(hostPeerId: string): string {
  return STORAGE_PREFIX + hostPeerId;
}

function isFresh(session: P2PSessionData): boolean {
  return Date.now() - session.timestamp < SESSION_TTL_MS;
}

export async function saveP2PSession(
  hostPeerId: string,
  data: { playerToken: string; playerId: number },
): Promise<void> {
  const session: P2PSessionData = {
    hostPeerId,
    playerToken: data.playerToken,
    playerId: data.playerId,
    timestamp: Date.now(),
  };
  try {
    await set(storageKey(hostPeerId), session, getSessionStore());
  } catch (err) {
    console.warn("[p2pSession] IDB write failed:", err);
  }
}

export async function loadP2PSession(hostPeerId: string): Promise<P2PSessionData | null> {
  try {
    const session = await get<P2PSessionData>(storageKey(hostPeerId), getSessionStore());
    if (!session) return null;
    if (!isFresh(session)) {
      await clearP2PSession(hostPeerId);
      return null;
    }
    return session;
  } catch {
    return null;
  }
}

export async function clearP2PSession(hostPeerId: string): Promise<void> {
  try {
    await del(storageKey(hostPeerId), getSessionStore());
  } catch { /* best-effort */ }
}
