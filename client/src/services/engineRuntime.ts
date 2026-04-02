type EngineModule = typeof import("@wasm/engine");

let engineModulePromise: Promise<EngineModule> | null = null;
let wasmInitPromise: Promise<void> | null = null;
let cardDbPromise: Promise<number> | null = null;

async function loadEngineModule(): Promise<EngineModule> {
  if (!engineModulePromise) {
    engineModulePromise = import("@wasm/engine");
  }
  return engineModulePromise;
}

export async function ensureWasmInit(): Promise<void> {
  if (!wasmInitPromise) {
    wasmInitPromise = (async () => {
      const engine = await loadEngineModule();
      await engine.default();
    })();
  }
  return wasmInitPromise;
}

export async function ensureCardDatabase(): Promise<number> {
  if (!cardDbPromise) {
    cardDbPromise = (async () => {
      await ensureWasmInit();
      const engine = await loadEngineModule();
      const resp = await fetch(__CARD_DATA_URL__);
      if (!resp.ok) {
        throw new Error(`Failed to load card-data.json (${resp.status})`);
      }
      const text = await resp.text();
      return engine.load_card_database(text);
    })();
  }
  return cardDbPromise;
}

export async function getCardFaceData(cardName: string) {
  await ensureCardDatabase();
  const engine = await loadEngineModule();
  return engine.get_card_face_data(cardName);
}

export async function getCardParseDetails(cardName: string) {
  await ensureCardDatabase();
  const engine = await loadEngineModule();
  return engine.get_card_parse_details(cardName);
}

export async function evaluateDeckCompatibilityJs(request: unknown) {
  await ensureCardDatabase();
  const engine = await loadEngineModule();
  return engine.evaluate_deck_compatibility_js(request);
}
