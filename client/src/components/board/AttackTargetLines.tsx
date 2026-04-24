import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { usePreferencesStore } from "../../stores/preferencesStore.ts";
import { useGameStore } from "../../stores/gameStore.ts";
import { usePlayerId } from "../../hooks/usePlayerId.ts";
import { useRafPositions } from "../../hooks/useRafPositions.ts";
import type { ObjectId, PlayerId } from "../../adapter/types.ts";

interface Pos {
  x: number;
  y: number;
}

interface PlayerArrow {
  attackerId: ObjectId;
  defender: PlayerId;
  isAtMe: boolean;
}

interface PlayerArrowPos {
  key: string;
  from: Pos;
  to: Pos;
  isAtMe: boolean;
}

/** Red arrows from attackers to their targets.
 *
 *  Two cases, two channels:
 *  - Planeswalker / Battle → dashed red line (legacy 1v1+ behavior, unchanged).
 *  - Player → solid red arc, only drawn when >2 players (multiplayer / Commander).
 *    In 2-player games a player attack is implicit and drawing would be noise.
 *    Thicker stroke + glow when the local player is the defender. */
export function AttackTargetLines() {
  const combat = useGameStore((s) => s.gameState?.combat ?? null);
  const seatOrder = useGameStore((s) => s.gameState?.seat_order);
  const vfxQuality = usePreferencesStore((s) => s.vfxQuality);
  const localPlayerId = usePlayerId();
  const isMinimal = vfxQuality === "minimal";

  const isMultiplayer = (seatOrder?.length ?? 0) > 2;

  const objectPairs = useMemo(() => {
    const map = new Map<ObjectId, ObjectId>();
    if (!combat) return map;
    for (const attacker of combat.attackers) {
      if (
        attacker.attack_target.type === "Planeswalker"
        || attacker.attack_target.type === "Battle"
      ) {
        map.set(attacker.object_id, attacker.attack_target.data);
      }
    }
    return map;
  }, [combat]);

  const playerArrows = useMemo<PlayerArrow[]>(() => {
    if (!combat || !isMultiplayer) return [];
    const out: PlayerArrow[] = [];
    for (const attacker of combat.attackers) {
      if (attacker.attack_target.type !== "Player") continue;
      const defender = attacker.attack_target.data;
      out.push({
        attackerId: attacker.object_id,
        defender,
        isAtMe: defender === localPlayerId,
      });
    }
    return out;
  }, [combat, isMultiplayer, localPlayerId]);

  const objectPositions = useRafPositions(objectPairs);
  const playerPositions = usePlayerArrowPositions(playerArrows);

  if (objectPositions.size === 0 && playerPositions.length === 0) return null;

  return createPortal(
    <svg className="pointer-events-none fixed inset-0 z-30 h-full w-full">
      {!isMinimal && (
        <defs>
          <filter id="attack-target-glow">
            <feGaussianBlur stdDeviation="3" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
          <marker
            id="attack-target-arrow"
            markerWidth="8"
            markerHeight="6"
            refX="8"
            refY="3"
            orient="auto"
          >
            <path d="M0,0 L8,3 L0,6 Z" fill="rgba(220,38,38,0.85)" />
          </marker>
        </defs>
      )}

      {/* Planeswalker / Battle targets: straight dashed red line. */}
      {Array.from(objectPositions.entries()).map(([attackerId, pos]) => (
        <line
          key={`obj-${attackerId}`}
          x1={pos.from.x}
          y1={pos.from.y}
          x2={pos.to.x}
          y2={pos.to.y}
          stroke="rgba(220,38,38,0.7)"
          strokeWidth={isMinimal ? 1.5 : 2.5}
          strokeDasharray={isMinimal ? undefined : "8 4"}
          filter={isMinimal ? undefined : "url(#attack-target-glow)"}
          markerEnd={isMinimal ? undefined : "url(#attack-target-arrow)"}
        />
      ))}

      {/* Player targets (multiplayer only): solid red arc. "At me" renders
          thicker and fully opaque; spectator arrows (other vs other) render
          lighter so the local defender view stays dominant. */}
      {playerPositions.map((arrow) => (
        <path
          key={`pl-${arrow.key}`}
          d={arcPath(arrow.from, arrow.to)}
          stroke={arrow.isAtMe ? "rgba(220,38,38,0.95)" : "rgba(220,38,38,0.45)"}
          strokeWidth={arrow.isAtMe ? 3.5 : 2}
          fill="none"
          filter={isMinimal || !arrow.isAtMe ? undefined : "url(#attack-target-glow)"}
          markerEnd={isMinimal ? undefined : "url(#attack-target-arrow)"}
        />
      ))}
    </svg>,
    document.body,
  );
}

function arcPath(from: Pos, to: Pos): string {
  const mx = (from.x + to.x) / 2;
  const my = (from.y + to.y) / 2;
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const dist = Math.sqrt(dx * dx + dy * dy) || 1;
  const offset = Math.min(80, dist * 0.3);
  const nx = -dy / dist;
  const ny = dx / dist;
  return `M ${from.x} ${from.y} Q ${mx + nx * offset} ${my + ny * offset} ${to.x} ${to.y}`;
}

/** RAF-polled positions for attacker → player-HUD pairs. Mirrors useRafPositions
 *  but resolves the target endpoint via `data-player-hud` rather than
 *  `data-object-id`. Kept local to this component so the shared hook doesn't
 *  need to learn about two endpoint kinds. */
function usePlayerArrowPositions(arrows: PlayerArrow[]): PlayerArrowPos[] {
  const [positions, setPositions] = useState<PlayerArrowPos[]>([]);
  const prevRectsRef = useRef<Map<string, DOMRect>>(new Map());
  const stableCountRef = useRef(0);

  useEffect(() => {
    if (arrows.length === 0) {
      setPositions([]);
      return;
    }
    stableCountRef.current = 0;
    prevRectsRef.current = new Map();
    let rafId = 0;

    const poll = () => {
      const current = new Map<string, DOMRect>();
      let changed = false;

      for (const a of arrows) {
        const fromKey = `o:${a.attackerId}`;
        const toKey = `p:${a.defender}`;
        if (!current.has(fromKey)) {
          const el = document.querySelector(`[data-object-id="${a.attackerId}"]`);
          if (el) current.set(fromKey, el.getBoundingClientRect());
        }
        if (!current.has(toKey)) {
          const el = document.querySelector(`[data-player-hud="${a.defender}"]`);
          if (el) current.set(toKey, el.getBoundingClientRect());
        }
        for (const key of [fromKey, toKey]) {
          const prev = prevRectsRef.current.get(key);
          const now = current.get(key);
          if (!now) continue;
          if (
            !prev
            || Math.abs(prev.left - now.left) > 0.5
            || Math.abs(prev.top - now.top) > 0.5
            || Math.abs(prev.width - now.width) > 0.5
          ) {
            changed = true;
          }
        }
      }

      stableCountRef.current = changed ? 0 : stableCountRef.current + 1;
      prevRectsRef.current = current;

      const next: PlayerArrowPos[] = [];
      for (const a of arrows) {
        const fromRect = current.get(`o:${a.attackerId}`);
        const toRect = current.get(`p:${a.defender}`);
        if (!fromRect || !toRect) continue;
        next.push({
          key: `${a.attackerId}->${a.defender}`,
          from: {
            x: fromRect.left + fromRect.width / 2,
            y: fromRect.top + fromRect.height / 2,
          },
          to: {
            x: toRect.left + toRect.width / 2,
            y: toRect.top + toRect.height / 2,
          },
          isAtMe: a.isAtMe,
        });
      }
      setPositions(next);

      if (stableCountRef.current < 10) {
        rafId = requestAnimationFrame(poll);
      }
    };

    rafId = requestAnimationFrame(poll);
    return () => cancelAnimationFrame(rafId);
  }, [arrows]);

  return positions;
}
