import { useCallback, useEffect, useMemo, useState } from "react";

import { fetchCardData, normalizeCardName, type ScryfallCard } from "../services/scryfall";

function mergeIntoCache(
  previous: Map<string, ScryfallCard>,
  cards: Iterable<ScryfallCard>,
): Map<string, ScryfallCard> {
  const next = new Map(previous);
  for (const card of cards) {
    next.set(card.name, card);
    next.set(normalizeCardName(card.name), card);
  }
  return next;
}

function mergeFetchedCards(
  previous: Map<string, ScryfallCard>,
  fetchedCards: Array<{ requestedName: string; card: ScryfallCard }>,
): Map<string, ScryfallCard> {
  const next = new Map(previous);
  for (const { requestedName, card } of fetchedCards) {
    next.set(requestedName, card);
    next.set(normalizeCardName(requestedName), card);
    next.set(card.name, card);
    next.set(normalizeCardName(card.name), card);
  }
  return next;
}

function hasCachedCard(
  cardDataCache: Map<string, ScryfallCard>,
  cardName: string,
): boolean {
  return cardDataCache.has(cardName) || cardDataCache.has(normalizeCardName(cardName));
}

export function useDeckCardData(requiredCardNames: string[]) {
  const [cardDataCache, setCardDataCache] = useState<Map<string, ScryfallCard>>(
    new Map(),
  );

  const cacheCards = useCallback((cards: Iterable<ScryfallCard>) => {
    setCardDataCache((prev) => mergeIntoCache(prev, cards));
  }, []);

  const requiredCardNamesKey = requiredCardNames.join("\n");
  const requiredNames = useMemo(
    () => [...new Set(requiredCardNamesKey ? requiredCardNamesKey.split("\n") : [])].sort(),
    [requiredCardNamesKey],
  );

  useEffect(() => {
    const missingNames = requiredNames.filter((name) => !hasCachedCard(cardDataCache, name));
    if (missingNames.length === 0) return;

    let cancelled = false;

    async function hydrateMissingCards(): Promise<void> {
      const results = await Promise.allSettled(
        missingNames.map((name) => fetchCardData(name)),
      );
      if (cancelled) return;

      const resolvedCards = results.flatMap((result, index) =>
        result.status === "fulfilled"
          ? [{ requestedName: missingNames[index], card: result.value }]
          : [],
      );
      if (resolvedCards.length === 0) return;

      setCardDataCache((prev) => mergeFetchedCards(prev, resolvedCards));
    }

    hydrateMissingCards();

    return () => {
      cancelled = true;
    };
  }, [cardDataCache, requiredNames]);

  return { cardDataCache, cacheCards };
}
