use std::collections::HashSet;

use crate::types::game_state::{GameState, WaitingFor};
use crate::types::identifiers::ObjectId;
use crate::types::player::PlayerId;
use crate::types::zones::Zone;

use super::players;

/// Returns a filtered copy of the game state for the given viewer.
/// Hides all opponents' hand contents and all library contents except where the
/// viewer is explicitly allowed to see them.
pub fn filter_state_for_viewer(state: &GameState, viewer: PlayerId) -> GameState {
    let mut filtered = state.clone();

    let opponents = players::opponents(state, viewer);
    let opp_hand_ids: Vec<ObjectId> = opponents
        .iter()
        .flat_map(|&opp| filtered.players[opp.0 as usize].hand.iter().copied())
        .collect();
    for obj_id in opp_hand_ids {
        if !state.revealed_cards.contains(&obj_id) {
            hide_card(&mut filtered, obj_id);
        }
    }

    let (manifest_dread_visible, manifest_dread_cards): (HashSet<ObjectId>, HashSet<ObjectId>) =
        if let WaitingFor::ManifestDreadChoice { player, ref cards } = filtered.waiting_for {
            let all_cards: HashSet<ObjectId> = cards.iter().copied().collect();
            if player == viewer {
                (all_cards.clone(), all_cards)
            } else {
                (HashSet::new(), all_cards)
            }
        } else {
            (HashSet::new(), HashSet::new())
        };

    let (dig_visible, dig_cards): (HashSet<ObjectId>, HashSet<ObjectId>) =
        if let WaitingFor::DigChoice {
            player, ref cards, ..
        } = filtered.waiting_for
        {
            let all_cards: HashSet<ObjectId> = cards.iter().copied().collect();
            if player == viewer {
                (all_cards.clone(), all_cards)
            } else {
                (HashSet::new(), all_cards)
            }
        } else {
            (HashSet::new(), HashSet::new())
        };

    let effect_zone_hand_cards: HashSet<ObjectId> = if let WaitingFor::EffectZoneChoice {
        zone: Zone::Hand,
        ref cards,
        ..
    } = filtered.waiting_for
    {
        cards.iter().copied().collect()
    } else {
        HashSet::new()
    };

    let all_library_ids: Vec<ObjectId> = filtered
        .players
        .iter()
        .flat_map(|p| p.library.iter().copied())
        .collect();
    for obj_id in all_library_ids {
        let visible = manifest_dread_visible.contains(&obj_id)
            || dig_visible.contains(&obj_id)
            || (state.revealed_cards.contains(&obj_id)
                && !manifest_dread_cards.contains(&obj_id)
                && !dig_cards.contains(&obj_id));
        if !visible && !effect_zone_hand_cards.contains(&obj_id) {
            hide_card(&mut filtered, obj_id);
        }
    }

    if let WaitingFor::ManifestDreadChoice { player, ref cards } = state.waiting_for {
        if player != viewer {
            filtered.waiting_for = WaitingFor::ManifestDreadChoice {
                player,
                cards: cards.iter().map(|_| ObjectId(0)).collect(),
            };
        }
    }

    if let WaitingFor::DigChoice {
        player,
        ref cards,
        keep_count,
        up_to,
        ref selectable_cards,
        kept_destination,
        rest_destination,
        source_id,
    } = state.waiting_for
    {
        if player != viewer {
            filtered.waiting_for = WaitingFor::DigChoice {
                player,
                cards: cards.iter().map(|_| ObjectId(0)).collect(),
                keep_count,
                up_to,
                selectable_cards: selectable_cards.iter().map(|_| ObjectId(0)).collect(),
                kept_destination,
                rest_destination,
                source_id,
            };
        }
    }

    if let WaitingFor::LearnChoice {
        player,
        ref hand_cards,
    } = state.waiting_for
    {
        if player != viewer {
            filtered.waiting_for = WaitingFor::LearnChoice {
                player,
                hand_cards: hand_cards.iter().map(|_| ObjectId(0)).collect(),
            };
        }
    }

    if let WaitingFor::EffectZoneChoice {
        player,
        ref cards,
        count,
        up_to,
        source_id,
        effect_kind,
        zone,
        destination,
        enter_tapped,
        enter_transformed,
        under_your_control,
        enters_attacking,
        owner_library,
    } = state.waiting_for
    {
        if player != viewer && zone == Zone::Hand {
            filtered.waiting_for = WaitingFor::EffectZoneChoice {
                player,
                cards: cards.iter().map(|_| ObjectId(0)).collect(),
                count,
                up_to,
                source_id,
                effect_kind,
                zone,
                destination,
                enter_tapped,
                enter_transformed,
                under_your_control,
                enters_attacking,
                owner_library,
            };
        }
    }

    filtered.auto_pass.retain(|pid, _| *pid == viewer);
    filtered
        .lands_tapped_for_mana
        .retain(|pid, _| *pid == viewer);

    if filtered.pending_cast.is_some() && filtered.waiting_for.acting_player() != Some(viewer) {
        filtered.pending_cast = None;
    }

    for pool in &mut filtered.deck_pools {
        if pool.player != viewer {
            pool.registered_main.clear();
            pool.registered_sideboard.clear();
            pool.current_main.clear();
            pool.current_sideboard.clear();
        }
    }

    filtered
}

fn hide_card(state: &mut GameState, obj_id: ObjectId) {
    if let Some(obj) = state.objects.get_mut(&obj_id) {
        obj.face_down = true;
        obj.name = "Hidden Card".to_string();
        obj.abilities.clear();
        obj.keywords.clear();
        obj.base_keywords.clear();
        obj.power = None;
        obj.toughness = None;
        obj.loyalty = None;
        obj.color.clear();
        obj.base_color.clear();
        obj.trigger_definitions.clear();
        obj.replacement_definitions.clear();
        obj.static_definitions.clear();
        obj.casting_permissions.clear();
    }
}
