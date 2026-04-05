import type { PlayerId } from "../adapter/types";
import { PLAYER_ID } from "../constants/game";
import { useGameStore } from "../stores/gameStore";
import { useMultiplayerStore } from "../stores/multiplayerStore";

/** React hook: returns the current player's game-assigned ID (0 or 1). Falls back to PLAYER_ID (0) for AI/local mode. */
export function usePlayerId(): PlayerId {
  return useMultiplayerStore((s) => s.activePlayerId) ?? PLAYER_ID;
}

/** Non-React getter for use in plain functions (autoPass, gameLoopController). */
export function getPlayerId(): PlayerId {
  return useMultiplayerStore.getState().activePlayerId ?? PLAYER_ID;
}

function waitingPlayer(waitingFor: ReturnType<typeof useGameStore.getState>["waitingFor"]): PlayerId | null {
  if (!waitingFor || waitingFor.type === "GameOver") return null;
  return "player" in waitingFor.data ? waitingFor.data.player : null;
}

export function usePerspectivePlayerId(): PlayerId {
  const playerId = usePlayerId();
  const gameState = useGameStore((s) => s.gameState);
  if (!gameState) return playerId;
  return gameState.turn_decision_controller === playerId ? gameState.active_player : playerId;
}

export function useCanActForWaitingState(): boolean {
  const playerId = usePlayerId();
  const gameState = useGameStore((s) => s.gameState);
  const waitingFor = useGameStore((s) => s.waitingFor);
  const semanticPlayer = waitingPlayer(waitingFor);
  if (!gameState || semanticPlayer == null) return false;
  if (semanticPlayer === playerId) return true;
  return gameState.turn_decision_controller === playerId && semanticPlayer === gameState.active_player;
}
