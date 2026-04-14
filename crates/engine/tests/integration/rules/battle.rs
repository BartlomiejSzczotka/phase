//! Integration tests for Battle permanents (CR 310).
//!
//! Covers:
//! - Defense-counter ETB (CR 310.4b)
//! - Zero-defense SBA (CR 704.5v + CR 310.7)
//! - Protector choice/getter (CR 310.11a + CR 310.8e)
//! - Attack target routing — defending player = protector (CR 508.5 + CR 310.8d)
//! - Protector cannot attack own battle (CR 310.8b)

#![allow(unused_imports)]
use super::*;

use engine::game::sba;
use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::ability::ChosenAttribute;
use engine::types::card_type::CoreType;
use engine::types::counter::CounterType;

/// Convert an existing battlefield creature into a Siege with the given defense.
fn make_into_siege(
    runner: &mut GameRunner,
    id: ObjectId,
    protector: PlayerId,
    printed_defense: u32,
) {
    let obj = runner.state_mut().objects.get_mut(&id).unwrap();
    obj.card_types.core_types.clear();
    obj.card_types.core_types.push(CoreType::Battle);
    obj.card_types.subtypes = vec!["Siege".to_string()];
    obj.base_card_types = obj.card_types.clone();
    obj.power = None;
    obj.toughness = None;
    obj.base_power = None;
    obj.base_toughness = None;
    obj.defense = Some(printed_defense);
    obj.base_defense = Some(printed_defense);
    obj.counters.insert(CounterType::Defense, printed_defense);
    obj.chosen_attributes
        .push(ChosenAttribute::Player(protector));
}

fn prime_siege(
    controller: PlayerId,
    protector: PlayerId,
    name: &str,
    printed_defense: u32,
) -> (GameRunner, ObjectId) {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let id = scenario.add_creature(controller, name, 0, 0).id();
    let mut runner = scenario.build();
    make_into_siege(&mut runner, id, protector, printed_defense);
    (runner, id)
}

/// CR 310.4b + CR 310.4c: A battle on the battlefield has defense equal to its
/// defense counters, with the `defense` field mirroring the counter count.
#[test]
fn battle_has_defense_equal_to_counters() {
    let (runner, battle) = prime_siege(P0, P1, "Test Siege", 4);
    let obj = &runner.state().objects[&battle];
    assert_eq!(obj.defense, Some(4));
    assert_eq!(obj.counters.get(&CounterType::Defense).copied(), Some(4));
}

/// CR 704.5v + CR 310.7: A battle with 0 defense is put into its owner's
/// graveyard by state-based actions.
#[test]
fn zero_defense_battle_goes_to_graveyard_via_sba() {
    let (mut runner, battle) = prime_siege(P0, P1, "Dying Siege", 0);

    let mut events = Vec::new();
    sba::check_state_based_actions(runner.state_mut(), &mut events);

    assert_eq!(
        runner.state().objects[&battle].zone,
        Zone::Graveyard,
        "0-defense battle should be sent to graveyard by SBA"
    );
}

/// CR 310.8e + CR 310.11a: The `protector()` getter returns the chosen opponent.
#[test]
fn protector_getter_returns_chosen_player() {
    let (runner, battle) = prime_siege(P0, P1, "Protected Siege", 3);
    assert_eq!(runner.state().objects[&battle].protector(), Some(P1));
}

/// CR 310.8e: Non-battle permanents always return None from `protector()`.
#[test]
fn non_battle_has_no_protector() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let creature = scenario.add_vanilla(P0, 2, 2);
    let runner = scenario.build();
    assert_eq!(runner.state().objects[&creature].protector(), None);
}

/// CR 508.1b + CR 508.5 + CR 310.8d: When a creature attacks a battle, the
/// defending player for combat purposes is the battle's protector, not the
/// battle's controller. Controller (P0) can attack their own Siege when the
/// protector (P1) is different — CR 310.8b.
#[test]
fn battle_attack_defending_player_is_protector() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let siege_id = scenario.add_creature(P0, "Attackable Siege", 0, 0).id();

    let attacker = scenario.add_creature(P0, "Attacker", 3, 3).id();
    let mut runner = scenario.build();

    // Make attacker combat-ready (not summoning sick).
    {
        let turn = runner.state().turn_number.saturating_sub(1);
        runner
            .state_mut()
            .objects
            .get_mut(&attacker)
            .unwrap()
            .entered_battlefield_turn = Some(turn);
    }
    // Turn the placeholder into a Siege with P0 controller, P1 protector.
    make_into_siege(&mut runner, siege_id, P1, 5);

    runner.pass_both_players(); // → DeclareAttackers

    runner
        .act(GameAction::DeclareAttackers {
            attacks: vec![(attacker, AttackTarget::Battle(siege_id))],
        })
        .expect("attacking a battle controlled by you but protected by an opponent is legal");

    let combat = runner.state().combat.as_ref().expect("combat state");
    let info = combat
        .attackers
        .iter()
        .find(|a| a.object_id == attacker)
        .expect("attacker recorded");
    assert_eq!(
        info.defending_player, P1,
        "defending player for battle = protector (not controller)"
    );
    assert!(matches!(info.attack_target, AttackTarget::Battle(id) if id == siege_id));
}

/// CR 310.8b: A battle's protector cannot attack it — the declaration is illegal.
#[test]
fn battle_protector_cannot_attack_own_battle() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let siege_id = scenario.add_creature(P1, "My Siege", 0, 0).id();
    let attacker = scenario.add_creature(P0, "Attacker", 3, 3).id();
    let mut runner = scenario.build();

    {
        let turn = runner.state().turn_number.saturating_sub(1);
        runner
            .state_mut()
            .objects
            .get_mut(&attacker)
            .unwrap()
            .entered_battlefield_turn = Some(turn);
    }
    // P1 controls, P0 (active) is the protector → P0 cannot attack.
    make_into_siege(&mut runner, siege_id, P0, 3);

    runner.pass_both_players();

    let result = runner.act(GameAction::DeclareAttackers {
        attacks: vec![(attacker, AttackTarget::Battle(siege_id))],
    });
    assert!(
        result.is_err(),
        "protector cannot attack the battle it protects"
    );
}
