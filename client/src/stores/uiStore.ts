import { create } from "zustand";
import type {
  GameAction,
  ObjectId,
} from "../adapter/types";

// Guard against spurious mouseleave events that fire ~30ms after mouseenter
// (caused by Framer Motion layout recalculations moving elements under the cursor).
let lastInspectSetAt = 0;
const INSPECT_DEBOUNCE_MS = 80;

interface UiStoreState {
  selectedObjectId: ObjectId | null;
  hoveredObjectId: ObjectId | null;
  inspectedObjectId: ObjectId | null;
  inspectedFaceIndex: number;
  selectedCardIds: ObjectId[];
  fullControl: boolean;
  autoPass: boolean;
  combatMode: "attackers" | "blockers" | null;
  selectedAttackers: ObjectId[];
  blockerAssignments: Map<ObjectId, ObjectId>;
  combatClickHandler: ((id: ObjectId) => void) | null;
  previewSticky: boolean;
  isDragging: boolean;
  showTurnBanner: boolean;
  turnBannerText: string;
  focusedOpponent: number | null;
  pendingAbilityChoice: { objectId: ObjectId; actions: GameAction[] } | null;
  debugPanelOpen: boolean;
}

interface UiStoreActions {
  selectObject: (id: ObjectId | null) => void;
  hoverObject: (id: ObjectId | null) => void;
  inspectObject: (id: ObjectId | null, faceIndex?: number) => void;
  addSelectedCard: (cardId: ObjectId) => void;
  toggleSelectedCard: (cardId: ObjectId) => void;
  clearSelectedCards: () => void;
  toggleFullControl: () => void;
  toggleAutoPass: () => void;
  setCombatMode: (mode: "attackers" | "blockers" | null) => void;
  toggleAttacker: (id: ObjectId) => void;
  selectAllAttackers: (ids: ObjectId[]) => void;
  assignBlocker: (blockerId: ObjectId, attackerId: ObjectId) => void;
  removeBlockerAssignment: (blockerId: ObjectId) => void;
  clearCombatSelection: () => void;
  setCombatClickHandler: (handler: ((id: ObjectId) => void) | null) => void;
  setPreviewSticky: (sticky: boolean) => void;
  setDragging: (dragging: boolean) => void;
  flashTurnBanner: (text: string) => void;
  setFocusedOpponent: (id: number | null) => void;
  setPendingAbilityChoice: (choice: { objectId: ObjectId; actions: GameAction[] } | null) => void;
  toggleDebugPanel: () => void;
}

export type UiStore = UiStoreState & UiStoreActions;

export const useUiStore = create<UiStore>()((set) => ({
  selectedObjectId: null,
  hoveredObjectId: null,
  inspectedObjectId: null,
  inspectedFaceIndex: 0,
  selectedCardIds: [],
  fullControl: false,
  autoPass: false,
  combatMode: null,
  selectedAttackers: [],
  blockerAssignments: new Map(),
  combatClickHandler: null,
  previewSticky: false,
  isDragging: false,
  showTurnBanner: false,
  turnBannerText: "",
  focusedOpponent: null,
  pendingAbilityChoice: null,
  debugPanelOpen: false,

  selectObject: (id) => set({ selectedObjectId: id }),
  hoverObject: (id) => set({ hoveredObjectId: id }),
  inspectObject: (id, faceIndex) => {
    if (id != null) {
      lastInspectSetAt = Date.now();
    } else if (Date.now() - lastInspectSetAt < INSPECT_DEBOUNCE_MS) {
      // Ignore spurious clear within debounce window
      return;
    }
    set({
      inspectedObjectId: id,
      inspectedFaceIndex: faceIndex ?? 0,
      ...(id == null ? { previewSticky: false } : {}),
    });
  },

  addSelectedCard: (cardId) =>
    set((state) => ({
      selectedCardIds: [...state.selectedCardIds, cardId],
    })),

  toggleSelectedCard: (cardId) =>
    set((state) => ({
      selectedCardIds: state.selectedCardIds.includes(cardId)
        ? state.selectedCardIds.filter((id) => id !== cardId)
        : [...state.selectedCardIds, cardId],
    })),

  clearSelectedCards: () =>
    set({
      selectedCardIds: [],
    }),

  toggleFullControl: () =>
    set((state) => ({ fullControl: !state.fullControl })),

  toggleAutoPass: () =>
    set((state) => ({ autoPass: !state.autoPass })),

  setCombatMode: (mode) => set({ combatMode: mode }),

  toggleAttacker: (id) =>
    set((state) => ({
      selectedAttackers: state.selectedAttackers.includes(id)
        ? state.selectedAttackers.filter((a) => a !== id)
        : [...state.selectedAttackers, id],
    })),

  selectAllAttackers: (ids) => set({ selectedAttackers: ids }),

  assignBlocker: (blockerId, attackerId) =>
    set((state) => {
      const next = new Map(state.blockerAssignments);
      next.set(blockerId, attackerId);
      return { blockerAssignments: next };
    }),

  removeBlockerAssignment: (blockerId) =>
    set((state) => {
      const next = new Map(state.blockerAssignments);
      next.delete(blockerId);
      return { blockerAssignments: next };
    }),

  clearCombatSelection: () =>
    set({
      combatMode: null,
      selectedAttackers: [],
      blockerAssignments: new Map(),
      combatClickHandler: null,
    }),

  setCombatClickHandler: (handler) => set({ combatClickHandler: handler }),
  setPreviewSticky: (sticky) => set({ previewSticky: sticky }),
  setDragging: (dragging) => set({ isDragging: dragging }),
  flashTurnBanner: (text) => {
    set({ showTurnBanner: true, turnBannerText: text });
    setTimeout(() => set({ showTurnBanner: false }), 1500);
  },
  setFocusedOpponent: (id) => set({ focusedOpponent: id }),
  setPendingAbilityChoice: (choice) => set({ pendingAbilityChoice: choice }),
  toggleDebugPanel: () => set((state) => ({ debugPanelOpen: !state.debugPanelOpen })),
}));

// DEBUG: temporary — remove after fixing hover preview
if (typeof window !== "undefined") {
  (window as unknown as Record<string, unknown>).__uiStore = useUiStore;
}
