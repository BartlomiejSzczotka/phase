use std::collections::HashSet;

use crate::game::quantity::resolve_quantity_with_targets;
use crate::game::replacement::{self, ReplacementResult};
use crate::types::ability::{
    Effect, EffectError, EffectKind, GainLifePlayer, ResolvedAbility, TargetRef,
};
use crate::types::events::GameEvent;
use crate::types::game_state::GameState;
use crate::types::player::PlayerId;
use crate::types::proposed_event::ProposedEvent;

/// Signals that a replacement effect requires player choice before the event can proceed.
/// Callers receiving this must yield control (the engine will re-enter after the choice).
#[derive(Debug)]
pub struct ReplacementDeferred;

/// CR 119.1: Gain life — increase the player's life total.
pub fn resolve_gain(
    state: &mut GameState,
    ability: &ResolvedAbility,
    events: &mut Vec<GameEvent>,
) -> Result<(), EffectError> {
    let (amount, player_kind) = match &ability.effect {
        Effect::GainLife { amount, player } => (amount, player),
        _ => return Err(EffectError::MissingParam("GainLife amount".to_string())),
    };

    // Resolve the target object (if any) for TargetedController.
    let target_obj = ability.targets.iter().find_map(|t| {
        if let TargetRef::Object(id) = t {
            state.objects.get(id)
        } else {
            None
        }
    });

    let player_id: PlayerId = match player_kind {
        GainLifePlayer::TargetedController => target_obj
            .map(|o| o.controller)
            .unwrap_or(ability.controller),
        GainLifePlayer::Controller => ability.controller,
    };

    let final_amount = resolve_quantity_with_targets(state, amount, ability);

    if final_amount <= 0 {
        events.push(GameEvent::EffectResolved {
            kind: EffectKind::from(&ability.effect),
            source_id: ability.source_id,
        });
        return Ok(());
    }

    let proposed = ProposedEvent::LifeGain {
        player_id,
        amount: final_amount as u32,
        applied: HashSet::new(),
    };

    // CR 614.1a: Route life gain through replacement pipeline.
    match replacement::replace_event(state, proposed, events) {
        ReplacementResult::Execute(event) => {
            if let ProposedEvent::LifeGain {
                player_id,
                amount: gain_amount,
                ..
            } = event
            {
                let player = state
                    .players
                    .iter_mut()
                    .find(|p| p.id == player_id)
                    .ok_or(EffectError::PlayerNotFound)?;
                player.life += gain_amount as i32;
                // CR 119.9: Track life gained this turn for triggered ability matching.
                player.life_gained_this_turn += gain_amount;
                state.layers_dirty = true;

                events.push(GameEvent::LifeChanged {
                    player_id,
                    amount: gain_amount as i32,
                });
            }
        }
        ReplacementResult::Prevented => {}
        ReplacementResult::NeedsChoice(player) => {
            // TODO(CR 614.7): When multiple replacement effects apply to life gain, controller should choose which applies first. Currently falls through unconditionally.
            state.waiting_for =
                crate::game::replacement::replacement_choice_waiting_for(player, state);
            return Ok(());
        }
    }

    events.push(GameEvent::EffectResolved {
        kind: EffectKind::from(&ability.effect),
        source_id: ability.source_id,
    });

    Ok(())
}

/// Apply life gain, running through the replacement pipeline.
/// Returns the actual amount of life gained (may differ due to replacements like Leyline of Hope).
/// Returns `Err(ReplacementDeferred)` when multiple replacement effects compete and
/// the player must choose which applies first (CR 614.7).
pub fn apply_life_gain(
    state: &mut GameState,
    player_id: PlayerId,
    amount: u32,
    events: &mut Vec<GameEvent>,
) -> Result<u32, ReplacementDeferred> {
    if amount == 0 {
        return Ok(0);
    }
    let proposed = ProposedEvent::LifeGain {
        player_id,
        amount,
        applied: HashSet::new(),
    };
    match replacement::replace_event(state, proposed, events) {
        ReplacementResult::Execute(event) => {
            if let ProposedEvent::LifeGain {
                player_id: pid,
                amount: gain_amount,
                ..
            } = event
            {
                if let Some(player) = state.players.iter_mut().find(|p| p.id == pid) {
                    player.life += gain_amount as i32;
                    player.life_gained_this_turn += gain_amount;
                }
                state.layers_dirty = true;
                events.push(GameEvent::LifeChanged {
                    player_id: pid,
                    amount: gain_amount as i32,
                });
                Ok(gain_amount)
            } else {
                Ok(0)
            }
        }
        ReplacementResult::Prevented => Ok(0),
        ReplacementResult::NeedsChoice(player) => {
            // CR 614.7: Multiple competing replacements — player must choose.
            state.waiting_for =
                crate::game::replacement::replacement_choice_waiting_for(player, state);
            Err(ReplacementDeferred)
        }
    }
}

/// CR 120.3: Damage to a player causes that much life loss.
/// Returns the actual amount of life lost (may differ due to replacements like doubling).
/// Returns `Err(ReplacementDeferred)` when multiple replacement effects compete and
/// the player must choose which applies first (CR 614.7).
pub fn apply_damage_life_loss(
    state: &mut GameState,
    player_id: PlayerId,
    amount: u32,
    events: &mut Vec<GameEvent>,
) -> Result<u32, ReplacementDeferred> {
    if amount == 0 {
        return Ok(0);
    }
    let proposed = ProposedEvent::LifeLoss {
        player_id,
        amount,
        applied: HashSet::new(),
    };
    match replacement::replace_event(state, proposed, events) {
        ReplacementResult::Execute(event) => {
            if let ProposedEvent::LifeLoss {
                player_id: pid,
                amount: loss_amount,
                ..
            } = event
            {
                if let Some(player) = state.players.iter_mut().find(|p| p.id == pid) {
                    player.life -= loss_amount as i32;
                    player.life_lost_this_turn += loss_amount;
                }
                state.layers_dirty = true;
                events.push(GameEvent::LifeChanged {
                    player_id: pid,
                    amount: -(loss_amount as i32),
                });
                Ok(loss_amount)
            } else {
                Ok(0)
            }
        }
        ReplacementResult::Prevented => Ok(0),
        ReplacementResult::NeedsChoice(player) => {
            // CR 614.7: Multiple competing replacements — player must choose.
            state.waiting_for =
                crate::game::replacement::replacement_choice_waiting_for(player, state);
            Err(ReplacementDeferred)
        }
    }
}

/// CR 119.3: If an effect causes a player to lose life, adjust their life total.
pub fn resolve_lose(
    state: &mut GameState,
    ability: &ResolvedAbility,
    events: &mut Vec<GameEvent>,
) -> Result<(), EffectError> {
    let amount: i32 = match &ability.effect {
        Effect::LoseLife { amount } => {
            crate::game::quantity::resolve_quantity_with_targets(state, amount, ability)
        }
        _ => return Err(EffectError::MissingParam("LoseLife amount".to_string())),
    };

    // Determine target player: use first player target or fall back to controller
    let target_player_id = ability
        .targets
        .iter()
        .find_map(|t| {
            if let TargetRef::Player(pid) = t {
                Some(*pid)
            } else {
                None
            }
        })
        .unwrap_or(ability.controller);

    let proposed = ProposedEvent::LifeLoss {
        player_id: target_player_id,
        amount: amount as u32,
        applied: HashSet::new(),
    };

    match replacement::replace_event(state, proposed, events) {
        ReplacementResult::Execute(event) => {
            if let ProposedEvent::LifeLoss {
                player_id,
                amount: loss_amount,
                ..
            } = event
            {
                let player = state
                    .players
                    .iter_mut()
                    .find(|p| p.id == player_id)
                    .ok_or(EffectError::PlayerNotFound)?;
                player.life -= loss_amount as i32;
                player.life_lost_this_turn += loss_amount;
                state.layers_dirty = true;

                events.push(GameEvent::LifeChanged {
                    player_id,
                    amount: -(loss_amount as i32),
                });
            }
        }
        ReplacementResult::Prevented => {}
        ReplacementResult::NeedsChoice(player) => {
            state.waiting_for =
                crate::game::replacement::replacement_choice_waiting_for(player, state);
            return Ok(());
        }
    }

    events.push(GameEvent::EffectResolved {
        kind: EffectKind::from(&ability.effect),
        source_id: ability.source_id,
    });

    Ok(())
}

/// CR 119.5: Set a player's life total to a specific number.
/// The player gains or loses the necessary amount of life to reach the target.
pub fn resolve_set_life_total(
    state: &mut GameState,
    ability: &ResolvedAbility,
    events: &mut Vec<GameEvent>,
) -> Result<(), EffectError> {
    let amount = match &ability.effect {
        Effect::SetLifeTotal { amount, .. } => {
            crate::game::quantity::resolve_quantity_with_targets(state, amount, ability)
        }
        _ => return Err(EffectError::MissingParam("SetLifeTotal amount".to_string())),
    };

    let target_player_id = ability
        .targets
        .iter()
        .find_map(|t| {
            if let TargetRef::Player(pid) = t {
                Some(*pid)
            } else {
                None
            }
        })
        .unwrap_or(ability.controller);

    let player = state
        .players
        .iter_mut()
        .find(|p| p.id == target_player_id)
        .ok_or(EffectError::PlayerNotFound)?;

    let current_life = player.life;
    let diff = amount - current_life;

    // CR 119.5: If the new total is higher, the player gains the difference.
    // If lower, the player loses the difference.
    if diff > 0 {
        player.life = amount;
        player.life_gained_this_turn += diff as u32;
    } else if diff < 0 {
        player.life = amount;
        player.life_lost_this_turn += (-diff) as u32;
    }
    state.layers_dirty = true;

    if diff != 0 {
        events.push(GameEvent::LifeChanged {
            player_id: target_player_id,
            amount: diff,
        });
    }

    events.push(GameEvent::EffectResolved {
        kind: EffectKind::from(&ability.effect),
        source_id: ability.source_id,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ability::{QuantityExpr, TargetRef};
    use crate::types::identifiers::ObjectId;
    use crate::types::player::PlayerId;

    #[test]
    fn gain_life_increases_controller_life() {
        let mut state = GameState::new_two_player(42);
        let ability = ResolvedAbility::new(
            Effect::GainLife {
                amount: QuantityExpr::Fixed { value: 5 },
                player: GainLifePlayer::Controller,
            },
            vec![],
            ObjectId(100),
            PlayerId(0),
        );
        let mut events = Vec::new();

        resolve_gain(&mut state, &ability, &mut events).unwrap();

        assert_eq!(state.players[0].life, 25);
    }

    #[test]
    fn lose_life_decreases_target_life() {
        let mut state = GameState::new_two_player(42);
        let ability = ResolvedAbility::new(
            Effect::LoseLife {
                amount: QuantityExpr::Fixed { value: 3 },
            },
            vec![TargetRef::Player(PlayerId(1))],
            ObjectId(100),
            PlayerId(0),
        );
        let mut events = Vec::new();

        resolve_lose(&mut state, &ability, &mut events).unwrap();

        assert_eq!(state.players[1].life, 17);
    }

    #[test]
    fn gain_life_emits_positive_life_changed() {
        let mut state = GameState::new_two_player(42);
        let ability = ResolvedAbility::new(
            Effect::GainLife {
                amount: QuantityExpr::Fixed { value: 4 },
                player: GainLifePlayer::Controller,
            },
            vec![],
            ObjectId(100),
            PlayerId(0),
        );
        let mut events = Vec::new();

        resolve_gain(&mut state, &ability, &mut events).unwrap();

        assert!(events
            .iter()
            .any(|e| matches!(e, GameEvent::LifeChanged { amount, .. } if *amount == 4)));
    }

    #[test]
    fn lose_life_emits_negative_life_changed() {
        let mut state = GameState::new_two_player(42);
        let ability = ResolvedAbility::new(
            Effect::LoseLife {
                amount: QuantityExpr::Fixed { value: 2 },
            },
            vec![TargetRef::Player(PlayerId(0))],
            ObjectId(100),
            PlayerId(0),
        );
        let mut events = Vec::new();

        resolve_lose(&mut state, &ability, &mut events).unwrap();

        assert!(events
            .iter()
            .any(|e| matches!(e, GameEvent::LifeChanged { amount, .. } if *amount == -2)));
    }
}
