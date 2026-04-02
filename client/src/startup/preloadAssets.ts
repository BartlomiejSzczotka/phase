import { audioManager, initAudioOnInteraction } from "../audio/AudioManager";

export interface PreloadProgress {
  phase: "audio" | "complete";
  percent: number;
}

type ProgressListener = (progress: PreloadProgress) => void;

const listeners = new Set<ProgressListener>();
let preloadPromise: Promise<void> | null = null;

function emit(progress: PreloadProgress): void {
  for (const listener of listeners) {
    listener(progress);
  }
}

/** Subscribe to preload progress updates. Returns an unsubscribe function. */
export function subscribePreload(listener: ProgressListener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/**
 * Run the startup preload sequence:
 * 1. Register music interaction listeners
 * 2. Preload SFX audio buffers
 *
 * Also registers audio interaction listeners for music playback.
 * Idempotent — safe to call multiple times.
 */
export function ensurePreload(): Promise<void> {
  if (preloadPromise) return preloadPromise;

  preloadPromise = (async () => {
    initAudioOnInteraction();

    emit({ phase: "audio", percent: 20 });
    audioManager.warmUp();
    await audioManager.preloadSfx();
    emit({ phase: "complete", percent: 100 });
  })();

  return preloadPromise;
}
