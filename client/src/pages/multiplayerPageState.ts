import type { GameFormat } from "../adapter/types";
import type { DeckCompatibilityResult } from "../services/deckCompatibility";

export type LiveCheck =
  | { status: "idle" }
  | { status: "checking"; format: GameFormat }
  | { status: "legal"; format: GameFormat }
  | { status: "illegal"; format: GameFormat; reasons: string[] };

/**
 * Classify an engine compatibility result into a `LiveCheck` display state.
 *
 * `selected_format_compatible` is a three-state:
 *   false → illegal (show the engine-provided reasons)
 *   true  → legal
 *   null / undefined → indeterminate; the chip must be suppressed (idle)
 *     rather than claiming "legal". Treating indeterminate as affirmative
 *     would mislead the user whenever the engine's format registry can't
 *     make a decision.
 */
export function classifyCompatResult(
  format: GameFormat,
  result: DeckCompatibilityResult,
): LiveCheck {
  if (result.selected_format_compatible === false) {
    return {
      status: "illegal",
      format,
      reasons: result.selected_format_reasons,
    };
  }
  if (result.selected_format_compatible === true) {
    return { status: "legal", format };
  }
  return { status: "idle" };
}
