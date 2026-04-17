import type { GameAction, ObjectId } from "../adapter/types.ts";

/**
 * Look up the legal actions whose `source_object()` is `objectId`.
 *
 * Per CLAUDE.md "the frontend is a display layer, not a logic layer", the
 * mapping from `GameAction` variant to "the permanent it acts on" is owned
 * by the engine via `GameAction::source_object()`. This function is now a
 * trivial map lookup over the engine-provided `legalActionsByObject` field
 * — never a client-side discriminated-union introspection.
 */
export function collectObjectActions(
  legalActionsByObject: Record<string, GameAction[]> | undefined,
  objectId: ObjectId,
): GameAction[] {
  if (!legalActionsByObject) return [];
  return legalActionsByObject[String(objectId)] ?? [];
}
