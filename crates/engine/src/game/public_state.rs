use crate::types::game_state::{GameState, WaitingFor};

use super::derived::derive_display_state;
use super::layers::evaluate_layers;

/// Finalize outward-facing game state before it leaves the engine boundary.
///
/// This is the single authoritative place that synchronizes `priority_player`
/// from `waiting_for`, evaluates layers when dirty, and derives display-only
/// state used by the frontend.
pub fn finalize_public_state(state: &mut GameState) {
    if let WaitingFor::Priority { player } = state.waiting_for {
        state.priority_player = player;
    }
    if state.layers_dirty {
        evaluate_layers(state);
    }
    derive_display_state(state);
}

pub fn sync_waiting_for(state: &mut GameState, waiting_for: &WaitingFor) {
    state.waiting_for = waiting_for.clone();
    if let WaitingFor::Priority { player } = waiting_for {
        state.priority_player = *player;
    }
}
