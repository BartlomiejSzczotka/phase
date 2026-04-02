use crate::types::game_state::{GameState, PublicStateDirty, WaitingFor};
use crate::types::identifiers::ObjectId;
use crate::types::player::PlayerId;

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
    clear_public_state_dirty(state);
}

pub fn sync_waiting_for(state: &mut GameState, waiting_for: &WaitingFor) {
    state.waiting_for = waiting_for.clone();
    if let WaitingFor::Priority { player } = waiting_for {
        state.priority_player = *player;
    }
}

pub fn mark_public_state_all_dirty(state: &mut GameState) {
    state.public_state_dirty = PublicStateDirty::all_dirty();
}

pub fn mark_public_state_object_dirty(state: &mut GameState, object_id: ObjectId) {
    if !state.public_state_dirty.all_objects_dirty {
        state.public_state_dirty.dirty_objects.insert(object_id);
    }
}

pub fn mark_public_state_player_dirty(state: &mut GameState, player_id: PlayerId) {
    if !state.public_state_dirty.all_players_dirty {
        state.public_state_dirty.dirty_players.insert(player_id);
    }
}

pub fn mark_battlefield_display_dirty(state: &mut GameState) {
    state.public_state_dirty.battlefield_display_dirty = true;
}

pub fn mark_mana_display_dirty(state: &mut GameState) {
    state.public_state_dirty.mana_display_dirty = true;
}

pub fn bump_state_revision(state: &mut GameState) {
    state.state_revision = state.state_revision.wrapping_add(1);
}

pub fn clear_public_state_dirty(state: &mut GameState) {
    state.public_state_dirty = PublicStateDirty::default();
}
