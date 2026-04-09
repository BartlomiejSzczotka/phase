import type { ResolvedAbility, StackEntry, TargetRef } from "../../adapter/types.ts";

function collectTargets(ability: ResolvedAbility): TargetRef[] {
  const own = ability.targets ?? [];
  const sub = ability.sub_ability ? collectTargets(ability.sub_ability) : [];
  return [...own, ...sub];
}

export function getStackEntryTargets(entry: StackEntry): TargetRef[] {
  if (!("ability" in entry.kind.data) || !entry.kind.data.ability) {
    return [];
  }
  return collectTargets(entry.kind.data.ability);
}
