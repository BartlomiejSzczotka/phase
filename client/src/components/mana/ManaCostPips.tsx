import type { ManaCost } from "../../adapter/types.ts";
import { SHARD_ABBREVIATION } from "../../viewmodel/costLabel.ts";
import { ManaSymbol } from "./ManaSymbol.tsx";

/** Convert a ManaCost to display-ready shard abbreviations (e.g., ["2", "U", "U"]). */
function manaCostToShards(cost: ManaCost): string[] {
  if (cost.type !== "Cost") return [];
  const shards: string[] = [];
  if (cost.generic > 0) shards.push(String(cost.generic));
  for (const s of cost.shards) {
    shards.push(SHARD_ABBREVIATION[s] ?? s);
  }
  return shards;
}

type PipSize = "sm" | "md" | "lg";

const PIP_SIZES: Record<PipSize, { container: string; gap: string }> = {
  sm: { container: "w-[18px] h-[18px] p-[1.5px]", gap: "gap-[2px]" },
  md: { container: "w-[22px] h-[22px] p-[2px]", gap: "gap-[3px]" },
  lg: { container: "w-[28px] h-[28px] pt-[1px] pb-[3px] px-[2.5px]", gap: "gap-[3px]" },
};

interface ManaCostPipsProps {
  cost: ManaCost;
  isReduced?: boolean;
  size?: PipSize;
  className?: string;
}

/** Mana cost pips with dark circular backgrounds, MTGA-style. */
export function ManaCostPips({ cost, isReduced, size = "md", className = "" }: ManaCostPipsProps) {
  const shards = manaCostToShards(cost);
  if (shards.length === 0) return null;

  const s = PIP_SIZES[size];

  return (
    <div className={`pointer-events-none ${className}`}>
      <div className={`relative flex ${s.gap}`}>
        {/* Backdrop shifted up 1px to visually center behind the pips */}
        <div className="absolute -inset-x-[3px] -top-[4px] -bottom-[2px] rounded-full bg-gray-900/70" />
        {shards.map((shard, i) => (
          <div
            key={i}
            className={`relative ${s.container} rounded-full bg-gray-900/80 shadow-[0_1px_3px_rgba(0,0,0,0.6)] ${
              isReduced ? "ring-[1.5px] ring-green-400" : ""
            }`}
          >
            <ManaSymbol shard={shard} size="xs" className="w-full h-full" />
          </div>
        ))}
      </div>
    </div>
  );
}
