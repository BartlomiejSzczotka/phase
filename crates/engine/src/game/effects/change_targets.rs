use crate::game::targeting::find_legal_targets;
use crate::types::ability::{
    Effect, EffectError, EffectKind, ResolvedAbility, TargetFilter, TargetRef,
};
use crate::types::events::GameEvent;
use crate::types::game_state::{GameState, WaitingFor};

/// CR 115.7: Change the target(s) of a spell or ability on the stack.
///
/// Resolves in two modes:
/// - `forced_to` is `Some`: directly update the stack entry's targets to the resolved target.
/// - `forced_to` is `None`: set `WaitingFor::RetargetChoice` so the player selects the new target.
pub fn resolve(
    state: &mut GameState,
    ability: &ResolvedAbility,
    events: &mut Vec<GameEvent>,
) -> Result<(), EffectError> {
    let Effect::ChangeTargets {
        scope, forced_to, ..
    } = &ability.effect
    else {
        return Err(EffectError::MissingParam(
            "ChangeTargets effect missing".to_string(),
        ));
    };

    // ability.targets[0] is the TargetRef::Object(id) of the stack entry being retargeted.
    let stack_entry_id = match ability.targets.first() {
        Some(TargetRef::Object(id)) => *id,
        _ => {
            return Err(EffectError::MissingParam(
                "ChangeTargets requires a stack entry target".to_string(),
            ))
        }
    };

    // CR 115.7: Find the stack entry by its object ID.
    let stack_entry_index = state
        .stack
        .iter()
        .position(|e| e.id == stack_entry_id)
        .ok_or_else(|| {
            EffectError::MissingParam("ChangeTargets: targeted entry not on stack".to_string())
        })?;

    if let Some(filter) = forced_to {
        // CR 115.7: Forced retarget — resolve the new target from the filter and apply directly.
        let new_targets = find_legal_targets(state, filter, ability.controller, ability.source_id);
        if let Some(new_target) = new_targets.into_iter().next() {
            if let Some(stack_ability) = state.stack[stack_entry_index].ability_mut() {
                stack_ability.targets = vec![new_target];
            }
        }
        events.push(GameEvent::EffectResolved {
            kind: EffectKind::from(&ability.effect),
            source_id: ability.source_id,
        });
        return Ok(());
    }

    // Interactive retarget: present choices to the player.
    // CR 115.7a: The current targets of the targeted spell/ability become the starting point.
    let Some(stack_ability) = state.stack[stack_entry_index].ability().cloned() else {
        // Permanent spell with no ability — nothing to retarget.
        events.push(GameEvent::EffectResolved {
            kind: EffectKind::from(&ability.effect),
            source_id: ability.source_id,
        });
        return Ok(());
    };
    let current_targets = stack_ability.targets.clone();

    // CR 115.7: Enumerate legal new targets by extracting the target filter from
    // the stack entry's effect and re-evaluating against the current game state.
    let legal_new_targets = extract_target_filter(&stack_ability.effect)
        .map(|filter| {
            find_legal_targets(
                state,
                filter,
                stack_ability.controller,
                stack_ability.source_id,
            )
        })
        .unwrap_or_else(|| current_targets.clone());

    state.waiting_for = WaitingFor::RetargetChoice {
        player: ability.controller,
        stack_entry_index,
        scope: scope.clone(),
        current_targets,
        legal_new_targets,
    };
    // EffectResolved is emitted by the engine handler after RetargetSpell action is submitted.
    Ok(())
}

/// Extract the target filter from an effect variant, if it has a standard `target` field.
/// Used to compute legal alternative targets for retargeting (CR 115.7).
fn extract_target_filter(effect: &Effect) -> Option<&TargetFilter> {
    effect.target_filter()
}
