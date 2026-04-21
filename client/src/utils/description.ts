/**
 * Render engine-provided ability descriptions for display.
 *
 * The engine uses `~` as the canonical self-reference token (CR 201.4b).
 * Trigger, replacement, and static descriptions reach the client with `~`
 * in place of the source card's name — e.g. "When ~ enters, draw a card."
 * This helper substitutes `~` back to the source's display name for
 * player-facing UI.
 *
 * Matches only the standalone token (word boundary or punctuation) so it
 * will not accidentally rewrite `~` that appears inside a longer
 * identifier — not that Oracle text uses tildes elsewhere, but the guard
 * keeps the substitution robust if that ever changes.
 */
export function renderDescription(description: string, sourceName: string): string {
  return description.replace(/~/g, sourceName);
}
