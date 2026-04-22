import { useEffect, useState } from "react";

import { onEngineLost } from "../../game/engineRecovery";

/**
 * Layer 3 fallback for engine state loss — the user-facing prompt.
 *
 * The recovery layers sit above this:
 *   Layer 1: adapter classifies `NOT_INITIALIZED:` as STATE_LOST.
 *   Layer 2: `attemptStateRehydrate` silently restores from the store.
 * When Layer 2 fails (or Layer 2 can't run because the mode isn't
 * locally recoverable — P2P host, WS, or the AI controller hits its hard
 * stop), this modal fires.
 *
 * Reloading is the correct escalation: `GameProvider` runs its resume
 * path on mount, rehydrating from IDB for AI/local games or from the
 * persisted P2P host session for hosts. The user's last-saved turn is
 * preserved because `dispatch.ts` saves to IDB *before* animations play.
 *
 * The listener is de-duped (`shown` latch) so repeated failures within
 * the same tab session don't stack multiple modals.
 */
export function EngineLostModal() {
  const [shown, setShown] = useState(false);
  const [reason, setReason] = useState<string>("");

  useEffect(() => {
    return onEngineLost((r: string) => {
      setShown((prev) => {
        if (prev) return prev;
        setReason(r);
        return true;
      });
    });
  }, []);

  if (!shown) return null;

  const handleReload = () => {
    window.location.reload();
  };

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center">
      <div className="absolute inset-0 bg-black/80" />
      <div className="relative z-10 max-w-md rounded-xl bg-gray-900 p-8 shadow-2xl ring-1 ring-rose-700/60">
        <h2 className="mb-3 text-xl font-bold text-white">Engine connection lost</h2>
        <p className="mb-4 text-sm text-gray-300">
          phase.rs lost its link to the game engine — most often caused by a
          background update activating mid-game. Your last saved turn is
          preserved.
        </p>
        <p className="mb-6 text-xs text-gray-500">
          Reload to restore the game. ({reason})
        </p>
        <div className="flex justify-end gap-3">
          <button
            onClick={handleReload}
            className="rounded-lg bg-rose-600 px-4 py-2 text-sm font-semibold text-white transition-colors hover:bg-rose-500"
            autoFocus
          >
            Reload
          </button>
        </div>
      </div>
    </div>
  );
}
