use crate::game::quantity::resolve_quantity_with_targets;
use crate::game::speed::{increase_speed, set_speed};
use crate::types::ability::{Effect, EffectError, PlayerFilter, ResolvedAbility};
use crate::types::events::GameEvent;
use crate::types::game_state::GameState;
use crate::types::player::PlayerId;

fn players_for_filter(
    state: &GameState,
    filter: &PlayerFilter,
    controller: PlayerId,
    source_id: crate::types::identifiers::ObjectId,
) -> Vec<PlayerId> {
    state
        .players
        .iter()
        .filter(|player| {
            crate::game::players::matches_scope_filter(
                state, player.id, filter, controller, source_id,
            )
        })
        .map(|player| player.id)
        .collect()
}

/// CR 702.179a: Effects that instruct players to start their engines set speed to 1
/// only if the player currently has no speed.
pub fn resolve_start(
    state: &mut GameState,
    ability: &ResolvedAbility,
    events: &mut Vec<GameEvent>,
) -> Result<(), EffectError> {
    let Effect::StartYourEngines { player_scope } = &ability.effect else {
        return Err(EffectError::InvalidParam(
            "expected StartYourEngines".to_string(),
        ));
    };

    for player_id in players_for_filter(state, player_scope, ability.controller, ability.source_id)
    {
        let has_no_speed = state
            .players
            .iter()
            .find(|player| player.id == player_id)
            .is_some_and(|player| player.speed.is_none());
        if has_no_speed {
            set_speed(state, player_id, Some(1), events);
        }
    }

    Ok(())
}

/// CR 702.179c-d: Increase speed by the resolved amount for each selected player.
pub fn resolve_increase(
    state: &mut GameState,
    ability: &ResolvedAbility,
    events: &mut Vec<GameEvent>,
) -> Result<(), EffectError> {
    let Effect::IncreaseSpeed {
        player_scope,
        amount,
    } = &ability.effect
    else {
        return Err(EffectError::InvalidParam(
            "expected IncreaseSpeed".to_string(),
        ));
    };

    let amount = resolve_quantity_with_targets(state, amount, ability);
    let amount = u8::try_from(amount.max(0)).unwrap_or(u8::MAX);
    if amount == 0 {
        return Ok(());
    }

    for player_id in players_for_filter(state, player_scope, ability.controller, ability.source_id)
    {
        increase_speed(state, player_id, amount, events);
    }

    Ok(())
}
