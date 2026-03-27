import type { GameAction, GameState, Phase, WaitingFor } from "../adapter/types";
import { getPlayerId } from "../hooks/usePlayerId";

/**
 * Determines whether the current priority window should be auto-passed.
 * Uses legal actions from the engine (like Alchemy/MTGA) for accurate decisions.
 *
 * Rules (in order):
 * 1. Full control mode disables auto-pass
 * 2. Only auto-pass Priority prompts for the local player
 * 3. If stack is empty, respect phase stops (initial priority in that phase)
 * 4. Auto-pass if no meaningful actions exist (only PassPriority available)
 * 5. MTGA-style: auto-pass when own spell/ability is on top of the stack
 */
export function shouldAutoPass(
  state: GameState,
  waitingFor: WaitingFor,
  phaseStops: Phase[],
  fullControl: boolean,
  legalActions: GameAction[],
): boolean {
  if (fullControl) return false;
  if (waitingFor.type !== "Priority") return false;
  if (waitingFor.data.player !== getPlayerId()) return false;

  // Don't auto-pass an invalid/empty game state (e.g. no cards loaded yet)
  if (state.players.length === 0 || Object.keys(state.objects).length === 0) return false;

  // Phase stops only gate initial priority (empty stack)
  if (state.stack.length === 0 && phaseStops.includes(state.phase)) return false;

  // If legal actions haven't been computed yet, don't auto-pass.
  // The engine always includes at least PassPriority when a player has priority,
  // so an empty array means the actions haven't arrived (e.g. P2P latency).
  if (legalActions.length === 0) return false;

  // If no meaningful actions exist beyond PassPriority, auto-pass.
  // Land-based activated abilities (e.g. Abandoned Air Temple's {3}{W},{T}: put counters)
  // are not considered meaningful — they're utility abilities the player would only use
  // deliberately on their own turn, not in response to opponent actions. Full control
  // mode (checked above) overrides this for players who want to respond with land abilities.
  const hasMeaningfulAction = legalActions.some((a) => {
    if (a.type === "PassPriority") return false;
    if (a.type === "ActivateAbility") {
      const obj = state.objects[a.data.source_id];
      if (obj?.card_types.core_types.includes("Land")) return false;
    }
    return true;
  });
  if (!hasMeaningfulAction) return true;

  // MTGA-style: auto-pass when our own spell/ability is on top of the stack.
  // The player almost never wants to respond to their own spell (e.g. counter
  // their own creature). Full control mode disables this (checked above).
  if (state.stack.length > 0) {
    const topEntry = state.stack[state.stack.length - 1];
    if (topEntry.controller === getPlayerId()) return true;
  }

  return false;
}
