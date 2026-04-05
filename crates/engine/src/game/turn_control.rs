use crate::types::game_state::GameState;
use crate::types::player::PlayerId;

pub fn turn_resource_owner(state: &GameState) -> PlayerId {
    state.active_player
}

pub fn turn_decision_maker(state: &GameState) -> PlayerId {
    state
        .turn_decision_controller
        .unwrap_or(state.active_player)
}

pub fn authorized_submitter_for_player(state: &GameState, semantic_player: PlayerId) -> PlayerId {
    if semantic_player == state.active_player {
        turn_decision_maker(state)
    } else {
        semantic_player
    }
}

pub fn authorized_submitter(state: &GameState) -> Option<PlayerId> {
    state
        .waiting_for
        .acting_player()
        .map(|player| authorized_submitter_for_player(state, player))
}

pub fn viewer_controls_active_turn(state: &GameState, viewer: PlayerId) -> bool {
    state.turn_decision_controller == Some(viewer)
}
