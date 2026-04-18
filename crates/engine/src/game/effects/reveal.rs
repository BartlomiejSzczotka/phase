use crate::types::ability::{
    Effect, EffectError, EffectKind, ResolvedAbility, TargetFilter, TargetRef,
};
use crate::types::events::GameEvent;
use crate::types::game_state::GameState;
use crate::types::identifiers::ObjectId;

/// CR 701.20: Reveal a specific object to all players.
///
/// Scope: only `TargetFilter::SelfRef` (resolves to `ability.source_id`) and
/// pre-resolved `TargetRef::Object` targets are supported. Other filter shapes
/// (e.g., `TargetFilter::Typed`) would require routing through the general
/// target-resolution pipeline and are intentionally not handled here — the parser
/// only emits `Effect::Reveal { target: SelfRef }` today. Extend this resolver
/// (and add parser coverage) before introducing other target shapes.
///
/// Emits a single `GameEvent::CardsRevealed` carrying all revealed card ids and names.
///
/// Per CR 701.20b, revealing a card does not cause it to change zones or otherwise
/// move — this resolver is read-only against game state.
///
/// Timing note (used by shuffle-back replacements per CR 614 + 701.20): when this
/// runs as a post-replacement effect after a redirected ZoneChange, the card has
/// already landed in its owner's library. The emitted event carries both
/// `card_ids` and `card_names`, so observers see which card caused the shuffle-back
/// regardless of the current zone.
pub fn resolve(
    state: &mut GameState,
    ability: &ResolvedAbility,
    events: &mut Vec<GameEvent>,
) -> Result<(), EffectError> {
    let target = match &ability.effect {
        Effect::Reveal { target } => target.clone(),
        _ => TargetFilter::SelfRef,
    };

    let object_ids: Vec<ObjectId> = ability
        .targets
        .iter()
        .filter_map(|t| match t {
            TargetRef::Object(id) => Some(*id),
            _ => None,
        })
        .collect();

    let object_ids = if object_ids.is_empty() && matches!(target, TargetFilter::SelfRef) {
        vec![ability.source_id]
    } else {
        object_ids
    };

    if !object_ids.is_empty() {
        let card_names: Vec<String> = object_ids
            .iter()
            .filter_map(|id| state.objects.get(id).map(|o| o.name.clone()))
            .collect();

        events.push(GameEvent::CardsRevealed {
            player: ability.controller,
            card_ids: object_ids,
            card_names,
        });
    }

    events.push(GameEvent::EffectResolved {
        kind: EffectKind::Reveal,
        source_id: ability.source_id,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::zones::create_object;
    use crate::types::identifiers::CardId;
    use crate::types::player::PlayerId;
    use crate::types::zones::Zone;

    #[test]
    fn reveal_self_ref_emits_cards_revealed_with_source_object() {
        let mut state = GameState::new_two_player(42);
        let obj = create_object(
            &mut state,
            CardId(1),
            PlayerId(0),
            "Nexus of Fate".to_string(),
            Zone::Library,
        );

        let ability = ResolvedAbility::new(
            Effect::Reveal {
                target: TargetFilter::SelfRef,
            },
            vec![],
            obj,
            PlayerId(0),
        );

        let mut events = Vec::new();
        resolve(&mut state, &ability, &mut events).unwrap();

        let revealed = events.iter().find_map(|e| match e {
            GameEvent::CardsRevealed {
                player,
                card_ids,
                card_names,
            } => Some((*player, card_ids.clone(), card_names.clone())),
            _ => None,
        });

        let (player, card_ids, card_names) = revealed.expect("CardsRevealed emitted");
        assert_eq!(player, PlayerId(0));
        assert_eq!(card_ids, vec![obj]);
        assert_eq!(card_names, vec!["Nexus of Fate".to_string()]);
    }

    #[test]
    fn reveal_does_not_mutate_game_state() {
        let mut state = GameState::new_two_player(42);
        let obj = create_object(
            &mut state,
            CardId(1),
            PlayerId(0),
            "Progenitus".to_string(),
            Zone::Graveyard,
        );

        let before_revealed = state.revealed_cards.clone();
        let before_zones = state
            .objects
            .get(&obj)
            .map(|o| (o.zone, o.owner, o.controller));

        let ability = ResolvedAbility::new(
            Effect::Reveal {
                target: TargetFilter::SelfRef,
            },
            vec![],
            obj,
            PlayerId(0),
        );

        let mut events = Vec::new();
        resolve(&mut state, &ability, &mut events).unwrap();

        // CR 701.20b: revealing does not change zones or mutate state.
        assert_eq!(state.revealed_cards, before_revealed);
        assert_eq!(
            state
                .objects
                .get(&obj)
                .map(|o| (o.zone, o.owner, o.controller)),
            before_zones
        );
    }
}
