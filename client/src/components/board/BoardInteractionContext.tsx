import { createContext, useContext } from "react";

interface BoardInteractionState {
  activatableObjectIds: Set<number>;
  committedAttackerIds: Set<number>;
  manaTappableObjectIds: Set<number>;
  selectableManaCostCreatureIds: Set<number>;
  undoableTapObjectIds: Set<number>;
  validAttackerIds: Set<number>;
  validTargetObjectIds: Set<number>;
}

const EMPTY_SET = new Set<number>();

const EMPTY_STATE: BoardInteractionState = {
  activatableObjectIds: EMPTY_SET,
  committedAttackerIds: EMPTY_SET,
  manaTappableObjectIds: EMPTY_SET,
  selectableManaCostCreatureIds: EMPTY_SET,
  undoableTapObjectIds: EMPTY_SET,
  validAttackerIds: EMPTY_SET,
  validTargetObjectIds: EMPTY_SET,
};

export const BoardInteractionContext =
  createContext<BoardInteractionState>(EMPTY_STATE);

export function useBoardInteractionState(): BoardInteractionState {
  return useContext(BoardInteractionContext);
}
