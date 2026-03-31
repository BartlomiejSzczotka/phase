import { AnimatePresence, motion } from "framer-motion";

import type { GameAction, WaitingFor } from "../../adapter/types.ts";
import { usePlayerId } from "../../hooks/usePlayerId.ts";
import { useGameStore } from "../../stores/gameStore.ts";

type ModalFaceChoice = Extract<WaitingFor, { type: "ModalFaceChoice" }>;

export function ModalFaceModal() {
  const playerId = usePlayerId();
  const waitingFor = useGameStore((s) => s.waitingFor);
  const dispatch = useGameStore((s) => s.dispatch);

  if (waitingFor?.type !== "ModalFaceChoice") return null;
  if (waitingFor.data.player !== playerId) return null;

  const data = waitingFor.data as ModalFaceChoice["data"];

  return <ModalFaceContent objectId={data.object_id} dispatch={dispatch} />;
}

function ModalFaceContent({
  objectId,
  dispatch,
}: {
  objectId: number;
  dispatch: (action: GameAction) => Promise<unknown>;
}) {
  const obj = useGameStore((s) => s.gameState?.objects[objectId]);

  if (!obj) return null;

  const frontName = obj.name;
  const backName = obj.back_face?.name ?? "Back Face";

  return (
    <AnimatePresence>
      <motion.div
        className="fixed inset-0 z-50 flex items-center justify-center px-2 py-2 lg:px-4 lg:py-6"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.2 }}
      >
        <div className="absolute inset-0 bg-black/60" />

        <motion.div
          className="relative z-10 w-full max-w-sm overflow-hidden rounded-[16px] lg:rounded-[24px] border border-white/10 bg-[#0b1020]/96 shadow-[0_28px_80px_rgba(0,0,0,0.42)] backdrop-blur-md"
          initial={{ scale: 0.95, opacity: 0, y: 10 }}
          animate={{ scale: 1, opacity: 1, y: 0 }}
          exit={{ scale: 0.95, opacity: 0, y: 10 }}
          transition={{ duration: 0.2, ease: "easeOut" }}
        >
          <div className="border-b border-white/10 px-3 py-3 lg:px-5 lg:py-5">
            <div className="text-[0.68rem] uppercase tracking-[0.22em] text-slate-500">
              Modal DFC
            </div>
            <h2 className="mt-1 text-base font-semibold text-white lg:text-xl">Choose a Face</h2>
            <p className="mt-1 text-xs text-slate-400 lg:text-sm">
              Play as the front or back land face.
            </p>
          </div>
          <div className="flex flex-col gap-2 px-3 py-3 lg:px-5 lg:py-5">
            <button
              onClick={() =>
                dispatch({ type: "ChooseModalFace", data: { back_face: false } })
              }
              className="rounded-[16px] border border-white/8 bg-white/5 px-4 py-3 text-left transition hover:bg-white/8 hover:ring-1 hover:ring-cyan-400/30"
            >
              <span className="font-semibold text-white">Play {frontName}</span>
              <span className="ml-2 text-xs text-slate-400">(Front)</span>
            </button>
            <button
              onClick={() =>
                dispatch({ type: "ChooseModalFace", data: { back_face: true } })
              }
              className="rounded-[16px] border border-white/8 bg-white/5 px-4 py-3 text-left transition hover:bg-white/8 hover:ring-1 hover:ring-amber-400/30"
            >
              <span className="font-semibold text-white">Play {backName}</span>
              <span className="ml-2 text-xs text-slate-400">(Back)</span>
            </button>
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
}
