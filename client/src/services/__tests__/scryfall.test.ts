import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("idb-keyval", () => ({
  get: vi.fn(() => Promise.resolve(undefined)),
}));

function makeCardResponse(name: string): Response {
  return new Response(
    JSON.stringify({
      id: `${name}-id`,
      name,
      mana_cost: "{1}",
      cmc: 1,
      type_line: "Instant",
      color_identity: [],
      legalities: {},
      image_uris: {
        normal: `https://img.example/${encodeURIComponent(name)}.jpg`,
      },
    }),
    {
      status: 200,
      headers: { "Content-Type": "application/json" },
    },
  );
}

async function loadScryfallModule() {
  vi.resetModules();
  return import("../scryfall.ts");
}

describe("scryfall service", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("deduplicates concurrent card lookups for the same name", async () => {
    global.fetch = vi.fn().mockResolvedValue(makeCardResponse("Lightning Bolt"));

    const { fetchCardData } = await loadScryfallModule();
    const [first, second] = await Promise.all([
      fetchCardData("Lightning Bolt"),
      fetchCardData("Lightning Bolt"),
    ]);

    expect(first.name).toBe("Lightning Bolt");
    expect(second.name).toBe("Lightning Bolt");
    expect(global.fetch).toHaveBeenCalledTimes(1);
    expect(global.fetch).toHaveBeenCalledWith(
      "https://api.scryfall.com/cards/named?exact=Lightning%20Bolt",
    );
  });

  it("retries when fetch is rejected before the browser exposes the status code", async () => {
    vi.useFakeTimers();
    global.fetch = vi
      .fn()
      .mockRejectedValueOnce(new TypeError("Failed to fetch"))
      .mockResolvedValueOnce(makeCardResponse("Counterspell"));

    const { fetchCardData } = await loadScryfallModule();
    const pending = fetchCardData("Counterspell");

    await vi.advanceTimersByTimeAsync(1000);
    const card = await pending;

    expect(card.name).toBe("Counterspell");
    expect(global.fetch).toHaveBeenCalledTimes(2);
  });

  it("serializes Scryfall requests so concurrent misses do not burst", async () => {
    vi.useFakeTimers();

    let inFlight = 0;
    let maxInFlight = 0;
    const resolvers: Array<(response: Response) => void> = [];

    global.fetch = vi.fn(() => {
      inFlight += 1;
      maxInFlight = Math.max(maxInFlight, inFlight);
      return new Promise<Response>((resolve) => {
        resolvers.push((response) => {
          inFlight -= 1;
          resolve(response);
        });
      });
    });

    const { fetchCardData } = await loadScryfallModule();
    const first = fetchCardData("Lightning Bolt");
    const second = fetchCardData("Counterspell");

    await vi.advanceTimersByTimeAsync(0);
    expect(global.fetch).toHaveBeenCalledTimes(1);

    resolvers.shift()!(makeCardResponse("Lightning Bolt"));
    await vi.advanceTimersByTimeAsync(0);
    expect(global.fetch).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(100);
    expect(global.fetch).toHaveBeenCalledTimes(2);

    resolvers.shift()!(makeCardResponse("Counterspell"));
    await Promise.all([first, second]);

    expect(maxInFlight).toBe(1);
  });
});
