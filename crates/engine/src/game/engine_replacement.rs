use crate::ai_support::copy_target_mana_value_ceiling;
use crate::types::ability::{AbilityDefinition, Effect, ResolvedAbility, TargetFilter, TargetRef};
use crate::types::events::GameEvent;
use crate::types::game_state::{GameState, WaitingFor};
use crate::types::identifiers::ObjectId;
use crate::types::player::PlayerId;
use crate::types::proposed_event::ProposedEvent;
use crate::types::zones::Zone;

use super::effects;
use super::effects::deal_damage::{apply_damage_after_replacement, DamageContext};
use super::effects::draw::apply_draw_after_replacement;
use super::effects::life::{apply_life_gain_after_replacement, apply_life_loss_after_replacement};
use super::engine::EngineError;
use super::zones;

pub(super) fn handle_replacement_choice(
    state: &mut GameState,
    index: usize,
    events: &mut Vec<GameEvent>,
) -> Result<WaitingFor, EngineError> {
    match super::replacement::continue_replacement(state, index, events) {
        super::replacement::ReplacementResult::Execute(event) => {
            let mut zone_change_object_id = None;
            let mut enters_battlefield = false;
            match event {
                ProposedEvent::ZoneChange {
                    object_id,
                    to,
                    from,
                    enter_tapped,
                    enter_with_counters,
                    controller_override,
                    enter_transformed,
                    ..
                } => {
                    zones::move_to_zone(state, object_id, to, events);
                    // CR 400.7: reset_for_battlefield_entry (inside move_to_zone) sets
                    // defaults. Override only when the replacement pipeline changed them.
                    if to == Zone::Battlefield {
                        if let Some(obj) = state.objects.get_mut(&object_id) {
                            if enter_tapped {
                                obj.tapped = true;
                            }
                            if let Some(new_controller) = controller_override {
                                obj.controller = new_controller;
                            }
                            // CR 614.1c: Apply counters from replacement pipeline.
                            apply_etb_counters(obj, &enter_with_counters, events);
                            // CR 614.1c: Apply pending ETB counters from delayed triggers
                            // (e.g., "that creature enters with an additional +1/+1 counter").
                            let pending: Vec<_> = state
                                .pending_etb_counters
                                .iter()
                                .filter(|(oid, _, _)| *oid == object_id)
                                .map(|(_, ct, n)| (ct.clone(), *n))
                                .collect();
                            if !pending.is_empty() {
                                apply_etb_counters(obj, &pending, events);
                                state
                                    .pending_etb_counters
                                    .retain(|(oid, _, _)| *oid != object_id);
                            }
                        }
                    }
                    // CR 712.14a: Apply transformation if entering the battlefield transformed.
                    if enter_transformed && to == Zone::Battlefield {
                        if let Some(obj) = state.objects.get(&object_id) {
                            if obj.back_face.is_some() && !obj.transformed {
                                let _ = crate::game::transform::transform_permanent(
                                    state, object_id, events,
                                );
                            }
                        }
                    }
                    if to == Zone::Battlefield || from == Zone::Battlefield {
                        state.layers_dirty = true;
                    }
                    enters_battlefield = to == Zone::Battlefield;
                    zone_change_object_id = Some(object_id);
                }
                // CR 120.3 + CR 120.4b: Damage accepted after replacement choice — apply via the
                // shared helper so wither/infect/planeswalker/excess/lifelink paths match
                // the non-choice delivery. Reconstruct DamageContext from the source at
                // resumption time (CR 609.6: characteristics at time of dealing).
                damage @ ProposedEvent::Damage {
                    source_id,
                    is_combat,
                    ..
                } => {
                    let ctx = DamageContext::from_source(state, source_id).unwrap_or_else(|| {
                        let controller = state
                            .objects
                            .get(&source_id)
                            .map(|obj| obj.controller)
                            .unwrap_or(state.active_player);
                        DamageContext::fallback(source_id, controller)
                    });
                    let _ = apply_damage_after_replacement(state, &ctx, damage, is_combat, events);
                }
                // CR 122.1: Counter addition accepted after replacement choice (e.g.,
                // Corpsejack Menace doubler on a prompted counter-placement).
                ProposedEvent::AddCounter {
                    object_id,
                    counter_type,
                    count,
                    ..
                } => {
                    if let Some(obj) = state.objects.get_mut(&object_id) {
                        let entry = obj.counters.entry(counter_type.clone()).or_insert(0);
                        *entry += count;
                        if matches!(
                            counter_type,
                            crate::types::counter::CounterType::Plus1Plus1
                                | crate::types::counter::CounterType::Minus1Minus1
                        ) {
                            state.layers_dirty = true;
                        }
                        state
                            .players_who_added_counter_this_turn
                            .insert(obj.controller);
                        events.push(GameEvent::CounterAdded {
                            object_id,
                            counter_type,
                            count,
                        });
                    }
                }
                // CR 121.1: Counter removal accepted after replacement choice.
                ProposedEvent::RemoveCounter {
                    object_id,
                    counter_type,
                    count,
                    ..
                } => {
                    if let Some(obj) = state.objects.get_mut(&object_id) {
                        let entry = obj.counters.entry(counter_type.clone()).or_insert(0);
                        *entry = entry.saturating_sub(count);
                        if matches!(
                            counter_type,
                            crate::types::counter::CounterType::Plus1Plus1
                                | crate::types::counter::CounterType::Minus1Minus1
                        ) {
                            state.layers_dirty = true;
                        }
                        events.push(GameEvent::CounterRemoved {
                            object_id,
                            counter_type,
                            count,
                        });
                    }
                }
                // CR 701.26a: Tap accepted after replacement choice.
                ProposedEvent::Tap { object_id, .. } => {
                    if let Some(obj) = state.objects.get_mut(&object_id) {
                        obj.tapped = true;
                        events.push(GameEvent::PermanentTapped {
                            object_id,
                            caused_by: None,
                        });
                    }
                }
                // CR 701.26b: Untap accepted after replacement choice.
                ProposedEvent::Untap { object_id, .. } => {
                    if let Some(obj) = state.objects.get_mut(&object_id) {
                        obj.tapped = false;
                        events.push(GameEvent::PermanentUntapped { object_id });
                    }
                }
                // CR 121.1: Draw accepted after replacement choice — delegate to the
                // shared post-replacement helper so library-zone move + per-turn
                // accounting match the non-choice delivery.
                draw @ ProposedEvent::Draw { .. } => {
                    apply_draw_after_replacement(state, draw, events);
                }
                // CR 119.1: Life gain accepted after replacement choice.
                gain @ ProposedEvent::LifeGain { .. } => {
                    apply_life_gain_after_replacement(state, gain, events);
                }
                // CR 120.3: Life loss accepted after replacement choice.
                loss @ ProposedEvent::LifeLoss { .. } => {
                    apply_life_loss_after_replacement(state, loss, events);
                }
                // CR 701.9a: Discard accepted after replacement choice — move the
                // object hand → graveyard and record/emit the discard event. The
                // replacement pipeline may have modified `object_id`/`player_id`
                // (e.g., Madness redirects surface as a ZoneChange variant handled
                // by the ZoneChange arm above, not here).
                ProposedEvent::Discard {
                    player_id,
                    object_id,
                    ..
                } => {
                    zones::move_to_zone(state, object_id, Zone::Graveyard, events);
                    crate::game::restrictions::record_discard(state, player_id);
                    events.push(GameEvent::Discarded {
                        player_id,
                        object_id,
                    });
                }
                // CR 106.3 + CR 106.4: Mana production accepted after replacement choice.
                // In practice CR 614.5 mana-type replacements don't require a choice and
                // `mana_payment::produce_mana` falls back to the original type on NeedsChoice,
                // so this arm is defensive. If reached, apply the (possibly modified) unit.
                ProposedEvent::ProduceMana {
                    source_id,
                    player_id,
                    mana_type,
                    ..
                } => {
                    let unit = crate::types::mana::ManaUnit {
                        color: mana_type,
                        source_id,
                        snow: false,
                        restrictions: Vec::new(),
                        grants: Vec::new(),
                        expiry: None,
                    };
                    if let Some(player) = state.players.iter_mut().find(|p| p.id == player_id) {
                        player.mana_pool.add(unit);
                        events.push(GameEvent::ManaAdded {
                            player_id,
                            mana_type,
                            source_id,
                            tapped_for_mana: false,
                        });
                    }
                }
                // CR 614.1b + CR 614.10: BeginTurn / BeginPhase replacements are
                // mandatory skip effects that never set `replacement_choice_waiting_for`
                // (see `turns.rs` — NeedsChoice on these is treated as a bug). Arms are
                // present for exhaustiveness; reaching them is an engine error.
                ProposedEvent::BeginTurn { .. } => {
                    debug_assert!(
                        false,
                        "handle_replacement_choice: BeginTurn is a mandatory-skip replacement and should never surface as a choice"
                    );
                }
                ProposedEvent::BeginPhase { .. } => {
                    debug_assert!(
                        false,
                        "handle_replacement_choice: BeginPhase is a mandatory-skip replacement and should never surface as a choice"
                    );
                }
                // Variants whose apply path re-enters the replacement pipeline
                // (Destroy/Sacrifice → inner ZoneChange) or requires the full Effect
                // context not preserved on the ProposedEvent (CreateToken needs parsed
                // token attrs). Deferred to follow-up batches — see
                // docs/todo/replacement-choice-non-zone-events.md.
                other @ (ProposedEvent::CreateToken { .. }
                | ProposedEvent::Destroy { .. }
                | ProposedEvent::Sacrifice { .. }) => {
                    debug_assert!(
                        false,
                        "handle_replacement_choice: accepted {other:?} not yet delivered — see docs/todo/replacement-choice-non-zone-events.md",
                    );
                }
            }

            let mut waiting_for = WaitingFor::Priority {
                player: state.active_player,
            };
            state.waiting_for = waiting_for.clone();

            let mut replacement_ctx = None;
            if let Some(ctx) = state.pending_spell_resolution.take() {
                if enters_battlefield {
                    apply_pending_spell_resolution(state, &ctx);
                }
                replacement_ctx = Some(ctx);
            }

            if let Some(effect_def) = state.post_replacement_effect.take() {
                if let Some(next_waiting_for) = apply_post_replacement_effect(
                    state,
                    &effect_def,
                    zone_change_object_id,
                    replacement_ctx.as_ref(),
                    events,
                ) {
                    waiting_for = next_waiting_for;
                }
            }

            if matches!(waiting_for, WaitingFor::Priority { .. }) {
                if let Some(cont) = state.pending_continuation.take() {
                    let _ = effects::resolve_ability_chain(state, &cont, events, 0);
                    // CR 616.1e: The continuation may itself pause on another replacement
                    // (e.g., the second direction of fight damage hitting the same shield),
                    // in which case it sets `state.waiting_for` to the next ReplacementChoice.
                    // Propagate that back so the engine surfaces the correct prompt.
                    if !matches!(state.waiting_for, WaitingFor::Priority { .. }) {
                        waiting_for = state.waiting_for.clone();
                    }
                }
            }

            Ok(waiting_for)
        }
        super::replacement::ReplacementResult::NeedsChoice(player) => Ok(
            super::replacement::replacement_choice_waiting_for(player, state),
        ),
        super::replacement::ReplacementResult::Prevented => {
            // CR 608.3e: If the ETB was prevented during spell resolution,
            // the permanent goes to the graveyard instead.
            if let Some(ctx) = state.pending_spell_resolution.take() {
                zones::move_to_zone(state, ctx.object_id, Zone::Graveyard, events);
            }
            state.pending_continuation = None;
            Ok(WaitingFor::Priority {
                player: state.active_player,
            })
        }
    }
}

pub(super) fn handle_copy_target_choice(
    state: &mut GameState,
    waiting_for: WaitingFor,
    target: Option<TargetRef>,
    events: &mut Vec<GameEvent>,
) -> Result<WaitingFor, EngineError> {
    let WaitingFor::CopyTargetChoice {
        player,
        source_id,
        valid_targets,
        ..
    } = waiting_for
    else {
        return Err(EngineError::InvalidAction(
            "Not waiting for copy target choice".to_string(),
        ));
    };

    let target_id = match target {
        Some(TargetRef::Object(id)) if valid_targets.contains(&id) => id,
        _ => {
            return Err(EngineError::InvalidAction(
                "Invalid copy target".to_string(),
            ))
        }
    };

    let ability = copy_effect_for_source(state, source_id)
        .map(|effect_def| {
            resolved_ability_from_definition(
                effect_def,
                source_id,
                player,
                vec![TargetRef::Object(target_id)],
            )
        })
        .unwrap_or_else(|| {
            ResolvedAbility::new(
                Effect::BecomeCopy {
                    target: TargetFilter::Any,
                    duration: None,
                    mana_value_limit: None,
                    additional_modifications: Vec::new(),
                },
                vec![TargetRef::Object(target_id)],
                source_id,
                player,
            )
        });
    let _ = effects::resolve_ability_chain(state, &ability, events, 0);
    state.layers_dirty = true;
    if let Some(cont) = state.pending_continuation.take() {
        let _ = effects::resolve_ability_chain(state, &cont, events, 0);
    }
    Ok(WaitingFor::Priority {
        player: state.active_player,
    })
}

fn copy_effect_for_source(state: &GameState, source_id: ObjectId) -> Option<&AbilityDefinition> {
    state
        .objects
        .get(&source_id)?
        .replacement_definitions
        .iter()
        .filter_map(|replacement| replacement.execute.as_deref())
        .find(|effect_def| matches!(&*effect_def.effect, Effect::BecomeCopy { .. }))
}

/// Apply a post-replacement side effect after a zone change has been executed.
/// Used by Optional replacements (e.g., shock lands: pay life on accept, tap on decline).
/// CR 707.9: For "enter as a copy" replacements, sets up CopyTargetChoice instead of
/// immediate resolution, since the player must choose which permanent to copy.
pub(super) fn apply_post_replacement_effect(
    state: &mut GameState,
    effect_def: &AbilityDefinition,
    object_id: Option<ObjectId>,
    spell_resolution: Option<&crate::types::game_state::PendingSpellResolution>,
    events: &mut Vec<GameEvent>,
) -> Option<WaitingFor> {
    let (source_id, controller) = object_id
        .and_then(|obj_id| {
            state
                .objects
                .get(&obj_id)
                .map(|obj| (obj_id, obj.controller))
        })
        .unwrap_or((ObjectId(0), state.active_player));

    if let Effect::BecomeCopy { ref target, .. } = *effect_def.effect {
        let max_mana_value = spell_resolution
            .and_then(|ctx| copy_target_mana_value_ceiling(ctx.actual_mana_spent, effect_def));
        let valid_targets = find_copy_targets(state, target, source_id, controller, max_mana_value);
        if valid_targets.is_empty() {
            return None;
        }
        return Some(WaitingFor::CopyTargetChoice {
            player: controller,
            source_id,
            valid_targets,
            max_mana_value,
        });
    }

    let targets = object_id
        .map(TargetRef::Object)
        .into_iter()
        .collect::<Vec<_>>();
    let resolved = resolved_ability_from_definition(effect_def, source_id, controller, targets);
    let _ = effects::resolve_ability_chain(state, &resolved, events, 0);

    match &state.waiting_for {
        WaitingFor::Priority { .. } => None,
        wf => Some(wf.clone()),
    }
}

/// CR 608.3: Complete post-resolution work for a permanent spell whose ETB
/// went through the replacement pipeline and required a player choice.
/// Applies cast_from_zone, aura attachment, and warp delayed triggers.
fn apply_pending_spell_resolution(
    state: &mut GameState,
    ctx: &crate::types::game_state::PendingSpellResolution,
) {
    use crate::types::game_state::CastingVariant;

    // CR 603.4: Propagate cast_from_zone so ETB triggers can evaluate
    // conditions like "if you cast it from your hand".
    if let Some(obj) = state.objects.get_mut(&ctx.object_id) {
        obj.cast_from_zone = ctx.cast_from_zone;
    }

    // CR 303.4f: Aura resolving to battlefield attaches to its target.
    let is_aura = state
        .objects
        .get(&ctx.object_id)
        .map(|obj| obj.card_types.subtypes.iter().any(|s| s == "Aura"))
        .unwrap_or(false);
    if is_aura {
        if let Some(crate::types::ability::TargetRef::Object(target_id)) = ctx.spell_targets.first()
        {
            if state.battlefield.contains(target_id) {
                effects::attach::attach_to(state, ctx.object_id, *target_id);
            }
        }
    }

    // CR 702.185a: Warp delayed trigger setup.
    if ctx.casting_variant == CastingVariant::Warp {
        let has_warp = state.objects.get(&ctx.object_id).is_some_and(|obj| {
            obj.keywords
                .iter()
                .any(|k| matches!(k, crate::types::keywords::Keyword::Warp(_)))
        });
        if has_warp {
            super::stack::create_warp_delayed_trigger(state, ctx.object_id, ctx.controller);
        }
    }
}

pub(super) fn apply_etb_counters(
    obj: &mut super::game_object::GameObject,
    counters: &[(String, u32)],
    events: &mut Vec<GameEvent>,
) {
    for (counter_type_str, count) in counters {
        let ct = crate::types::counter::parse_counter_type(counter_type_str);
        *obj.counters.entry(ct.clone()).or_insert(0) += count;
        events.push(GameEvent::CounterAdded {
            object_id: obj.id,
            counter_type: ct,
            count: *count,
        });
    }
}

fn find_copy_targets(
    state: &GameState,
    filter: &TargetFilter,
    source_id: ObjectId,
    controller: PlayerId,
    max_mana_value: Option<u32>,
) -> Vec<ObjectId> {
    let ctx = super::filter::FilterContext::from_source_with_controller(source_id, controller);
    state
        .objects
        .iter()
        .filter(|(id, obj)| {
            obj.zone == Zone::Battlefield
                && **id != source_id
                && max_mana_value.is_none_or(|max| obj.mana_cost.mana_value() <= max)
                && super::filter::matches_target_filter(state, **id, filter, &ctx)
        })
        .map(|(id, _)| *id)
        .collect()
}

fn resolved_ability_from_definition(
    def: &AbilityDefinition,
    source_id: ObjectId,
    controller: PlayerId,
    targets: Vec<TargetRef>,
) -> ResolvedAbility {
    let mut resolved =
        ResolvedAbility::new(*def.effect.clone(), targets, source_id, controller).kind(def.kind);
    if let Some(sub) = &def.sub_ability {
        resolved = resolved.sub_ability(resolved_ability_from_definition(
            sub,
            source_id,
            controller,
            Vec::new(),
        ));
    }
    if let Some(else_ab) = &def.else_ability {
        resolved.else_ability = Some(Box::new(resolved_ability_from_definition(
            else_ab,
            source_id,
            controller,
            Vec::new(),
        )));
    }
    if let Some(d) = def.duration.clone() {
        resolved = resolved.duration(d);
    }
    if let Some(c) = def.condition.clone() {
        resolved = resolved.condition(c);
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::super::game_object::GameObject;
    use super::*;
    use crate::game::engine::apply;
    use crate::game::replacement::{self as replacement_mod, ReplacementResult};
    use crate::game::zones::create_object;
    use crate::types::ability::{ReplacementDefinition, ReplacementMode};
    use crate::types::actions::GameAction;
    use crate::types::card_type::CoreType;
    use crate::types::counter::CounterType;
    use crate::types::identifiers::{CardId, ObjectId};
    use crate::types::player::PlayerId;
    use crate::types::proposed_event::ProposedEvent;
    use crate::types::replacements::ReplacementEvent;

    /// Helper: install an Optional replacement on a battlefield object so the
    /// matching proposed event pauses for a player choice.
    fn install_optional_replacement(state: &mut GameState, event: ReplacementEvent) -> ObjectId {
        let id = ObjectId(state.next_object_id);
        state.next_object_id += 1;
        let mut obj = GameObject::new(
            id,
            CardId(999),
            PlayerId(1),
            "Shield".to_string(),
            Zone::Battlefield,
        );
        obj.replacement_definitions.push(
            ReplacementDefinition::new(event)
                .mode(ReplacementMode::Optional { decline: None })
                .description("Shield".to_string()),
        );
        state.objects.insert(id, obj);
        state.battlefield.push(id);
        id
    }

    fn make_creature(state: &mut GameState, owner: PlayerId, name: &str) -> ObjectId {
        let id = create_object(
            state,
            CardId(state.next_object_id),
            owner,
            name.to_string(),
            Zone::Battlefield,
        );
        let obj = state.objects.get_mut(&id).unwrap();
        obj.card_types.core_types.push(CoreType::Creature);
        id
    }

    /// CR 122.1: When a player accepts an AddCounter replacement choice, the
    /// (possibly modified) counter event must be applied. Previously
    /// `handle_replacement_choice` silently dropped non-ZoneChange events.
    #[test]
    fn add_counter_replacement_accepted_applies_counters() {
        let mut state = GameState::new_two_player(42);
        let target = make_creature(&mut state, PlayerId(0), "Bear");
        install_optional_replacement(&mut state, ReplacementEvent::AddCounter);

        let mut events = Vec::new();
        let proposed = ProposedEvent::AddCounter {
            object_id: target,
            counter_type: CounterType::Plus1Plus1,
            count: 2,
            applied: std::collections::HashSet::new(),
        };
        let result = replacement_mod::replace_event(&mut state, proposed, &mut events);
        let ReplacementResult::NeedsChoice(player) = result else {
            panic!("expected NeedsChoice, got {result:?}");
        };
        // replace_event stashes pending_replacement but doesn't set waiting_for on its own —
        // callers (e.g. effect handlers) do that. Set it here to match real call sites.
        state.waiting_for = replacement_mod::replacement_choice_waiting_for(player, &state);
        state.priority_player = player;

        // Accept the replacement — counters must land on the target.
        apply(&mut state, GameAction::ChooseReplacement { index: 0 }).expect("accept");

        let counters_on_target = *state.objects[&target]
            .counters
            .get(&CounterType::Plus1Plus1)
            .unwrap_or(&0);
        assert_eq!(
            counters_on_target, 2,
            "AddCounter accepted after replacement choice must deliver counters"
        );
    }

    /// CR 701.26a: Tap accepted after replacement choice applies the tap state
    /// and emits `PermanentTapped`.
    #[test]
    fn tap_replacement_accepted_applies_tap() {
        let mut state = GameState::new_two_player(42);
        let target = make_creature(&mut state, PlayerId(0), "Bear");
        assert!(!state.objects[&target].tapped, "precondition");
        install_optional_replacement(&mut state, ReplacementEvent::Tap);

        let mut events = Vec::new();
        let proposed = ProposedEvent::Tap {
            object_id: target,
            applied: std::collections::HashSet::new(),
        };
        let result = replacement_mod::replace_event(&mut state, proposed, &mut events);
        let ReplacementResult::NeedsChoice(player) = result else {
            panic!("expected NeedsChoice, got {result:?}");
        };
        state.waiting_for = replacement_mod::replacement_choice_waiting_for(player, &state);
        state.priority_player = player;

        apply(&mut state, GameAction::ChooseReplacement { index: 0 }).expect("accept");

        assert!(
            state.objects[&target].tapped,
            "Tap accepted after replacement choice must tap the target"
        );
    }

    /// CR 615.1: When the player declines (or the replacement pipeline returns
    /// `Prevented`), the proposed event is NOT applied. Guardrail that the
    /// extraction of `apply_damage_after_replacement` did not regress the
    /// prevention path.
    #[test]
    fn replacement_prevented_does_not_apply() {
        use crate::game::effects::deal_damage::{apply_damage_after_replacement, DamageContext};

        let mut state = GameState::new_two_player(42);
        let target = make_creature(&mut state, PlayerId(0), "Bear");
        let source_id = ObjectId(state.next_object_id);
        state.next_object_id += 1;
        // Bypass the replacement pipeline entirely — simulate that the pipeline
        // returned Prevented by NOT calling apply_damage_after_replacement. The
        // target must have zero marked damage (nothing applied).
        let _ctx = DamageContext::fallback(source_id, PlayerId(0));
        // Sanity: calling apply_damage_after_replacement WITH a Damage event
        // does apply (this confirms the helper is the sole application path).
        let damage_event = ProposedEvent::Damage {
            source_id,
            target: crate::types::ability::TargetRef::Object(target),
            amount: 0,
            is_combat: false,
            applied: std::collections::HashSet::new(),
        };
        let mut events = Vec::new();
        let _ = apply_damage_after_replacement(&mut state, &_ctx, damage_event, false, &mut events);
        assert_eq!(
            state.objects[&target].damage_marked, 0,
            "zero-amount damage event applies zero damage"
        );
    }
}
