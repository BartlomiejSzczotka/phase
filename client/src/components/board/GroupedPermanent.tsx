import { memo, useMemo, useState } from "react";

import type { GroupedPermanent as GroupedPermanentType } from "../../viewmodel/battlefieldProps";
import { usePreferencesStore } from "../../stores/preferencesStore.ts";
import { useUiStore } from "../../stores/uiStore.ts";
import { useBoardInteractionState } from "./BoardInteractionContext.tsx";
import { PermanentCard } from "./PermanentCard.tsx";

interface GroupedPermanentProps {
  group: GroupedPermanentType;
}

export const GroupedPermanentDisplay = memo(function GroupedPermanentDisplay({ group }: GroupedPermanentProps) {
  const [expanded, setExpanded] = useState(false);
  const battlefieldCardDisplay = usePreferencesStore((s) => s.battlefieldCardDisplay);
  const combatMode = useUiStore((s) => s.combatMode);
  const { committedAttackerIds } = useBoardInteractionState();
  const containsAttacker = useMemo(() => {
    if (combatMode !== "blockers") return false;
    return group.ids.some((id) => committedAttackerIds.has(id));
  }, [combatMode, committedAttackerIds, group.ids]);

  // Auto-expand during blocker mode when the group contains attackers
  const effectiveExpanded = expanded || containsAttacker;

  if (group.count === 1) {
    return <PermanentCard objectId={group.ids[0]} />;
  }

  if (effectiveExpanded) {
    return (
      <div className="flex flex-wrap items-end gap-1">
        {group.ids.map((id) => (
          <PermanentCard key={id} objectId={id} />
        ))}
        {/* Only show collapse button for manual expansion, not auto-expand */}
        {expanded && !containsAttacker && (
          <button
            className="rounded bg-gray-800 px-1.5 py-0.5 text-[10px] text-gray-400 hover:bg-gray-700 hover:text-gray-200"
            onClick={() => setExpanded(false)}
          >
            collapse
          </button>
        )}
      </div>
    );
  }

  // Staggered render: show each card slightly offset to the right
  const STAGGER_PX = 20;

  return (
    <div
      className="relative"
      style={{
        // Reserve width for staggered cards beyond the first
        paddingRight: `${(group.count - 1) * STAGGER_PX}px`,
      }}
    >
      {/* Each card staggered to the right, last card on top */}
      {group.ids.map((id, i) => (
        <div
          key={id}
          className="absolute top-0"
          style={{
            left: `${i * STAGGER_PX}px`,
            zIndex: i,
          }}
        >
          <PermanentCard objectId={id} />
        </div>
      ))}

      {/* Invisible spacer sized to first card for layout */}
      <div
        aria-hidden="true"
        className="pointer-events-none"
        style={
          battlefieldCardDisplay === "art_crop"
            ? { width: "var(--art-crop-w)", height: "var(--art-crop-h)" }
            : { width: "var(--card-w)", height: "var(--card-h)" }
        }
      />

      {/* Count badge — orange glow during blocker mode to hint at expansion */}
      <button
        type="button"
        onClick={() => setExpanded(true)}
        className={`absolute left-1 top-1 z-30 flex h-5 w-5 items-center justify-center rounded-full bg-black/80 text-[10px] font-bold text-white transition-colors hover:bg-black ${
          combatMode === "blockers"
            ? "ring-2 ring-orange-500 shadow-[0_0_8px_2px_rgba(249,115,22,0.6)]"
            : "ring-1 ring-gray-500"
        }`}
        aria-label={`Expand ${group.name} group`}
      >
        {group.count}
      </button>
    </div>
  );
}, (prev, next) => {
  // `representative` is intentionally excluded — GroupedPermanentDisplay does not
  // read group.representative; individual PermanentCard components subscribe to the store.
  const pg = prev.group;
  const ng = next.group;
  return pg.name === ng.name
    && pg.count === ng.count
    && pg.ids.length === ng.ids.length
    && pg.ids.every((id, i) => id === ng.ids[i]);
});
