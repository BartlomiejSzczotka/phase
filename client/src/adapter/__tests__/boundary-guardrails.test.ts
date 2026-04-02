import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

const ADAPTER_FILES = [
  "ws-adapter.ts",
  "p2p-adapter.ts",
  "wasm-adapter.ts",
  "engine-worker-client.ts",
  "engine-worker.ts",
  "tauri-adapter.ts",
  "index.ts",
];

describe("adapter boundary guardrails", () => {
  it("adapter modules do not import stores or use localStorage directly", () => {
    const adapterDir = dirname(fileURLToPath(import.meta.url));
    for (const file of ADAPTER_FILES) {
      const source = readFileSync(resolve(adapterDir, "..", file), "utf8");
      expect(source).not.toMatch(/from "\.\.\/stores\//);
      expect(source).not.toContain("localStorage");
    }
  });
});
