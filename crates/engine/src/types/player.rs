use std::collections::{HashMap, HashSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::events::BendingType;
use super::identifiers::ObjectId;
use super::mana::ManaPool;

use crate::game::deck_loading::DeckEntry;

/// CR 702.139: Tracks a declared companion outside the game.
/// The companion is not a `GameObject` until it moves to hand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompanionInfo {
    /// The companion card's face data for creating a GameObject when moved to hand.
    pub card: DeckEntry,
    /// CR 702.139c: Whether the companion has been put into hand this game (once per game).
    pub used: bool,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(transparent)]
pub struct PlayerId(pub u8);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    pub id: PlayerId,
    pub life: i32,
    pub mana_pool: ManaPool,

    // Per-player zones
    pub library: Vec<ObjectId>,
    pub hand: Vec<ObjectId>,
    pub graveyard: Vec<ObjectId>,

    // Tracking
    pub has_drawn_this_turn: bool,
    pub lands_played_this_turn: u8,
    pub poison_counters: u32,
    /// CR 122.1: Energy counters are a kind of counter that a player may have.
    #[serde(default)]
    pub energy: u32,
    #[serde(default)]
    pub life_gained_this_turn: u32,
    #[serde(default)]
    pub life_lost_this_turn: u32,
    #[serde(default)]
    pub descended_this_turn: bool,
    #[serde(default)]
    pub cards_drawn_this_turn: u32,
    /// CR 710.2: Number of crimes committed this turn.
    #[serde(default)]
    pub crimes_committed_this_turn: u32,
    /// CR 704.5b: Set when this player attempted to draw from an empty library.
    /// Checked by SBAs — the player loses the game.
    #[serde(default)]
    pub drew_from_empty_library: bool,

    /// Number of turns this player has taken (cumulative, never reset).
    /// Used by "your Nth turn of the game" Oracle conditions (QuantityRef::TurnsTaken).
    #[serde(default)]
    pub turns_taken: u32,

    // Elimination tracking (N-player support)
    #[serde(default)]
    pub is_eliminated: bool,

    /// Avatar crossover: which bending types this player has performed this turn.
    #[serde(default)]
    pub bending_types_this_turn: HashSet<BendingType>,

    /// CR 122.1: Generic player counters (experience, rad, ticket, etc.).
    /// Poison counters have a dedicated field due to SBA rules (CR 104.3d).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub player_counters: HashMap<String, u32>,

    /// CR 702.139: The player's declared companion (if any). Lives outside the game.
    /// Stored as card data (not a GameObject) until moved to hand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub companion: Option<CompanionInfo>,

    // Derived fields (computed in WASM bridge, not persisted)
    #[serde(skip_deserializing, default)]
    pub can_look_at_top_of_library: bool,
}

impl Default for Player {
    fn default() -> Self {
        Player {
            id: PlayerId(0),
            life: 20,
            mana_pool: ManaPool::default(),
            library: Vec::new(),
            hand: Vec::new(),
            graveyard: Vec::new(),
            has_drawn_this_turn: false,
            lands_played_this_turn: 0,
            poison_counters: 0,
            energy: 0,
            life_gained_this_turn: 0,
            life_lost_this_turn: 0,
            descended_this_turn: false,
            cards_drawn_this_turn: 0,
            crimes_committed_this_turn: 0,
            drew_from_empty_library: false,
            turns_taken: 0,
            is_eliminated: false,
            bending_types_this_turn: HashSet::new(),
            player_counters: HashMap::new(),
            companion: None,
            can_look_at_top_of_library: false,
        }
    }
}

impl Player {
    /// CR 122.1: Get the current count of a named player counter.
    /// Poison counters use the dedicated field; all others use the generic map.
    pub fn player_counter(&self, kind: &str) -> u32 {
        if kind == "poison" {
            self.poison_counters
        } else {
            self.player_counters.get(kind).copied().unwrap_or(0)
        }
    }

    /// CR 122.1: Add counters of a named type to this player.
    /// Poison counters use the dedicated field (has SBA at CR 104.3d); all others use the generic map.
    pub fn add_player_counters(&mut self, kind: &str, count: u32) {
        if kind == "poison" {
            self.poison_counters += count;
        } else {
            *self.player_counters.entry(kind.to_string()).or_insert(0) += count;
        }
    }
}
