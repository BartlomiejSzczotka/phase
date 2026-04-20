#![allow(unused_imports)]
use super::*;

use engine::types::ability::{
    AbilityDefinition, AbilityKind, ControllerRef, Effect, FilterProp, ReplacementCondition,
    ReplacementDefinition, TargetFilter, TypedFilter,
};
use engine::types::card_type::CoreType;
use engine::types::identifiers::CardId;
use engine::types::replacements::ReplacementEvent;

/// Build a fast land replacement definition matching
/// "This land enters tapped unless you control two or fewer other lands."
fn fast_land_replacement(description: &str) -> ReplacementDefinition {
    ReplacementDefinition::new(ReplacementEvent::Moved)
        .execute(AbilityDefinition::new(
            AbilityKind::Spell,
            Effect::Tap {
                target: TargetFilter::SelfRef,
            },
        ))
        .valid_card(TargetFilter::SelfRef)
        .destination_zone(Zone::Battlefield)
        .description(description.to_string())
        .condition(ReplacementCondition::UnlessControlsOtherLeq {
            count: 2,
            filter: TypedFilter::new(engine::types::ability::TypeFilter::Land)
                .controller(ControllerRef::You)
                .properties(vec![FilterProp::Another]),
        })
}

fn replacement_choice_index(runner: &GameRunner, description: &str) -> usize {
    let WaitingFor::ReplacementChoice {
        candidate_descriptions,
        ..
    } = &runner.state().waiting_for
    else {
        panic!(
            "expected ReplacementChoice, got {:?}",
            runner.state().waiting_for
        );
    };

    candidate_descriptions
        .iter()
        .position(|candidate| candidate.contains(description))
        .unwrap_or_else(|| {
            panic!(
                "replacement choice {description:?} not found in {:?}",
                candidate_descriptions
            )
        })
}

// ── Fast land integration tests ──

/// CR 305.7 + CR 614.1c: Fast land with 0 other lands → enters untapped.
#[test]
fn fast_land_zero_other_lands_enters_untapped() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    let mut builder = scenario.add_land_to_hand(P0, "Spirebluff Canal");
    builder.with_replacement_definition(fast_land_replacement(
        "This land enters tapped unless you control two or fewer other lands.",
    ));
    let land_id = builder.id();

    let mut runner = scenario.build();
    let card_id = runner.state().objects[&land_id].card_id;

    runner
        .act(GameAction::PlayLand {
            object_id: land_id,
            card_id,
        })
        .expect("play land should succeed");

    let obj = &runner.state().objects[&land_id];
    assert_eq!(obj.zone, Zone::Battlefield);
    assert!(
        !obj.tapped,
        "Fast land should enter untapped with 0 other lands"
    );
}

/// CR 305.7 + CR 614.1c: Fast land with exactly 2 other lands → enters untapped (boundary).
#[test]
fn fast_land_two_other_lands_enters_untapped() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    // Two lands already on the battlefield (controlled by P0)
    scenario.add_basic_land(P0, engine::types::mana::ManaColor::Blue);
    scenario.add_basic_land(P0, engine::types::mana::ManaColor::Red);

    let mut builder = scenario.add_land_to_hand(P0, "Spirebluff Canal");
    builder.with_replacement_definition(fast_land_replacement(
        "This land enters tapped unless you control two or fewer other lands.",
    ));
    let land_id = builder.id();

    let mut runner = scenario.build();
    let card_id = runner.state().objects[&land_id].card_id;

    runner
        .act(GameAction::PlayLand {
            object_id: land_id,
            card_id,
        })
        .expect("play land should succeed");

    let obj = &runner.state().objects[&land_id];
    assert_eq!(obj.zone, Zone::Battlefield);
    assert!(
        !obj.tapped,
        "Fast land should enter untapped with exactly 2 other lands (boundary)"
    );
}

/// CR 305.7 + CR 614.1c: Fast land with 3 other lands → enters tapped (boundary).
#[test]
fn fast_land_three_other_lands_enters_tapped() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    // Three lands already on the battlefield (controlled by P0)
    scenario.add_basic_land(P0, engine::types::mana::ManaColor::Blue);
    scenario.add_basic_land(P0, engine::types::mana::ManaColor::Red);
    scenario.add_basic_land(P0, engine::types::mana::ManaColor::White);

    let mut builder = scenario.add_land_to_hand(P0, "Spirebluff Canal");
    builder.with_replacement_definition(fast_land_replacement(
        "This land enters tapped unless you control two or fewer other lands.",
    ));
    let land_id = builder.id();

    let mut runner = scenario.build();
    let card_id = runner.state().objects[&land_id].card_id;

    runner
        .act(GameAction::PlayLand {
            object_id: land_id,
            card_id,
        })
        .expect("play land should succeed");

    let obj = &runner.state().objects[&land_id];
    assert_eq!(obj.zone, Zone::Battlefield);
    assert!(
        obj.tapped,
        "Fast land should enter tapped with 3 other lands"
    );
}

/// CR 305.7 + CR 614.1c: Opponent's lands do NOT count for "you control" check.
/// 3 lands total on battlefield but only 2 controlled by P0 → enters untapped.
#[test]
fn fast_land_opponent_lands_not_counted() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    // Two lands controlled by P0
    scenario.add_basic_land(P0, engine::types::mana::ManaColor::Blue);
    scenario.add_basic_land(P0, engine::types::mana::ManaColor::Red);
    // One land controlled by P1 (should NOT count)
    scenario.add_basic_land(P1, engine::types::mana::ManaColor::Green);

    let mut builder = scenario.add_land_to_hand(P0, "Spirebluff Canal");
    builder.with_replacement_definition(fast_land_replacement(
        "This land enters tapped unless you control two or fewer other lands.",
    ));
    let land_id = builder.id();

    let mut runner = scenario.build();
    let card_id = runner.state().objects[&land_id].card_id;

    runner
        .act(GameAction::PlayLand {
            object_id: land_id,
            card_id,
        })
        .expect("play land should succeed");

    let obj = &runner.state().objects[&land_id];
    assert_eq!(obj.zone, Zone::Battlefield);
    assert!(
        !obj.tapped,
        "Fast land should enter untapped — opponent's lands don't count"
    );
}

/// CR 305.7 + CR 614.1c: The entering land itself must NOT be counted
/// in the "other" check.
#[test]
fn fast_land_self_not_counted() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    // Exactly 2 other lands — the entering land makes 3 on the battlefield,
    // but "other" means it must not count itself.
    scenario.add_basic_land(P0, engine::types::mana::ManaColor::Blue);
    scenario.add_basic_land(P0, engine::types::mana::ManaColor::Red);

    let mut builder = scenario.add_land_to_hand(P0, "Spirebluff Canal");
    builder.with_replacement_definition(fast_land_replacement(
        "This land enters tapped unless you control two or fewer other lands.",
    ));
    let land_id = builder.id();

    let mut runner = scenario.build();
    let card_id = runner.state().objects[&land_id].card_id;

    runner
        .act(GameAction::PlayLand {
            object_id: land_id,
            card_id,
        })
        .expect("play land should succeed");

    let obj = &runner.state().objects[&land_id];
    assert_eq!(obj.zone, Zone::Battlefield);
    assert!(
        !obj.tapped,
        "Fast land must not count itself in 'other lands' check — 2 other lands ≤ 2 → untapped"
    );
}

#[test]
fn spelunking_order_can_leave_tapland_tapped() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario
        .add_creature(P0, "Spelunking", 0, 4)
        .as_enchantment()
        .from_oracle_text("Lands you control enter untapped.");

    let land_id = scenario
        .add_land_to_hand(P0, "Guildgate")
        .from_oracle_text("This land enters tapped.")
        .id();

    let mut runner = scenario.build();
    let card_id = runner.state().objects[&land_id].card_id;

    runner
        .act(GameAction::PlayLand {
            object_id: land_id,
            card_id,
        })
        .expect("play land should succeed");
    let spelunking_first = replacement_choice_index(&runner, "Lands you control enter untapped.");
    runner
        .act(GameAction::ChooseReplacement {
            index: spelunking_first,
        })
        .expect("replacement choice should resolve");

    let obj = &runner.state().objects[&land_id];
    assert_eq!(obj.zone, Zone::Battlefield);
    assert!(
        obj.tapped,
        "Choosing Spelunking first should leave the tapland tapped"
    );
}

#[test]
fn spelunking_order_can_leave_tapland_untapped() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario
        .add_creature(P0, "Spelunking", 0, 4)
        .as_enchantment()
        .from_oracle_text("Lands you control enter untapped.");

    let land_id = scenario
        .add_land_to_hand(P0, "Guildgate")
        .from_oracle_text("This land enters tapped.")
        .id();

    let mut runner = scenario.build();
    let card_id = runner.state().objects[&land_id].card_id;

    runner
        .act(GameAction::PlayLand {
            object_id: land_id,
            card_id,
        })
        .expect("play land should succeed");
    let tapland_first = replacement_choice_index(&runner, "This land enters tapped.");
    runner
        .act(GameAction::ChooseReplacement {
            index: tapland_first,
        })
        .expect("replacement choice should resolve");

    let obj = &runner.state().objects[&land_id];
    assert_eq!(obj.zone, Zone::Battlefield);
    assert!(
        !obj.tapped,
        "Choosing the tapland replacement first should let Spelunking untap it"
    );
}

#[test]
fn archelos_untapped_makes_other_taplands_enter_untapped() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario
        .add_creature(P0, "Archelos, Lagoon Mystic", 2, 4)
        .from_oracle_text(
            "As long as ~ is untapped, other permanents enter untapped.\nAs long as ~ is tapped, other permanents enter tapped.",
        );

    let land_id = scenario
        .add_land_to_hand(P0, "Guildgate")
        .from_oracle_text("This land enters tapped.")
        .id();

    let mut runner = scenario.build();
    let card_id = runner.state().objects[&land_id].card_id;

    runner
        .act(GameAction::PlayLand {
            object_id: land_id,
            card_id,
        })
        .expect("play land should succeed");
    let tapland_first = replacement_choice_index(&runner, "This land enters tapped.");
    runner
        .act(GameAction::ChooseReplacement {
            index: tapland_first,
        })
        .expect("replacement choice should resolve");

    let obj = &runner.state().objects[&land_id];
    assert_eq!(obj.zone, Zone::Battlefield);
    assert!(
        !obj.tapped,
        "Untapped Archelos should let other permanents enter untapped"
    );
}

#[test]
fn archelos_tapped_makes_other_lands_enter_tapped() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let archelos_id = scenario
        .add_creature(P0, "Archelos, Lagoon Mystic", 2, 4)
        .from_oracle_text(
            "As long as ~ is untapped, other permanents enter untapped.\nAs long as ~ is tapped, other permanents enter tapped.",
        )
        .id();

    let land_id = scenario.add_land_to_hand(P0, "Forest").id();

    let mut runner = scenario.build();
    runner
        .state_mut()
        .objects
        .get_mut(&archelos_id)
        .unwrap()
        .tapped = true;

    let card_id = runner.state().objects[&land_id].card_id;
    runner
        .act(GameAction::PlayLand {
            object_id: land_id,
            card_id,
        })
        .expect("play land should succeed");

    let obj = &runner.state().objects[&land_id];
    assert_eq!(obj.zone, Zone::Battlefield);
    assert!(
        obj.tapped,
        "Tapped Archelos should make other permanents enter tapped"
    );
}
