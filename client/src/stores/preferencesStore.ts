import { create } from "zustand";
import { persist } from "zustand/middleware";

import type { GameFormat, MatchType, Phase } from "../adapter/types";
import type { AnimationSpeed, CombatPacing, VfxQuality } from "../animation/types";
import type { AIDifficulty } from "../constants/ai";
import { DEFAULT_AI_DIFFICULTY } from "../constants/ai";

export type CardSizePreference = "small" | "medium" | "large";
export type HudLayout = "inline" | "floating";
export type LogDefaultState = "open" | "closed";
export type BattlefieldCardDisplay = "art_crop" | "full_card";
export type TapRotation = "mtga" | "classic";
/** "auto-wubrg" picks a random battlefield matching the dominant mana color.
 *  "random" picks a random battlefield each game regardless of color.
 *  "none" disables the background image.
 *  "custom" uses the URL stored in `customBackgroundUrl`.
 *  Any other string is a battlefield or plain-color ID. */
export type BoardBackground = "auto-wubrg" | "random" | "none" | "custom" | (string & {});

interface PreferencesState {
  cardSize: CardSizePreference;
  hudLayout: HudLayout;
  followActiveOpponent: boolean;
  logDefaultState: LogDefaultState;
  boardBackground: BoardBackground;
  customBackgroundUrl: string;
  vfxQuality: VfxQuality;
  animationSpeed: AnimationSpeed;
  combatPacing: CombatPacing;
  phaseStops: Phase[];
  masterVolume: number;
  sfxVolume: number;
  musicVolume: number;
  sfxMuted: boolean;
  musicMuted: boolean;
  masterMuted: boolean;
  audioThemeId: string;
  customThemeUrls: Array<{ id: string; url: string }>;
  battlefieldCardDisplay: BattlefieldCardDisplay;
  tapRotation: TapRotation;
  showKeywordStrip: boolean;
  aiDifficulty: AIDifficulty;
  lastFormat: GameFormat | null;
  lastMatchType: MatchType;
  lastPlayerCount: number;
}

interface PreferencesActions {
  setCardSize: (size: CardSizePreference) => void;
  setHudLayout: (layout: HudLayout) => void;
  setFollowActiveOpponent: (enabled: boolean) => void;
  setLogDefaultState: (state: LogDefaultState) => void;
  setBoardBackground: (bg: BoardBackground) => void;
  setCustomBackgroundUrl: (url: string) => void;
  setVfxQuality: (quality: VfxQuality) => void;
  setAnimationSpeed: (speed: AnimationSpeed) => void;
  setCombatPacing: (pacing: CombatPacing) => void;
  setPhaseStops: (stops: Phase[]) => void;
  setMasterVolume: (vol: number) => void;
  setSfxVolume: (vol: number) => void;
  setMusicVolume: (vol: number) => void;
  setSfxMuted: (muted: boolean) => void;
  setMusicMuted: (muted: boolean) => void;
  setMasterMuted: (muted: boolean) => void;
  setAudioThemeId: (id: string) => void;
  addCustomThemeUrl: (id: string, url: string) => void;
  removeCustomThemeUrl: (id: string) => void;
  setBattlefieldCardDisplay: (display: BattlefieldCardDisplay) => void;
  setTapRotation: (rotation: TapRotation) => void;
  setShowKeywordStrip: (show: boolean) => void;
  setAiDifficulty: (difficulty: AIDifficulty) => void;
  setLastFormat: (format: GameFormat) => void;
  setLastMatchType: (matchType: MatchType) => void;
  setLastPlayerCount: (count: number) => void;
}

export const usePreferencesStore = create<PreferencesState & PreferencesActions>()(
  persist(
    (set) => ({
      cardSize: "medium",
      hudLayout: "inline",
      followActiveOpponent: false,
      logDefaultState: "closed",
      boardBackground: "auto-wubrg",
      customBackgroundUrl: "",
      vfxQuality: "full",
      animationSpeed: "normal",
      combatPacing: "normal",
      phaseStops: [],
      masterVolume: 100,
      sfxVolume: 70,
      musicVolume: 40,
      sfxMuted: false,
      musicMuted: false,
      masterMuted: false,
      audioThemeId: "planeswalker",
      customThemeUrls: [],
      battlefieldCardDisplay: "art_crop",
      tapRotation: "mtga",
      showKeywordStrip: true,
      aiDifficulty: DEFAULT_AI_DIFFICULTY,
      lastFormat: null,
      lastMatchType: "Bo1",
      lastPlayerCount: 2,

      setCardSize: (size) => set({ cardSize: size }),
      setHudLayout: (layout) => set({ hudLayout: layout }),
      setFollowActiveOpponent: (enabled) => set({ followActiveOpponent: enabled }),
      setLogDefaultState: (state) => set({ logDefaultState: state }),
      setBoardBackground: (bg) => set({ boardBackground: bg }),
      setCustomBackgroundUrl: (url) => set({ customBackgroundUrl: url.trim() }),
      setVfxQuality: (quality) => set({ vfxQuality: quality }),
      setAnimationSpeed: (speed) => set({ animationSpeed: speed }),
      setCombatPacing: (pacing) => set({ combatPacing: pacing }),
      setPhaseStops: (stops) => set({ phaseStops: stops }),
      setMasterVolume: (vol) => set({ masterVolume: vol }),
      setSfxVolume: (vol) => set({ sfxVolume: vol }),
      setMusicVolume: (vol) => set({ musicVolume: vol }),
      setSfxMuted: (muted) => set({ sfxMuted: muted }),
      setMusicMuted: (muted) => set({ musicMuted: muted }),
      setMasterMuted: (muted) => set({ masterMuted: muted }),
      setAudioThemeId: (id) => set({ audioThemeId: id }),
      addCustomThemeUrl: (id, url) =>
        set((state) => ({
          customThemeUrls: [...state.customThemeUrls, { id, url }],
        })),
      removeCustomThemeUrl: (id) =>
        set((state) => ({
          customThemeUrls: state.customThemeUrls.filter((e) => e.id !== id),
          // Reset to default if the removed theme was active
          ...(state.audioThemeId === id ? { audioThemeId: "planeswalker" } : {}),
        })),
      setBattlefieldCardDisplay: (display) => set({ battlefieldCardDisplay: display }),
      setTapRotation: (rotation) => set({ tapRotation: rotation }),
      setShowKeywordStrip: (show) => set({ showKeywordStrip: show }),
      setAiDifficulty: (difficulty) => set({ aiDifficulty: difficulty }),
      setLastFormat: (format) => set({ lastFormat: format }),
      setLastMatchType: (matchType) => set({ lastMatchType: matchType }),
      setLastPlayerCount: (count) => set({ lastPlayerCount: count }),
    }),
    { name: "phase-preferences" },
  ),
);
