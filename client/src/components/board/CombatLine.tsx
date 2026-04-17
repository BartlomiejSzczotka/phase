import { useEffect, useState } from "react";
import { motion, useAnimationControls, useReducedMotion } from "framer-motion";

import type { Phase } from "../../adapter/types.ts";
import { useGameStore } from "../../stores/gameStore.ts";

/** CR 506.1: combat phase steps. */
const COMBAT_PHASES: ReadonlySet<Phase> = new Set<Phase>([
  "BeginCombat",
  "DeclareAttackers",
  "DeclareBlockers",
  "CombatDamage",
  "EndCombat",
]);

/**
 * Midfield combat line — 1:1 port of alchemy's `BattleLine`. A faint slate
 * midline is always visible; during combat phases it warms to a pulsing
 * red/orange, with an ignition flash + glow when combat begins and three
 * staggered ember particles drifting along the line. Lowest z-order and
 * pointer-events-none so it never interferes with card interaction.
 */
export function CombatLine() {
  const phase = useGameStore((s) => s.gameState?.phase);
  const inCombat = phase !== undefined && COMBAT_PHASES.has(phase);
  const shouldReduceMotion = useReducedMotion();
  const igniteControls = useAnimationControls();
  const [prevCombat, setPrevCombat] = useState(false);
  const [showIgnition, setShowIgnition] = useState(false);

  // Derived-state transition detection (alchemy pattern).
  if (inCombat && !prevCombat) {
    setPrevCombat(true);
    setShowIgnition(true);
  }
  if (!inCombat && prevCombat) {
    setPrevCombat(false);
  }

  useEffect(() => {
    if (!showIgnition) return;
    igniteControls.start({
      opacity: [0, 1, 0.6, 0],
      scaleY: [0.5, 3, 1.5, 0],
      transition: { duration: 0.7, ease: "easeOut" },
    });
    const timer = setTimeout(() => setShowIgnition(false), 700);
    return () => clearTimeout(timer);
  }, [showIgnition, igniteControls]);

  return (
    <div className="pointer-events-none relative z-0 flex shrink-0 items-center justify-center px-8 py-1">
      {/* Main line */}
      <motion.div
        className="w-full"
        style={{
          height: inCombat ? 1.5 : 1,
          background:
            "linear-gradient(90deg, transparent, rgba(148, 163, 184, 0.2), transparent)",
        }}
        animate={
          inCombat && !shouldReduceMotion
            ? {
                boxShadow: [
                  "0 0 6px 1px rgba(239, 68, 68, 0.2), 0 0 14px 3px rgba(239, 68, 68, 0.1)",
                  "0 0 12px 3px rgba(239, 68, 68, 0.4), 0 0 24px 6px rgba(239, 68, 68, 0.15)",
                  "0 0 6px 1px rgba(239, 68, 68, 0.2), 0 0 14px 3px rgba(239, 68, 68, 0.1)",
                ],
                background: [
                  "linear-gradient(90deg, transparent 5%, rgba(239, 68, 68, 0.25) 30%, rgba(251, 146, 60, 0.3) 50%, rgba(239, 68, 68, 0.25) 70%, transparent 95%)",
                  "linear-gradient(90deg, transparent 5%, rgba(239, 68, 68, 0.45) 30%, rgba(251, 146, 60, 0.5) 50%, rgba(239, 68, 68, 0.45) 70%, transparent 95%)",
                  "linear-gradient(90deg, transparent 5%, rgba(239, 68, 68, 0.25) 30%, rgba(251, 146, 60, 0.3) 50%, rgba(239, 68, 68, 0.25) 70%, transparent 95%)",
                ],
              }
            : { boxShadow: "0 0 3px 1px rgba(148, 163, 184, 0.06)" }
        }
        transition={
          inCombat && !shouldReduceMotion
            ? { duration: 1.2, repeat: Infinity, ease: "easeInOut" }
            : { duration: 0.3 }
        }
      />

      {/* Ignition flash — bright horizontal sweep when combat begins */}
      {showIgnition && !shouldReduceMotion && (
        <motion.div
          className="absolute left-0 right-0 mx-8"
          style={{
            height: 2,
            top: "50%",
            marginTop: -1,
            background:
              "linear-gradient(90deg, transparent, rgba(255, 200, 60, 0.9) 20%, rgba(255, 255, 255, 0.95) 50%, rgba(255, 200, 60, 0.9) 80%, transparent)",
            filter: "blur(1px)",
          }}
          initial={{ scaleX: 0, opacity: 0 }}
          animate={{ scaleX: [0, 1.1, 1], opacity: [0, 1, 0] }}
          transition={{ duration: 0.5, ease: [0.22, 1, 0.36, 1] }}
        />
      )}

      {/* Ignition glow — vertical bloom that fades */}
      {showIgnition && !shouldReduceMotion && (
        <motion.div
          className="absolute left-0 right-0 mx-8"
          style={{
            height: 40,
            top: "50%",
            marginTop: -20,
            background:
              "linear-gradient(90deg, transparent, rgba(239, 68, 68, 0.3) 30%, rgba(251, 146, 60, 0.4) 50%, rgba(239, 68, 68, 0.3) 70%, transparent)",
            filter: "blur(8px)",
          }}
          animate={igniteControls}
        />
      )}

      {/* Ember particles — three staggered drifts along the line during combat */}
      {inCombat && !shouldReduceMotion && (
        <>
          <motion.div
            className="absolute rounded-full"
            style={{
              width: 4,
              height: 4,
              background: "rgba(251, 146, 60, 0.9)",
              boxShadow: "0 0 8px rgba(251, 146, 60, 0.7)",
              top: "50%",
              marginTop: -2,
            }}
            animate={{ left: ["10%", "90%"], opacity: [0, 1, 1, 0] }}
            transition={{ duration: 2.5, repeat: Infinity, ease: "linear", delay: 0 }}
          />
          <motion.div
            className="absolute rounded-full"
            style={{
              width: 3,
              height: 3,
              background: "rgba(239, 68, 68, 0.8)",
              boxShadow: "0 0 6px rgba(239, 68, 68, 0.6)",
              top: "50%",
              marginTop: -1.5,
            }}
            animate={{ left: ["85%", "15%"], opacity: [0, 0.8, 0.8, 0] }}
            transition={{ duration: 3, repeat: Infinity, ease: "linear", delay: 0.5 }}
          />
          <motion.div
            className="absolute rounded-full"
            style={{
              width: 3,
              height: 3,
              background: "rgba(255, 200, 60, 0.85)",
              boxShadow: "0 0 6px rgba(255, 200, 60, 0.6)",
              top: "50%",
              marginTop: -1.5,
            }}
            animate={{ left: ["20%", "80%"], opacity: [0, 0.9, 0.9, 0] }}
            transition={{ duration: 2, repeat: Infinity, ease: "linear", delay: 1 }}
          />
        </>
      )}
    </div>
  );
}
