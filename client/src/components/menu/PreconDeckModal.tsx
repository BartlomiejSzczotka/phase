import { useMemo, useState } from "react";

import { STORAGE_KEY_PREFIX } from "../../constants/storage";
import { useDecks, type DeckEntry } from "../../hooks/useDecks";
import type { ParsedDeck } from "../../services/deckParser";
import { menuButtonClass } from "./buttonStyles";

interface PreconDeckModalProps {
  open: boolean;
  onClose: () => void;
  onImported: (name: string) => void;
}

function deckEntryToParsedDeck(deck: DeckEntry): ParsedDeck {
  return {
    main: deck.mainBoard.map((c) => ({ name: c.name, count: c.count })),
    sideboard: (deck.sideBoard ?? []).map((c) => ({ name: c.name, count: c.count })),
    commander:
      deck.commander && deck.commander.length > 0
        ? deck.commander.map((c) => c.name)
        : undefined,
  };
}

export function PreconDeckModal({ open, onClose, onImported }: PreconDeckModalProps) {
  const decks = useDecks();
  const [query, setQuery] = useState("");
  const [typeFilter, setTypeFilter] = useState<string>("All");

  const deckTypes = useMemo(() => {
    if (!decks) return ["All"];
    const set = new Set<string>();
    for (const d of Object.values(decks)) set.add(d.type);
    return ["All", ...Array.from(set).sort()];
  }, [decks]);

  const filtered = useMemo(() => {
    if (!decks) return [];
    const q = query.trim().toLowerCase();
    const all = Object.entries(decks);
    const byTypeAndQuery = all.filter(([, d]) => {
      if (typeFilter !== "All" && d.type !== typeFilter) return false;
      if (!q) return true;
      return (
        d.name.toLowerCase().includes(q) ||
        d.code.toLowerCase().includes(q) ||
        d.type.toLowerCase().includes(q)
      );
    });
    // Newest first — preconstructed decks are usually sought by recent release.
    byTypeAndQuery.sort(([, a], [, b]) =>
      (b.releaseDate ?? "").localeCompare(a.releaseDate ?? ""),
    );
    return byTypeAndQuery.slice(0, 500);
  }, [decks, query, typeFilter]);

  if (!open) return null;

  const handlePick = (id: string, deck: DeckEntry) => {
    const suggested = `${deck.name} (${deck.code})`;
    const chosen = prompt("Save preconstructed deck as:", suggested);
    if (!chosen) return;
    const key = STORAGE_KEY_PREFIX + chosen;
    if (localStorage.getItem(key) && !confirm(`"${chosen}" already exists. Overwrite?`)) return;
    localStorage.setItem(key, JSON.stringify(deckEntryToParsedDeck(deck)));
    onImported(chosen);
    onClose();
    void id;
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        className="flex max-h-[80vh] w-full max-w-3xl flex-col gap-4 rounded-2xl border border-white/10 bg-[#0a0f1b] p-6 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start justify-between gap-4">
          <div>
            <h2 className="text-xl font-semibold text-white">Preconstructed Decks</h2>
            <p className="mt-1 text-xs text-slate-400">
              Sourced from MTGJSON. Filtered to decks the engine can play fully
              {decks && (
                <span className="ml-1 text-slate-500">
                  · {Object.keys(decks).length} available
                </span>
              )}
            </p>
          </div>
          <button
            onClick={onClose}
            className="text-slate-400 transition hover:text-white"
            aria-label="Close"
          >
            ✕
          </button>
        </div>

        <div className="flex flex-col gap-2 sm:flex-row">
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search by name, set code, or type…"
            className="flex-1 rounded-lg border border-white/10 bg-black/30 px-3 py-2 text-sm text-white placeholder:text-slate-500 focus:border-white/30 focus:outline-none"
            autoFocus
          />
          <select
            value={typeFilter}
            onChange={(e) => setTypeFilter(e.target.value)}
            className="rounded-lg border border-white/10 bg-black/30 px-3 py-2 text-sm text-white focus:border-white/30 focus:outline-none"
          >
            {deckTypes.map((t) => (
              <option key={t} value={t}>
                {t}
              </option>
            ))}
          </select>
        </div>

        <div className="flex-1 overflow-y-auto rounded-lg border border-white/5 bg-black/20">
          {!decks ? (
            <div className="p-8 text-center text-sm text-slate-500">Loading deck catalog…</div>
          ) : filtered.length === 0 ? (
            <div className="p-8 text-center text-sm text-slate-500">No decks match.</div>
          ) : (
            <ul className="divide-y divide-white/5">
              {filtered.map(([id, deck]) => (
                <li key={id}>
                  <button
                    onClick={() => handlePick(id, deck)}
                    className="flex w-full items-center justify-between gap-3 px-4 py-2 text-left transition hover:bg-white/5"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-sm font-medium text-white">{deck.name}</div>
                      <div className="truncate text-[11px] text-slate-500">
                        {deck.type}
                        {deck.releaseDate && <span> · {deck.releaseDate}</span>}
                        <span> · {deck.code}</span>
                      </div>
                    </div>
                    <span className="shrink-0 text-[11px] text-slate-600">
                      {deck.mainBoard.reduce((n, c) => n + c.count, 0)} cards
                      {deck.commander && deck.commander.length > 0 && " · cmdr"}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div className="flex items-center justify-between text-xs text-slate-500">
          <span>
            {decks && filtered.length === 500 && "Showing first 500 — refine search to narrow."}
          </span>
          <button
            onClick={onClose}
            className={menuButtonClass({ tone: "neutral", size: "sm" })}
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
