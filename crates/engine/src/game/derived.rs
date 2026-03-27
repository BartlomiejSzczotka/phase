use crate::game::combat::has_summoning_sickness;
use crate::game::coverage::unimplemented_mechanics;
use crate::game::devotion::count_devotion;
use crate::game::mana_abilities;
use crate::game::mana_sources::display_land_mana_colors;
use crate::game::static_abilities::{check_static_ability, StaticCheckContext};
use crate::types::ability::StaticCondition;
use crate::types::card_type::CoreType;
use crate::types::game_state::GameState;
use crate::types::statics::StaticMode;

/// Compute display-only derived fields (CR 302.6 summoning sickness, CR 700.5 devotion).
///
/// This must be called by any consumer (WASM, Tauri, server) before
/// serializing the state to the frontend. It sets:
/// - `GameObject::unimplemented_mechanics`
/// - `GameObject::has_summoning_sickness`
/// - `GameObject::devotion` (for Theros gods pattern)
/// - `GameObject::commander_tax` (CR 903.8 commander tax)
/// - `Player::can_look_at_top_of_library`
pub fn derive_display_state(state: &mut GameState) {
    let turn = state.turn_number;

    for obj in state.objects.values_mut() {
        obj.unimplemented_mechanics = unimplemented_mechanics(obj);
        obj.has_summoning_sickness = has_summoning_sickness(obj, turn);
        let mana_idx = obj
            .abilities
            .iter()
            .position(mana_abilities::is_mana_ability);
        obj.has_mana_ability = mana_idx.is_some();
        obj.mana_ability_index = mana_idx;
        obj.available_mana_colors.clear();
    }

    // Compute per-card devotion for cards with DevotionGE conditions
    // (Theros gods pattern — derive colors from the card's own base_color)
    let devotion_cards: Vec<_> = state
        .objects
        .iter()
        .filter_map(|(&id, obj)| {
            let has_devotion_static =
                obj.static_definitions
                    .iter()
                    .any(|def| match &def.condition {
                        Some(StaticCondition::DevotionGE { .. }) => true,
                        Some(StaticCondition::Not { condition })
                            if matches!(condition.as_ref(), StaticCondition::DevotionGE { .. }) =>
                        {
                            true
                        }
                        _ => false,
                    });
            if has_devotion_static && !obj.base_color.is_empty() {
                let devotion = count_devotion(state, obj.controller, &obj.base_color);
                Some((id, devotion))
            } else {
                None
            }
        })
        .collect();
    for (id, devotion) in devotion_cards {
        if let Some(obj) = state.objects.get_mut(&id) {
            obj.devotion = Some(devotion);
        }
    }

    // CR 903.8: Compute commander tax for display.
    let commander_taxes: Vec<_> = state
        .objects
        .iter()
        .filter_map(|(&id, obj)| {
            if obj.is_commander {
                Some((id, super::commander::commander_tax(state, id)))
            } else {
                None
            }
        })
        .collect();
    for (id, tax) in commander_taxes {
        if let Some(obj) = state.objects.get_mut(&id) {
            obj.commander_tax = Some(tax);
        }
    }

    // Compute dynamic land frame colors from currently available mana options.
    let mana_color_cards: Vec<_> = state
        .battlefield
        .iter()
        .filter_map(|&id| {
            let obj = state.objects.get(&id)?;
            if !obj.card_types.core_types.contains(&CoreType::Land) {
                return None;
            }
            let colors = display_land_mana_colors(state, id, obj.controller);
            Some((id, colors))
        })
        .collect();
    for (id, colors) in mana_color_cards {
        if let Some(obj) = state.objects.get_mut(&id) {
            obj.available_mana_colors = colors;
        }
    }

    // Compute per-player derived fields
    let peek_flags: Vec<bool> = state
        .players
        .iter()
        .map(|p| {
            let ctx = StaticCheckContext {
                player_id: Some(p.id),
                ..Default::default()
            };
            check_static_ability(state, StaticMode::MayLookAtTopOfLibrary, &ctx)
        })
        .collect();
    for (i, flag) in peek_flags.into_iter().enumerate() {
        state.players[i].can_look_at_top_of_library = flag;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::zones::create_object;
    use crate::types::identifiers::CardId;
    use crate::types::player::PlayerId;
    use crate::types::zones::Zone;

    #[test]
    fn derive_sets_summoning_sickness_for_new_creature() {
        let mut state = GameState::new_two_player(42);
        state.turn_number = 1;
        let id = create_object(
            &mut state,
            CardId(1),
            PlayerId(0),
            "Bear".to_string(),
            Zone::Battlefield,
        );
        state
            .objects
            .get_mut(&id)
            .unwrap()
            .card_types
            .core_types
            .push(crate::types::card_type::CoreType::Creature);
        state.objects.get_mut(&id).unwrap().entered_battlefield_turn = Some(1);

        derive_display_state(&mut state);

        assert!(state.objects[&id].has_summoning_sickness);
    }

    #[test]
    fn derive_clears_summoning_sickness_for_old_creature() {
        let mut state = GameState::new_two_player(42);
        state.turn_number = 3;
        let id = create_object(
            &mut state,
            CardId(1),
            PlayerId(0),
            "Bear".to_string(),
            Zone::Battlefield,
        );
        state
            .objects
            .get_mut(&id)
            .unwrap()
            .card_types
            .core_types
            .push(crate::types::card_type::CoreType::Creature);
        state.objects.get_mut(&id).unwrap().entered_battlefield_turn = Some(1);

        derive_display_state(&mut state);

        assert!(!state.objects[&id].has_summoning_sickness);
    }

    #[test]
    fn derive_sets_unimplemented_flag() {
        let mut state = GameState::new_two_player(42);
        let id = create_object(
            &mut state,
            CardId(1),
            PlayerId(0),
            "Test".to_string(),
            Zone::Battlefield,
        );

        derive_display_state(&mut state);

        // Should have set the flag (false for a card with no mechanics)
        let obj = &state.objects[&id];
        assert!(obj.unimplemented_mechanics.is_empty());
    }

    #[test]
    fn derive_sets_can_look_at_top_default_false() {
        let mut state = GameState::new_two_player(42);

        derive_display_state(&mut state);

        assert!(!state.players[0].can_look_at_top_of_library);
        assert!(!state.players[1].can_look_at_top_of_library);
    }

    #[test]
    fn derive_sets_commander_tax_for_commander() {
        use crate::game::commander::record_commander_cast;

        let mut state = GameState::new_two_player(42);
        let id = create_object(
            &mut state,
            CardId(1),
            PlayerId(0),
            "Commander".to_string(),
            Zone::Command,
        );
        state.objects.get_mut(&id).unwrap().is_commander = true;

        // No casts yet — tax should be 0
        derive_display_state(&mut state);
        assert_eq!(state.objects[&id].commander_tax, Some(0));

        // After 2 casts — tax should be 4
        record_commander_cast(&mut state, id);
        record_commander_cast(&mut state, id);
        derive_display_state(&mut state);
        assert_eq!(state.objects[&id].commander_tax, Some(4));
    }

    #[test]
    fn derive_does_not_set_commander_tax_for_non_commander() {
        let mut state = GameState::new_two_player(42);
        let id = create_object(
            &mut state,
            CardId(1),
            PlayerId(0),
            "Bear".to_string(),
            Zone::Battlefield,
        );

        derive_display_state(&mut state);
        assert_eq!(state.objects[&id].commander_tax, None);
    }
}
