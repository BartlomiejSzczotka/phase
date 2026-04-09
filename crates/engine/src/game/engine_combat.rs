use crate::game::combat::{AttackTarget, DamageAssignment, DamageTarget, TrampleKind};
use crate::types::events::GameEvent;
use crate::types::game_state::{CombatDamageAssignmentMode, DamageSlot, GameState, WaitingFor};
use crate::types::identifiers::ObjectId;
use crate::types::phase::Phase;
use crate::types::player::PlayerId;
use crate::types::zones::Zone;

use super::engine::{begin_pending_trigger_target_selection, EngineError};
use super::priority;
use super::triggers;
use super::turns;

pub(super) fn handle_declare_attackers(
    state: &mut GameState,
    player: PlayerId,
    attacks: &[(ObjectId, AttackTarget)],
    events: &mut Vec<GameEvent>,
) -> Result<WaitingFor, EngineError> {
    if state.active_player != player {
        return Err(EngineError::WrongPlayer);
    }
    super::combat::declare_attackers(state, attacks, events).map_err(EngineError::InvalidAction)?;

    triggers::process_triggers(state, events);
    if let Some(waiting_for) = begin_pending_trigger_target_selection(state)? {
        return Ok(waiting_for);
    }

    if attacks.is_empty() {
        state.phase = Phase::EndCombat;
        events.push(GameEvent::PhaseChanged {
            phase: Phase::EndCombat,
        });
        state.combat = None;
        super::layers::prune_end_of_combat_effects(state);
        turns::advance_phase(state, events);
        Ok(turns::auto_advance(state, events))
    } else {
        priority::reset_priority(state);
        Ok(WaitingFor::Priority {
            player: state.active_player,
        })
    }
}

pub(super) fn handle_declare_blockers(
    state: &mut GameState,
    assignments: &[(ObjectId, ObjectId)],
    events: &mut Vec<GameEvent>,
) -> Result<WaitingFor, EngineError> {
    super::combat::declare_blockers(state, assignments, events)
        .map_err(EngineError::InvalidAction)?;

    triggers::process_triggers(state, events);
    if let Some(waiting_for) = begin_pending_trigger_target_selection(state)? {
        return Ok(waiting_for);
    }

    priority::reset_priority(state);
    Ok(WaitingFor::Priority {
        player: state.active_player,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_assign_combat_damage(
    state: &mut GameState,
    player: PlayerId,
    attacker_id: ObjectId,
    total_damage: u32,
    blockers: &[DamageSlot],
    assignment_modes: &[CombatDamageAssignmentMode],
    trample: Option<TrampleKind>,
    defending_player: PlayerId,
    attack_target: &AttackTarget,
    pw_loyalty: Option<u32>,
    pw_controller: Option<PlayerId>,
    mode: CombatDamageAssignmentMode,
    assignments: &[(ObjectId, u32)],
    trample_damage: u32,
    controller_damage: u32,
    events: &mut Vec<GameEvent>,
) -> Result<WaitingFor, EngineError> {
    if !assignment_modes.contains(&mode) {
        return Err(EngineError::InvalidAction(format!(
            "Combat damage assignment mode {:?} is not allowed for attacker {:?}",
            mode, attacker_id
        )));
    }

    if mode == CombatDamageAssignmentMode::AsThoughUnblocked {
        if !assignments.is_empty() || trample_damage > 0 || controller_damage > 0 {
            return Err(EngineError::InvalidAction(
                "As-though-unblocked assignment does not use blocker or trample splits".to_string(),
            ));
        }
        let attacker_info = state
            .combat
            .as_ref()
            .and_then(|combat| {
                combat
                    .attackers
                    .iter()
                    .find(|info| info.object_id == attacker_id)
                    .cloned()
            })
            .ok_or_else(|| {
                EngineError::InvalidAction(format!(
                    "Attacker {:?} not found in combat state",
                    attacker_id
                ))
            })?;
        let damage_assignments = super::combat_damage::assign_damage_as_though_unblocked(
            state,
            &attacker_info,
            total_damage,
            trample,
        );
        if let Some(combat) = &mut state.combat {
            combat.pending_damage.extend(
                damage_assignments
                    .into_iter()
                    .map(|assignment| (attacker_id, assignment)),
            );
            combat.damage_step_index = Some(combat.damage_step_index.unwrap_or(0) + 1);
        }

        if let Some(waiting_for) = super::combat_damage::resolve_combat_damage(state, events) {
            return Ok(waiting_for);
        }

        priority::reset_priority(state);
        return Ok(WaitingFor::Priority { player });
    }

    let assigned_total: u32 = assignments.iter().map(|(_, amount)| *amount).sum::<u32>()
        + trample_damage
        + controller_damage;
    let expected_total = if blockers.is_empty() && trample.is_none() {
        0
    } else {
        total_damage
    };
    if assigned_total != expected_total {
        return Err(EngineError::InvalidAction(format!(
            "Damage assignment total {} != expected {}",
            assigned_total, expected_total
        )));
    }

    let valid_blocker_ids: Vec<ObjectId> = blockers.iter().map(|slot| slot.blocker_id).collect();
    for (blocker_id, _) in assignments {
        if !valid_blocker_ids.contains(blocker_id) {
            return Err(EngineError::InvalidAction(format!(
                "{:?} is not a blocker of attacker {:?}",
                blocker_id, attacker_id
            )));
        }
    }

    if (trample_damage > 0 || controller_damage > 0) && trample.is_none() {
        return Err(EngineError::InvalidAction(
            "Cannot assign trample damage without trample".to_string(),
        ));
    }

    if controller_damage > 0 {
        let is_valid = trample == Some(TrampleKind::OverPlaneswalkers)
            && pw_controller.is_some()
            && matches!(attack_target, AttackTarget::Planeswalker(_));
        if !is_valid {
            return Err(EngineError::InvalidAction(
                "Controller damage only allowed with trample over planeswalkers attacking a planeswalker".to_string(),
            ));
        }

        let loyalty_threshold = pw_loyalty.unwrap_or(0);
        if trample_damage < loyalty_threshold {
            return Err(EngineError::InvalidAction(format!(
                "Trample over planeswalkers: must assign at least {} to PW before {} to controller",
                loyalty_threshold, controller_damage
            )));
        }
    }

    if trample.is_some() {
        for slot in blockers {
            let assigned = assignments
                .iter()
                .find(|(id, _)| *id == slot.blocker_id)
                .map(|(_, amount)| *amount)
                .unwrap_or(0);
            if assigned < slot.lethal_minimum {
                return Err(EngineError::InvalidAction(format!(
                    "Trample: blocker {:?} must receive at least {} lethal damage before excess to player",
                    slot.blocker_id, slot.lethal_minimum
                )));
            }
        }
    }

    if let Some(combat) = &mut state.combat {
        for (blocker_id, amount) in assignments {
            if *amount > 0 {
                combat.pending_damage.push((
                    attacker_id,
                    DamageAssignment {
                        target: DamageTarget::Object(*blocker_id),
                        amount: *amount,
                    },
                ));
            }
        }

        if trample_damage > 0 {
            let is_over_pw = trample == Some(TrampleKind::OverPlaneswalkers);
            let excess_target = match attack_target {
                AttackTarget::Player(player_id) => Some(DamageTarget::Player(*player_id)),
                AttackTarget::Planeswalker(pw_id) => match state.objects.get(pw_id) {
                    Some(obj) if obj.zone == Zone::Battlefield => {
                        Some(DamageTarget::Object(*pw_id))
                    }
                    _ if is_over_pw => Some(DamageTarget::Player(defending_player)),
                    _ => None,
                },
                AttackTarget::Battle(battle_id) => match state.objects.get(battle_id) {
                    Some(obj) if obj.zone == Zone::Battlefield => {
                        Some(DamageTarget::Object(*battle_id))
                    }
                    _ => None,
                },
            };
            if let Some(target) = excess_target {
                combat.pending_damage.push((
                    attacker_id,
                    DamageAssignment {
                        target,
                        amount: trample_damage,
                    },
                ));
            }
        }

        if controller_damage > 0 {
            if let Some(controller) = pw_controller {
                combat.pending_damage.push((
                    attacker_id,
                    DamageAssignment {
                        target: DamageTarget::Player(controller),
                        amount: controller_damage,
                    },
                ));
            }
        }

        combat.damage_step_index = Some(combat.damage_step_index.unwrap_or(0) + 1);
    }

    if let Some(waiting_for) = super::combat_damage::resolve_combat_damage(state, events) {
        return Ok(waiting_for);
    }

    priority::reset_priority(state);
    Ok(WaitingFor::Priority { player })
}

/// CR 508.8: If no creatures are declared as attackers, skip declare blockers and combat damage steps.
pub(super) fn handle_empty_attackers(
    state: &mut GameState,
    events: &mut Vec<GameEvent>,
) -> Result<WaitingFor, EngineError> {
    super::combat::declare_attackers(state, &[], events).map_err(EngineError::InvalidAction)?;

    triggers::process_triggers(state, events);
    if let Some(waiting_for) = begin_pending_trigger_target_selection(state)? {
        return Ok(waiting_for);
    }

    state.phase = Phase::EndCombat;
    events.push(GameEvent::PhaseChanged {
        phase: Phase::EndCombat,
    });
    state.combat = None;
    super::layers::prune_end_of_combat_effects(state);
    turns::advance_phase(state, events);
    Ok(turns::auto_advance(state, events))
}

pub(super) fn handle_empty_blockers(
    state: &mut GameState,
    events: &mut Vec<GameEvent>,
) -> Result<WaitingFor, EngineError> {
    super::combat::declare_blockers(state, &[], events).map_err(EngineError::InvalidAction)?;

    triggers::process_triggers(state, events);
    if let Some(waiting_for) = begin_pending_trigger_target_selection(state)? {
        return Ok(waiting_for);
    }

    turns::advance_phase(state, events);
    Ok(turns::auto_advance(state, events))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::combat::{AttackerInfo, CombatState};
    use crate::game::zones::create_object;
    use crate::types::game_state::CombatDamageAssignmentMode;
    use crate::types::identifiers::CardId;

    fn setup() -> GameState {
        let mut state = GameState::new_two_player(42);
        state.turn_number = 2;
        state.active_player = PlayerId(0);
        state.priority_player = PlayerId(0);
        state
    }

    fn create_creature(
        state: &mut GameState,
        owner: PlayerId,
        name: &str,
        power: i32,
        toughness: i32,
    ) -> ObjectId {
        let id = create_object(
            state,
            CardId(state.next_object_id),
            owner,
            name.to_string(),
            Zone::Battlefield,
        );
        let obj = state.objects.get_mut(&id).unwrap();
        obj.card_types
            .core_types
            .push(crate::types::card_type::CoreType::Creature);
        obj.power = Some(power);
        obj.toughness = Some(toughness);
        obj.entered_battlefield_turn = Some(1);
        id
    }

    fn create_planeswalker(
        state: &mut GameState,
        owner: PlayerId,
        name: &str,
        loyalty: u32,
    ) -> ObjectId {
        let id = create_object(
            state,
            CardId(state.next_object_id),
            owner,
            name.to_string(),
            Zone::Battlefield,
        );
        let obj = state.objects.get_mut(&id).unwrap();
        obj.card_types
            .core_types
            .push(crate::types::card_type::CoreType::Planeswalker);
        obj.loyalty = Some(loyalty);
        id
    }

    #[test]
    fn as_though_unblocked_mode_applies_only_when_chosen() {
        let mut state = setup();
        let attacker = create_creature(&mut state, PlayerId(0), "Thorn Elemental", 5, 5);
        let blocker = create_creature(&mut state, PlayerId(1), "Wall", 0, 4);
        state.combat = Some(CombatState {
            attackers: vec![AttackerInfo::attacking_player(attacker, PlayerId(1))],
            blocker_assignments: std::iter::once((attacker, vec![blocker])).collect(),
            blocker_to_attacker: std::iter::once((blocker, vec![attacker])).collect(),
            ..Default::default()
        });
        if let Some(combat) = &mut state.combat {
            combat.attackers[0].blocked = true;
        }

        let mut events = Vec::new();
        let waiting = handle_assign_combat_damage(
            &mut state,
            PlayerId(0),
            attacker,
            5,
            &[DamageSlot {
                blocker_id: blocker,
                lethal_minimum: 4,
            }],
            &[
                CombatDamageAssignmentMode::Normal,
                CombatDamageAssignmentMode::AsThoughUnblocked,
            ],
            None,
            PlayerId(1),
            &AttackTarget::Player(PlayerId(1)),
            None,
            None,
            CombatDamageAssignmentMode::AsThoughUnblocked,
            &[],
            0,
            0,
            &mut events,
        )
        .unwrap();

        assert!(matches!(waiting, WaitingFor::Priority { .. }));
        assert_eq!(state.players[1].life, 15);
        assert_eq!(state.objects[&blocker].damage_marked, 0);
    }

    #[test]
    fn as_though_unblocked_mode_can_hit_planeswalker() {
        let mut state = setup();
        let attacker = create_creature(&mut state, PlayerId(0), "Thorn Elemental", 4, 4);
        let blocker = create_creature(&mut state, PlayerId(1), "Wall", 0, 4);
        let pw = create_planeswalker(&mut state, PlayerId(1), "Test Planeswalker", 6);
        state.combat = Some(CombatState {
            attackers: vec![AttackerInfo::new(
                attacker,
                AttackTarget::Planeswalker(pw),
                PlayerId(1),
            )],
            blocker_assignments: std::iter::once((attacker, vec![blocker])).collect(),
            blocker_to_attacker: std::iter::once((blocker, vec![attacker])).collect(),
            ..Default::default()
        });
        if let Some(combat) = &mut state.combat {
            combat.attackers[0].blocked = true;
        }

        let mut events = Vec::new();
        let waiting = handle_assign_combat_damage(
            &mut state,
            PlayerId(0),
            attacker,
            4,
            &[DamageSlot {
                blocker_id: blocker,
                lethal_minimum: 4,
            }],
            &[
                CombatDamageAssignmentMode::Normal,
                CombatDamageAssignmentMode::AsThoughUnblocked,
            ],
            None,
            PlayerId(1),
            &AttackTarget::Planeswalker(pw),
            None,
            None,
            CombatDamageAssignmentMode::AsThoughUnblocked,
            &[],
            0,
            0,
            &mut events,
        )
        .unwrap();

        assert!(matches!(waiting, WaitingFor::Priority { .. }));
        assert_eq!(state.objects[&pw].loyalty, Some(2));
        assert_eq!(state.players[1].life, 20);
        assert_eq!(state.objects[&blocker].damage_marked, 0);
    }
}
