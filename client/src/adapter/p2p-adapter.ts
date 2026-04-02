import type {
  EngineAdapter,
  GameAction,
  GameEvent,
  GameState,
  LegalActionsResult,
  MatchConfig,
  PlayerId,
  SubmitResult,
} from "./types";

import { AdapterError } from "./types";
import { WasmAdapter } from "./wasm-adapter";
import type { PeerSession } from "../network/peer";
import type { P2PMessage } from "../network/protocol";

/** Events emitted by P2P adapters for UI state updates. */
export type P2PAdapterEvent =
  | { type: "playerIdentity"; playerId: PlayerId }
  | { type: "roomCreated"; roomCode: string }
  | { type: "waitingForGuest" }
  | { type: "guestConnected" }
  | { type: "opponentDisconnected"; reason: string }
  | { type: "gameOver"; winner: PlayerId | null; reason: string }
  | { type: "error"; message: string }
  | { type: "stateChanged"; state: GameState; events: GameEvent[]; legalResult: LegalActionsResult };

type P2PAdapterEventListener = (event: P2PAdapterEvent) => void;

interface DeckListPayload {
  player: { main_deck: string[]; sideboard: string[] };
  opponent: { main_deck: string[]; sideboard: string[] };
  ai_decks: Array<{ main_deck: string[]; sideboard: string[] }>;
}

/**
 * Host-side P2P adapter. Runs the WASM engine locally and relays
 * filtered state to the guest via WebRTC DataChannel.
 */
export class P2PHostAdapter implements EngineAdapter {
  private wasm = new WasmAdapter();
  private listeners: P2PAdapterEventListener[] = [];
  private messageUnsub: (() => void) | null = null;
  private disconnectUnsub: (() => void) | null = null;

  // Promise + resolver created eagerly so guest_deck messages arriving
  // before initializeGame() are captured instead of silently dropped.
  private guestDeckPromise: Promise<unknown>;
  private guestDeckResolve!: (deckData: unknown) => void;

  constructor(
    private readonly deckData: unknown,
    private readonly session: PeerSession,
    playerCount = 2,
  ) {
    if (playerCount > 2) {
      throw new AdapterError(
        "P2P_PLAYER_LIMIT",
        "P2P is only available for 2-player games. Use server mode for multiplayer.",
        false,
      );
    }
    this.guestDeckPromise = new Promise<unknown>((resolve) => {
      this.guestDeckResolve = resolve;
    });
  }

  onEvent(listener: P2PAdapterEventListener): () => void {
    this.listeners.push(listener);
    return () => {
      this.listeners = this.listeners.filter((l) => l !== listener);
    };
  }

  private emit(event: P2PAdapterEvent): void {
    for (const listener of this.listeners) {
      listener(event);
    }
  }

  async initialize(): Promise<void> {
    await this.wasm.initialize();

    // Listen for guest messages
    this.messageUnsub = this.session.onMessage((msg) => {
      this.handleGuestMessage(msg);
    });

    this.disconnectUnsub = this.session.onDisconnect((reason) => {
      this.emit({ type: "opponentDisconnected", reason });
    });
  }

  async initializeGame(
    _deckData?: unknown,
    _formatConfig?: unknown,
    _playerCount?: number,
    matchConfig?: MatchConfig,
  ): Promise<SubmitResult> {
    // Await the eagerly-created promise — resolves immediately if
    // guest_deck arrived during initialize(), otherwise waits.
    const guestDeckData = await this.guestDeckPromise;

    // Build combined deck payload for WASM
    const hostDeck = this.deckData as DeckListPayload;
    const guestDeck = guestDeckData as DeckListPayload;
    const deckPayload = {
      player: hostDeck.player,
      opponent: guestDeck.player,
      ai_decks: [],
    };

    const result = await this.wasm.initializeGame(deckPayload, undefined, 2, matchConfig);
    const legalResult = await this.wasm.getLegalActions();
    this.emit({ type: "playerIdentity", playerId: 0 });

    // Send initial state to guest (filtered) with legal actions
    const filteredState = await this.wasm.getFilteredState(1);
    this.session.send({
      type: "game_setup",
      state: filteredState,
      events: result.events,
      legalActions: legalResult.actions,
      autoPassRecommended: legalResult.autoPassRecommended,
    });

    return result;
  }

  async submitAction(action: GameAction): Promise<SubmitResult> {
    const result = await this.wasm.submitAction(action);
    const legalResult = await this.wasm.getLegalActions();

    // Send filtered state update to guest with legal actions
    const filteredState = await this.wasm.getFilteredState(1);
    this.session.send({
      type: "state_update",
      state: filteredState,
      events: result.events,
      legalActions: legalResult.actions,
      autoPassRecommended: legalResult.autoPassRecommended,
    });

    return result;
  }

  async getState(): Promise<GameState> {
    return this.wasm.getState();
  }

  async getLegalActions(): Promise<LegalActionsResult> {
    return this.wasm.getLegalActions();
  }

  getAiAction(_difficulty: string): GameAction | null {
    return null;
  }

  restoreState(_state: GameState): void {
    throw new AdapterError("P2P_ERROR", "Undo not supported in P2P games", false);
  }

  dispose(): void {
    if (this.messageUnsub) this.messageUnsub();
    if (this.disconnectUnsub) this.disconnectUnsub();
    this.session.close();
    this.wasm.dispose();
    this.listeners = [];
  }

  private async handleGuestMessage(msg: P2PMessage): Promise<void> {
    switch (msg.type) {
      case "guest_deck": {
        this.guestDeckResolve(msg.deckData);
        break;
      }

      case "action": {
        try {
          const result = await this.wasm.submitAction(msg.action);
          const state = await this.wasm.getState();
          const legalResult = await this.wasm.getLegalActions();

          // Send filtered state back to guest with legal actions
          const filteredState = await this.wasm.getFilteredState(1);
          this.session.send({
            type: "state_update",
            state: filteredState,
            events: result.events,
            legalActions: legalResult.actions,
            autoPassRecommended: legalResult.autoPassRecommended,
          });

          // Emit state update locally so host UI updates for opponent actions
          this.emit({ type: "stateChanged", state, events: result.events, legalResult });
        } catch (err) {
          const reason = err instanceof Error ? err.message : String(err);
          this.session.send({ type: "action_rejected", reason });
        }
        break;
      }

      case "concede": {
        this.emit({ type: "gameOver", winner: 0, reason: "Opponent conceded" });
        break;
      }

      default:
        break;
    }
  }
}

/**
 * Guest-side P2P adapter. Receives state from host and sends actions.
 */
export class P2PGuestAdapter implements EngineAdapter {
  private gameState: GameState | null = null;
  private legalActions: LegalActionsResult = { actions: [], autoPassRecommended: false };
  private listeners: P2PAdapterEventListener[] = [];
  private pendingResolve: ((result: SubmitResult) => void) | null = null;
  private pendingReject: ((error: Error) => void) | null = null;
  private messageUnsub: (() => void) | null = null;
  private disconnectUnsub: (() => void) | null = null;

  // Promise + resolver created eagerly so game_setup messages arriving
  // before initializeGame() are captured instead of silently dropped.
  private gameSetupPromise: Promise<SubmitResult>;
  private gameSetupResolve!: (result: SubmitResult) => void;

  constructor(
    private readonly deckData: unknown,
    private readonly session: PeerSession,
  ) {
    this.gameSetupPromise = new Promise<SubmitResult>((resolve) => {
      this.gameSetupResolve = resolve;
    });
  }

  onEvent(listener: P2PAdapterEventListener): () => void {
    this.listeners.push(listener);
    return () => {
      this.listeners = this.listeners.filter((l) => l !== listener);
    };
  }

  private emit(event: P2PAdapterEvent): void {
    for (const listener of this.listeners) {
      listener(event);
    }
  }

  async initialize(): Promise<void> {
    // Listen for host messages
    this.messageUnsub = this.session.onMessage((msg) => {
      this.handleHostMessage(msg);
    });

    this.disconnectUnsub = this.session.onDisconnect((reason) => {
      this.emit({ type: "opponentDisconnected", reason });
    });

    // Send deck data to host
    this.session.send({ type: "guest_deck", deckData: this.deckData });
  }

  async initializeGame(
    _deckData?: unknown,
    _formatConfig?: unknown,
    _playerCount?: number,
    _matchConfig?: MatchConfig,
  ): Promise<SubmitResult> {
    this.emit({ type: "playerIdentity", playerId: 1 });

    // Await the eagerly-created promise — resolves immediately if
    // game_setup arrived during initialize(), otherwise waits.
    return this.gameSetupPromise;
  }

  async submitAction(action: GameAction): Promise<SubmitResult> {
    return new Promise<SubmitResult>((resolve, reject) => {
      this.pendingResolve = resolve;
      this.pendingReject = reject;
      this.session.send({ type: "action", action });
    });
  }

  async getState(): Promise<GameState> {
    if (!this.gameState) {
      throw new AdapterError("P2P_ERROR", "No game state available", false);
    }
    return this.gameState;
  }

  async getLegalActions(): Promise<LegalActionsResult> {
    return this.legalActions;
  }

  getAiAction(_difficulty: string): GameAction | null {
    return null;
  }

  restoreState(_state: GameState): void {
    throw new AdapterError("P2P_ERROR", "Undo not supported in P2P games", false);
  }

  dispose(): void {
    if (this.messageUnsub) this.messageUnsub();
    if (this.disconnectUnsub) this.disconnectUnsub();
    this.session.close();
    this.gameState = null;
    this.legalActions = { actions: [], autoPassRecommended: false };
    this.pendingResolve = null;
    this.pendingReject = null;
    this.listeners = [];
  }

  private handleHostMessage(msg: P2PMessage): void {
    switch (msg.type) {
      case "game_setup": {
        this.gameState = msg.state;
        this.legalActions = { actions: msg.legalActions, autoPassRecommended: msg.autoPassRecommended ?? false };
        this.gameSetupResolve({ events: msg.events });
        break;
      }

      case "state_update": {
        this.gameState = msg.state;
        this.legalActions = { actions: msg.legalActions, autoPassRecommended: msg.autoPassRecommended ?? false };
        if (this.pendingResolve) {
          this.pendingResolve({ events: msg.events });
          this.pendingResolve = null;
          this.pendingReject = null;
        } else {
          // Unsolicited update (opponent's action result)
          this.emit({ type: "stateChanged", state: msg.state, events: msg.events, legalResult: { actions: msg.legalActions, autoPassRecommended: msg.autoPassRecommended ?? false } });
        }
        break;
      }

      case "action_rejected": {
        if (this.pendingReject) {
          this.pendingReject(new AdapterError("ACTION_REJECTED", msg.reason, true));
          this.pendingResolve = null;
          this.pendingReject = null;
        }
        break;
      }

      default:
        break;
    }
  }
}
