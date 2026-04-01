# Oracle Parser — Architecture & Contribution Guide

The oracle parser converts MTG card text (from MTGJSON) into typed `AbilityDefinition` structs
that the engine can execute. This document describes the philosophy, structure, and the correct
way to extend it.

---

## Core Philosophy

**The parser is a one-way translation layer.** It takes natural-language Oracle text and produces
a typed data model. All game logic lives in `crates/engine/src/game/` — the parser only produces
data structures, never executes game rules.

1. **Parse intent, not syntax.** Oracle text for the same concept (e.g. "exile target creature")
   can appear in many grammatical forms. The parser must handle all of them and produce the same
   typed output.

2. **Information must not be silently lost.** If Oracle text encodes a semantic distinction (e.g.
   "its controller" vs "you"), that distinction must be preserved in the typed output — never
   discarded by generic subject-stripping.

3. **Unrecognized text → `Effect::Unimplemented`, never panic.** The parser is best-effort. Unknown
   patterns fall through cleanly; the engine skips `Unimplemented` effects without crashing.

4. **Follow the existing type patterns.** The data model uses `QuantityExpr` for all amounts/counts,
   `QuantityRef` for dynamic game-state references, `PlayerFilter` for player-level conditions,
   `GainLifePlayer` for player targeting, and `TargetFilter` for object targeting. New semantic
   distinctions belong in the type layer, not as ad-hoc boolean flags.

---

## Architecture

```
oracle.rs               Entry point: parse_oracle_text(), dispatch_line_nom() fallback
oracle_effect/          Effect / ability parsing (directory with mod.rs + sub-modules)
oracle_target.rs        Target filter parsing (TargetFilter) + event-context references
oracle_cost.rs          Cost parsing (AbilityCost)
oracle_trigger.rs       Trigger condition parsing
oracle_static.rs        Static ability parsing
oracle_replacement.rs   Replacement effect parsing (lands, graveyard exile, counters, …)
oracle_util.rs          Shared utilities (parse_number wrapper, TextPair, phrase helpers, …)
oracle_nom/             Nom 8.0 combinator foundation (see below)
```

### Nom Combinator Foundation — `oracle_nom/`

All parser branches delegate atomic parsing operations to shared nom 8.0 combinators in
`parser/oracle_nom/`. This module provides typed, composable parsers with structured error
traces via `VerboseError`.

```
oracle_nom/
├── primitives.rs   — Numbers, mana symbols, colors, counters, P/T modifiers, roman numerals
├── target.rs       — Target phrase combinators (controller suffix, color prefix, combat status)
├── quantity.rs     — Quantity expression combinators (quantity refs, for-each)
├── duration.rs     — Duration phrase combinators (until end of turn, etc.)
├── condition.rs    — Condition phrase combinators (if/unless/as long as)
├── filter.rs       — Filter property combinators (type phrases, controller)
├── bridge.rs       — Case-bridging utilities: nom_on_lower (run nom on lowercase, map remainder to original case), nom_on_lower_required (Result variant), nom_parse_lower (discard remainder)
├── error.rs        — OracleResult type, parse_or_unimplemented error boundary, format_verbose_error
├── context.rs      — ParseContext for stateful parsing
└── mod.rs          — Re-exports
```

**Key primitives in `primitives.rs`:**

| Combinator | What it parses | Notes |
|-----------|---------------|-------|
| `parse_number` | Digits, English words ("three"), articles ("a"/"an") | Word-boundary guard prevents "another" → "a" false match |
| `parse_number_or_x` | Same as above + "x" → 0 | Use for costs/P/T/counters where X is variable |
| `parse_mana_symbol` | `{W}`, `{U/B}`, `{R/P}`, `{2/W}`, `{X}`, `{S}` | Full hybrid/phyrexian/generic support |
| `parse_mana_cost` | `{2}{W}{U}` → `ManaCost` | Accumulates generic mana correctly |
| `parse_color` | "white", "blue", "black", "red", "green" → `ManaColor` | |
| `parse_counter_type` | "+1/+1", "-1/-1", "loyalty", "charge", etc. | |
| `parse_pt_modifier` | "+2/+3", "-1/-1", "+3/-2" → `(i32, i32)` | Handles mixed signs |
| `parse_roman_numeral` | I through XX → `u32` | Case-insensitive, for saga/class/level |

**Error boundary — `parse_or_unimplemented` in `error.rs`:**

At the dispatcher level (`oracle.rs`), `dispatch_line_nom` wraps nom combinators with
`parse_or_unimplemented`, which converts nom errors into `Effect::Unimplemented` with
diagnostic traces. Partial parses (non-empty remainder) also become `Unimplemented`. This
ensures unparsed fragments never silently pass.

**Parser dispatch architecture:**

- **Nom combinators** handle all parsing dispatch — atomic operations (numbers, mana, colors, P/T), medium-level structural patterns (conditions, durations, quantities, target filters), sentence-level verb dispatch (via `tag()`/`alt()`/`nom_on_lower` bridge), and top-level routing via `dispatch_line_nom`.
- **`TextPair`** provides dual-string operations where both original-case and lowercase slices are needed simultaneously (subject-predicate decomposition, clause AST classification). `TextPair::strip_prefix` is the correct tool for these — it's a case-bridging utility, not parsing dispatch.

**When writing new parser code:**
- Use nom combinators in `oracle_nom/` or `tag()`/`alt()` with the `nom_on_lower` bridge for all parsing dispatch.
- Use `nom_on_lower` from `oracle_nom/bridge.rs` when bridging mixed-case text to nom. Use `tag().parse()` directly for already-lowercase input.
- `starts_with` is acceptable ONLY for: runtime array loops, char-level scanners, dynamic (non-literal) prefixes.
- All parser branches import from `oracle_nom` — use the shared combinators rather than
  reimplementing number/color/mana/condition parsing locally.

### Parse pipeline for a spell ability

The effect parser uses a two-phase approach: first build a `ClauseAst` (structured intermediate
representation), then lower it into typed `Effect` data.

```
parse_oracle_text()
  └── parse_effect_chain(text)             # splits "Sentence 1. Sentence 2." into sub_ability chain
        └── parse_effect_clause(sent)      # handles one sentence
              ├── try_parse_damage_prevention_disabled() # CR 614.16
              ├── strip_leading_duration()               # "until end of turn, …"
              ├── try_parse_still_a_type()               # "it's still a land" (CR 205.1a)
              ├── try_parse_for_each_effect()             # "draw a card for each [filter]" — delegates to parse_numeric_imperative_ast() + with_for_each_quantity() + thread_for_each_subject()
              └── parse_clause_ast(text) → lower_clause_ast(ast)
                    ├── Conditional { clause }            # "if X, Y" → lower body
                    ├── SubjectPredicate { subject, predicate }
                    │     (via try_parse_subject_predicate_ast)
                    │     ├── try_parse_subject_continuous_clause() # "creatures you control get…"
                    │     ├── try_parse_subject_become_clause()     # "~ becomes a [type]…"
                    │     ├── try_parse_subject_restriction_clause()# "~ can't attack…"
                    │     └── strip_subject_clause() → ImperativeFallback
                    └── Imperative { text } → lower_imperative_clause()
                          ├── try_parse_targeted_controller_gain_life()
                          ├── try_parse_compound_shuffle()     # multi-step shuffles
                          ├── try_split_targeted_compound()    # "tap X and put counter on it"
                          └── parse_imperative_effect()        # bare verb phrases
```

The `ClauseAst` enum separates sentence structure from effect lowering:
- **`Imperative`** — bare verb phrases ("draw two cards", "exile target creature")
- **`SubjectPredicate`** — subject + verb ("creatures you control get +1/+1")
- **`Conditional`** — "if X, Y" wrappers (body is lowered recursively)

---

## Subject Stripping — The Key Design Decision

`strip_subject_clause` removes subjects like "you", "target creature", "its controller" and
recurses on the predicate. This simplifies parsing for most effects — but **it discards semantic
information**.

**Rule:** If the subject encodes game-relevant information (i.e. it changes *who* the effect
applies to), you **must** intercept the text *before* `strip_subject_clause` is called, using a
dedicated `try_parse_*` helper that preserves the subject's meaning.

In the current AST-based pipeline, subject interception happens at two levels:
1. **In `try_parse_subject_predicate_ast`** — for subject-verb clauses like "creatures you control
   get +1/+1" (continuous, become, restriction predicates).
2. **In `lower_imperative_clause`** — for imperative clauses where the subject is semantically
   critical (e.g. `try_parse_targeted_controller_gain_life`).

### Example: "Its controller gains life equal to its power"

❌ Wrong approach — letting `strip_subject_clause` handle it:
```
"Its controller gains life equal to its power"
    → strip_subject_clause strips "Its controller"
    → parse "gains life equal to its power"
    → GainLife { amount: Fixed(1), player: Controller }  ← BUG: wrong player, wrong amount
```

✅ Correct approach — intercept in `lower_imperative_clause`, before `parse_imperative_effect`:
```rust
// In lower_imperative_clause, BEFORE parse_imperative_effect:
if let Some(clause) = try_parse_targeted_controller_gain_life(text) {
    return clause;
}
```
```rust
fn try_parse_targeted_controller_gain_life(text: &str) -> Option<ParsedEffectClause> {
    let lower = text.to_lowercase();
    if !lower.starts_with("its controller ") { return None; }
    // … parse amount and player, preserving semantic context
    Some(parsed_clause(Effect::GainLife {
        amount: QuantityExpr::Ref { qty: QuantityRef::TargetPower },
        player: GainLifePlayer::TargetedController,
    }))
}
```

---

## Adding a New Effect Type

### Step 1 — Add the variant to `Effect` in `types/ability.rs`

Follow existing patterns:
- Use enum fields for variants that carry distinct data (e.g. `QuantityExpr`, `QuantityRef`).
- **Never use boolean flags** as a substitute for a proper enum variant. Boolean flags create
  undefined combinations and obscure intent.
- Use `QuantityExpr` for any amount/count field — never raw `i32` on new effects.
- Mark optional fields `#[serde(default)]` so old card-data.json files are still deserializable.
- Add the variant name to `effect_variant_name()` and a dispatch arm to `resolve_effect()`.

```rust
// Good: QuantityExpr separates fixed constants from dynamic game-state references
Draw { count: QuantityExpr },
DealDamage { amount: QuantityExpr, target: TargetFilter },

// Bad: raw integer with boolean flag
Draw { count: i32, use_variable: bool }  // ← DON'T DO THIS
```

### Step 2 — Handle the effect in `game/effects/`

Create or extend an effect handler in `crates/engine/src/game/effects/`:
- One `resolve_*` function per logical operation.
- Never access card data or parse text in effect handlers — only process the typed `ResolvedAbility`.
- Register the new effect variant in `game/effects/mod.rs::resolve_effect()`.

### Step 3 — Add the parser logic in `oracle_effect/`

- **Bare verb forms** (e.g. "exile target creature"): add a pattern in the relevant `parse_*_ast()` helper in `oracle_effect/imperative.rs`.
- **Subject-preserving effects** (e.g. "its controller gains life"): add a `try_parse_*` helper
  in `lower_imperative_clause()` (in `oracle_effect/mod.rs`), before `parse_imperative_effect()` is called.
- **Subject-predicate effects** (e.g. "creatures you control get +1/+1"): extend
  `try_parse_subject_predicate_ast()` in `oracle_effect/subject.rs` or add a new predicate variant to `PredicateAst`.
- **"For each" patterns**: `try_parse_for_each_effect()` in `oracle_effect/mod.rs` delegates verb parsing to `parse_numeric_imperative_ast()` via `with_for_each_quantity()`, then threads subject via `thread_for_each_subject()`. For new verbs, extend `parse_numeric_imperative_ast()` in `imperative.rs`. For new quantity clauses, extend `parse_for_each_clause()` in `oracle_quantity.rs`. Non-numeric for-each patterns (DealDamage, Token, PutCounter) are handled as separate branches using their existing building blocks.
- Use `strip_prefix()` over manual index arithmetic to avoid clippy warnings.
- Return `Effect::Unimplemented { name, description }` for patterns that are recognized but
  not yet implemented rather than panicking or silently returning a wrong effect.

### Step 4 — Write parser tests

Every new parser pattern must have a test in the relevant `oracle_effect/` sub-module:
```rust
#[test]
fn effect_its_controller_gains_life_equal_to_power() {
    let e = parse_effect("Its controller gains life equal to its power");
    assert!(matches!(
        e,
        Effect::GainLife {
            amount: QuantityExpr::Ref { qty: QuantityRef::TargetPower },
            player: GainLifePlayer::TargetedController,
        }
    ));
}
```

---

## Sub-Ability Chains

`parse_effect_chain` splits Oracle text on ". " boundaries and links each clause as a
`sub_ability`. At runtime, `game/effects/mod.rs::resolve_ability_chain` walks this chain.

**Target propagation:** When a parent ability has targets but the sub-ability does not, the engine
propagates the parent's targets to the sub-ability. This allows sub-effects like "its controller
gains life" (in the Swords to Plowshares chain) to access the targeted creature without
duplicating target information in the data model.

This means:
- Parser sub-abilities do **not** need to store their own target lists.
- Effect handlers may receive targets from the parent ability even when `ability.targets` was
  empty in the raw `AbilityDefinition`.

---

## Amounts — `QuantityExpr` and `QuantityRef`

Effects that carry a count or amount (`Draw`, `DealDamage`, `GainLife`, `LoseLife`, `Mill`) use
`QuantityExpr` instead of raw integers. This separates **fixed constants** from **dynamic
game-state lookups** at the type level:

```rust
pub enum QuantityExpr {
    Ref { qty: QuantityRef },   // dynamic — resolved from game state at runtime
    Fixed { value: i32 },       // literal constant
}

pub enum QuantityRef {
    HandSize,                              // cards in controller's hand
    LifeTotal,                             // controller's life total
    GraveyardSize,                         // cards in controller's graveyard
    LifeAboveStarting,                     // life - starting life (CR 107.1)
    ObjectCount { filter: TargetFilter },  // "for each creature you control"
    PlayerCount { filter: PlayerFilter },  // "for each opponent who lost life"
    CountersOnSelf { counter_type: String },// "for each [type] counter on ~"
    CountersOnTarget { counter_type: String },// "for each [type] counter on that creature"
    Variable { name: String },             // "X", "that much"
    TargetPower,                           // power of targeted permanent
    TrackedSetSize,                        // "for each card [moved] this way"
}
```

**Mapping Oracle text → `QuantityExpr`:**

| Oracle phrase                              | Type / variant                                     |
|--------------------------------------------|----------------------------------------------------|
| "3 damage" / "2 life"                      | `QuantityExpr::Fixed { value: N }`                 |
| "damage equal to its power"                | `QuantityExpr::Ref { qty: QuantityRef::TargetPower }` |
| "X damage"                                 | `QuantityExpr::Ref { qty: QuantityRef::Variable { name: "X" } }` |
| "a card for each creature you control"     | `QuantityExpr::Ref { qty: QuantityRef::ObjectCount { filter } }` |
| "a card for each opponent who lost life"   | `QuantityExpr::Ref { qty: QuantityRef::PlayerCount { filter } }` |

**Rules:**
- When parsing "equal to its power" / "for each [filter]", always return a `QuantityRef` variant —
  never `Fixed { value: 0 }` as a sentinel.
- `QuantityRef` contains only dynamic references that require game-state lookup. Constants
  (`Fixed`) belong in `QuantityExpr`, not `QuantityRef` — this is the "separate abstraction layers"
  principle (see CLAUDE.md).

**Legacy amount types** (`DamageAmount`, `LifeAmount`) still exist for backward compatibility but
new effects should use `QuantityExpr`.

**Zone-aware counting:** `ObjectCount` uses `TargetFilter::extract_in_zone()` to determine which
zone to iterate. If the filter contains an `InZone` property (e.g., `InZone: Graveyard`), objects
from that zone are counted instead of battlefield. Without `InZone`, defaults to battlefield.
The reusable helper `targeting::zone_object_ids(state, zone)` returns all object IDs in a zone.

---

## Replacement Effect Parser — `oracle_replacement.rs`

`parse_replacement_line` classifies replacement effects by priority. **Order matters** — patterns
that are subsets of other patterns must be checked later:

```
parse_replacement_line(text, card_name)
  ├── parse_as_enters_choose()          # "As ~ enters, choose a [type]" (must be BEFORE shock)
  ├── parse_shock_land()                # "you may pay N life. If you don't, enters tapped"
  ├── parse_fast_land()                 # "enters tapped unless you control N or fewer other [type]"
  ├── parse_check_land()                # "enters tapped unless you control a [LandType] or..."
  ├── parse_external_enters_tapped()    # "Creatures your opponents control enter tapped" (CR 614.12)
  ├── unconditional enters tapped       # "~ enters the battlefield tapped"
  ├── parse_graveyard_exile_replacement()  # "If a card would be put into a graveyard, exile it"
  ├── "~ would die" / "~ would be destroyed"
  ├── "Prevent all [combat] damage"
  ├── "you would draw" / "you would gain life" / "would lose life"
  └── parse_enters_with_counters()      # "~ enters with N [type] counter(s)"
```

Replacement definitions use the builder pattern:
```rust
ReplacementDefinition::new(ReplacementEvent::Moved)
    .execute(ability)
    .condition(ReplacementCondition::UnlessControlsSubtype { subtypes })
    .valid_card(filter)
    .destination_zone(Zone::Battlefield)
    .description(text)
```

`ReplacementCondition` encodes land-cycle conditions as typed variants:

| Land cycle   | Condition variant                                 |
|--------------|---------------------------------------------------|
| Check lands  | `UnlessControlsSubtype { subtypes: Vec<String> }` |
| Fast lands   | `UnlessControlsOtherLeq { count, filter }`        |
| Shock lands  | `ReplacementMode::Optional { decline: Some(…) }`  |

### Adding a new replacement pattern

1. Add a `parse_*` function matching the Oracle text pattern.
2. Insert it at the correct priority in `parse_replacement_line` — before any pattern it overlaps with.
3. Add parser tests in the `#[cfg(test)]` module.

---

## Event-Context References — `parse_event_context_ref`

Trigger effects often reference entities from the triggering event rather than targeting a player
or permanent. `parse_event_context_ref()` in `oracle_target.rs` handles these anaphoric references:

| Oracle phrase                    | `TargetFilter` variant          |
|----------------------------------|---------------------------------|
| "that spell's controller"       | `TriggeringSpellController`     |
| "that spell's owner"            | `TriggeringSpellOwner`          |
| "that player"                   | `TriggeringPlayer`              |
| "that source" / "that permanent"| `TriggeringSource`              |
| "defending player"              | `DefendingPlayer` (CR 506.3d)   |

**Rule:** `parse_event_context_ref` must be checked **before** standard `parse_target` for
trigger-based effects. These filters resolve at runtime from the triggering event context, not
from targeting.

### Other notable `TargetFilter` variants

| Variant                           | Purpose                                               |
|-----------------------------------|-------------------------------------------------------|
| `ParentTarget`                    | Resolves to same targets as parent ability (compound effects) |
| `TrackedSet { id: TrackedSetId }` | CR 603.7: anaphoric pronoun resolution for delayed triggers ("those cards", "the exiled cards") |

---

## Self-Reference Normalization (`~`) and `SELF_REF_TYPE_PHRASES`

Before any condition or effect text is parsed, `normalize_self_refs` replaces the card's own name
and phrases like "this creature", "this enchantment", "this artifact" with `~` (tilde). This
normalization happens in the trigger parser (`oracle_trigger.rs`) but the effect parser also
receives `~`-normalized text when parsing trigger effects.

`parse_target` in `oracle_target.rs` recognizes self-references in two ways:
- `~` (tilde) → `SelfRef` — for normalized text
- `SELF_REF_TYPE_PHRASES` ("this creature", "this permanent", etc.) → `SelfRef` — for
  un-normalized text (e.g. activated ability effects that are parsed before normalization)

The canonical phrase list lives in `oracle_util.rs` as `SELF_REF_TYPE_PHRASES` and is shared by
three consumers: `parse_target` (prefix matching), `subject.rs` (exact matching), and
`normalize_card_name_refs` (word-boundary replacement). When adding a new "this \<type\>" phrase,
update the shared constant — not each consumer individually.

**Rule:** Any parser function that checks for self-references must recognize `~` alongside explicit
phrases like "this creature" or "it". `parse_target` in `oracle_target.rs` handles both `~` and
`SELF_REF_TYPE_PHRASES` → `SelfRef` at the root level, so any effect that delegates to
`parse_target` automatically gets this behavior.

```
"put a +1/+1 counter on Ajani's Pridemate"
  → normalize_self_refs → "put a +1/+1 counter on ~"
  → try_parse_put_counter → PutCounter { target: SelfRef }  ✅
```

---

## Trigger Parser — Subject + Event Decomposition

`oracle_trigger.rs` parses trigger conditions into `TriggerDefinition` structs. The parser uses a
**subject + event decomposition** pattern:

```
parse_trigger_line(text, card_name)
  └── normalize_self_refs()              # card name / "this creature" → ~
  └── split_trigger()                    # split "condition, effect" at first ", "
  └── parse_trigger_condition(condition) # decompose into subject + event
        ├── try_parse_phase_trigger()     # "At the beginning of..."
        ├── try_parse_player_trigger()    # "you gain life", "you cast a spell"
        └── parse_trigger_subject()       # "~", "another creature you control", "a creature"
            └── try_parse_event()         # "enters", "dies", "attacks", "deals damage"
                └── try_parse_counter_trigger()  # "counter is put on ~"
  └── parse_trigger_constraint()         # "triggers only once each turn"
```

### Adding a new trigger event

1. Add a pattern in `try_parse_event()` matching the event verb (e.g. `"leaves the battlefield"`).
2. Set the appropriate `TriggerMode`, `origin`/`destination` zones, and wire the subject into
   `valid_card` or `valid_source`.
3. Add parser tests in the `tests` module.

### Adding a new trigger subject

1. Add a pattern in `parse_trigger_subject()` (e.g. `"each creature"`, `"a nontoken creature"`).
2. Use `parse_type_phrase()` from `oracle_target.rs` for type/controller/property parsing.
3. Compose with `FilterProp::Another` for exclusion patterns ("another creature").

### Trigger constraints

`TriggerConstraint` models rate-limiting on triggers:

| Oracle text | Variant |
|------------|---------|
| "This ability triggers only once each turn." | `OncePerTurn` |
| "This ability triggers only once." | `OncePerGame` |
| "only during your turn" | `OnlyDuringYourTurn` |

Parsed from the full trigger text in `parse_trigger_constraint()`. The runtime enforces constraints
in `process_triggers()` using `(ObjectId, trigger_index)` tracking sets on `GameState`.

---

## Static Ability Parser — Turn-Condition Handling

`oracle_static.rs` handles turn conditions in both **prefix** and **suffix** forms:

| Oracle text form | Handler |
|-----------------|---------|
| "During your turn, ~ has first strike." | Prefix: `nom_tag_tp("during your turn, ")` at line ~342 |
| "~ has first strike during your turn." | Suffix: `strip_suffix_turn_condition()` in both self-ref and `parse_subject_continuous_static` paths |
| "During turns other than yours, ~ has hexproof." | Prefix: `nom_tag_tp("during turns other than yours, ")` |
| "~ has hexproof during turns other than yours." | Suffix: same `strip_suffix_turn_condition()` |

The `strip_suffix_turn_condition(text)` helper returns `(stripped_text, Option<StaticCondition>)`.
Both forms produce identical output: `StaticCondition::DuringYourTurn` (or `Not { DuringYourTurn }`).

---

## Common Pitfalls

| Pitfall | Correct approach |
|---------|-----------------|
| Manual index arithmetic `&text[n..]` | Use nom `tag()`/`nom_on_lower` for parsing dispatch; `strip_prefix()` for TextPair/structural operations. Never use `&text[N..]` |
| Reimplementing number/color/mana parsing | Delegate to `oracle_nom::primitives` combinators |
| Using `nom::tag("a")` without word boundary | Use `parse_article_number` (prevents "another" → "a") |
| Using `parse_number` for X-cost values | Use `parse_number_or_x` (X → 0 at parse time) |
| `unwrap()` on parse results | Return `None` or `Effect::Unimplemented` instead |
| Losing subject context via `strip_subject_clause` | Add `try_parse_*` before the strip call |
| Boolean flags on effect types | Use an enum variant |
| `parse_number("equal to its power")` → `unwrap_or(1)` | Detect the "equal to" pattern first |
| Hardcoding `amount: 1` as default when text is unparseable | Prefer `Unimplemented` so the gap is visible in coverage reports |
| Not recognizing `~` as self-reference in effect parsers | Always check for `~` alongside "this creature", "it", etc. — `parse_target` handles this |
| Monolithic condition parsing | Use subject+event decomposition — add subjects and events independently |
| Raw `i32` for effect amounts on new effects | Use `QuantityExpr` — separates fixed constants from dynamic game-state lookups |
| Splitting compound effects on " and " naively | Use `try_split_targeted_compound` which delegates to `parse_target` for boundary detection |
| Putting `Fixed(i32)` inside `QuantityRef` | `QuantityRef` is only for dynamic references; constants go in `QuantityExpr::Fixed` |
| `starts_with("verb ")` for dispatch | Use `tag("verb ").parse(lower)` or `nom_on_lower` bridge — keeps prefix and consumed length in sync |
| `&text[N..]` hardcoded byte offset after prefix match | Use `nom_on_lower` which calculates remainder automatically from combinator output |
| Inline `use nom::*` inside function bodies | All nom imports at file top — CLAUDE.md prohibits inline imports |
