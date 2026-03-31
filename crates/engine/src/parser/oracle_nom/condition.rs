//! Condition combinators for Oracle text parsing.
//!
//! Parses condition phrases: "if [condition]", "as long as [condition]",
//! "unless [condition]" into typed `StaticCondition` values.

use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::combinator::{map, value};
use nom::sequence::preceded;
use nom::Parser;

use super::error::OracleResult;
use super::primitives::parse_number;
use crate::parser::oracle_target::parse_type_phrase;
use crate::types::ability::{
    Comparator, ControllerRef, QuantityExpr, QuantityRef, StaticCondition, TargetFilter,
};

/// Parse a condition phrase from Oracle text.
///
/// Matches patterns like "if you control a creature", "as long as you have no
/// cards in hand", "unless an opponent controls a creature".
pub fn parse_condition(input: &str) -> OracleResult<'_, StaticCondition> {
    alt((
        preceded(tuple_ws_tag("if "), parse_inner_condition),
        preceded(tuple_ws_tag("as long as "), parse_inner_condition),
        preceded(tuple_ws_tag("unless "), parse_unless_condition),
    ))
    .parse(input)
}

/// Parse an "if" or "as long as" condition without the prefix keyword.
///
/// Useful when the prefix has already been consumed by the caller.
pub fn parse_inner_condition(input: &str) -> OracleResult<'_, StaticCondition> {
    alt((
        parse_turn_conditions,
        parse_source_state_conditions,
        parse_you_have_conditions,
        parse_control_conditions,
        parse_life_conditions,
        parse_zone_conditions,
    ))
    .parse(input)
}

/// Helper: tag with potential leading whitespace trimmed.
fn tuple_ws_tag(t: &str) -> impl FnMut(&str) -> OracleResult<'_, &str> + '_ {
    move |input: &str| tag(t).parse(input)
}

/// Parse turn-based conditions.
fn parse_turn_conditions(input: &str) -> OracleResult<'_, StaticCondition> {
    alt((
        value(StaticCondition::DuringYourTurn, tag("it's your turn")),
        value(StaticCondition::DuringYourTurn, tag("it is your turn")),
        // "it's not your turn" → Not(DuringYourTurn)
        map(tag("it's not your turn"), |_| StaticCondition::Not {
            condition: Box::new(StaticCondition::DuringYourTurn),
        }),
    ))
    .parse(input)
}

/// Parse source-state conditions (tapped, untapped, in-zone).
fn parse_source_state_conditions(input: &str) -> OracleResult<'_, StaticCondition> {
    alt((
        value(StaticCondition::SourceIsTapped, tag("~ is tapped")),
        // "~ is untapped" → Not(SourceIsTapped) per existing convention
        map(tag("~ is untapped"), |_| StaticCondition::Not {
            condition: Box::new(StaticCondition::SourceIsTapped),
        }),
        value(
            StaticCondition::SourceEnteredThisTurn,
            tag("~ entered the battlefield this turn"),
        ),
        value(StaticCondition::IsRingBearer, tag("~ is your ring-bearer")),
    ))
    .parse(input)
}

/// Parse "you have" quantity conditions: hand size, graveyard size, life.
///
/// Composable: "you have " + threshold/absence + quantity suffix.
/// Handles "you have no cards in hand", "you have N or more cards in hand",
/// "you have N or more cards in your graveyard", "you have N or more life".
fn parse_you_have_conditions(input: &str) -> OracleResult<'_, StaticCondition> {
    let (rest, _) = tag("you have ").parse(input)?;

    // "you have no cards in hand" → HandSize EQ 0
    if let Ok((rest, _)) = tag::<_, _, nom_language::error::VerboseError<&str>>("no cards in hand")
        .parse(rest)
    {
        return Ok((
            rest,
            StaticCondition::QuantityComparison {
                lhs: QuantityExpr::Ref {
                    qty: QuantityRef::HandSize,
                },
                comparator: Comparator::EQ,
                rhs: QuantityExpr::Fixed { value: 0 },
            },
        ));
    }

    // "you have N or more [quantity-suffix]"
    let (rest, n) = parse_number(rest)?;

    // Try each quantity suffix
    if let Ok((rest, _)) = tag::<_, _, nom_language::error::VerboseError<&str>>(
        " or more cards in hand",
    )
    .parse(rest)
    {
        return Ok((
            rest,
            make_quantity_ge(QuantityRef::HandSize, n),
        ));
    }
    if let Ok((rest, _)) = tag::<_, _, nom_language::error::VerboseError<&str>>(
        " or more cards in your graveyard",
    )
    .parse(rest)
    {
        return Ok((
            rest,
            make_quantity_ge(QuantityRef::GraveyardSize, n),
        ));
    }
    if let Ok((rest, _)) =
        tag::<_, _, nom_language::error::VerboseError<&str>>(" or more life").parse(rest)
    {
        return Ok((
            rest,
            make_quantity_ge(QuantityRef::LifeTotal, n),
        ));
    }

    Err(nom::Err::Error(nom_language::error::VerboseError {
        errors: vec![(
            input,
            nom_language::error::VerboseErrorKind::Nom(nom::error::ErrorKind::Tag),
        )],
    }))
}

/// Build a QuantityComparison: qty >= n.
fn make_quantity_ge(qty: QuantityRef, n: u32) -> StaticCondition {
    StaticCondition::QuantityComparison {
        lhs: QuantityExpr::Ref { qty },
        comparator: Comparator::GE,
        rhs: QuantityExpr::Fixed { value: n as i32 },
    }
}

/// Parse "you control" condition patterns.
fn parse_control_conditions(input: &str) -> OracleResult<'_, StaticCondition> {
    alt((
        // "you control N or more [type]" → QuantityComparison(ObjectCount >= N)
        parse_control_count_ge,
        // "you control a/an [type]" → IsPresent with filter
        parse_you_control_a,
        // "you don't control a/an [type]" → Not(IsPresent)
        parse_you_dont_control_a,
    ))
    .parse(input)
}

/// Canonical combinator: "you control N or more [type]" → QuantityComparison.
///
/// Single authority for this pattern — called from `oracle_static.rs` and
/// `oracle_trigger.rs` to avoid three-way duplication.
/// Returns the remainder after the type phrase (may be non-empty for trailing text).
pub fn parse_control_count_ge(input: &str) -> OracleResult<'_, StaticCondition> {
    let (rest, _) = tag("you control ").parse(input)?;
    let (rest, n) = parse_number(rest)?;
    let rest = rest.trim_start();
    let (rest, _) = tag("or more ").parse(rest)?;
    let type_text = rest.trim_end_matches('.');
    let (filter, remainder) = parse_type_phrase(type_text);
    if matches!(filter, TargetFilter::Any) {
        return Err(nom::Err::Error(nom_language::error::VerboseError {
            errors: vec![(
                input,
                nom_language::error::VerboseErrorKind::Nom(nom::error::ErrorKind::Tag),
            )],
        }));
    }
    let filter = inject_controller_you(filter);
    // Map remainder back to original input slice — parse_type_phrase consumed
    // from a trimmed copy, so use byte offset from the original.
    let consumed = input.len() - remainder.len();
    Ok((
        &input[consumed..],
        StaticCondition::QuantityComparison {
            lhs: QuantityExpr::Ref {
                qty: QuantityRef::ObjectCount { filter },
            },
            comparator: Comparator::GE,
            rhs: QuantityExpr::Fixed { value: n as i32 },
        },
    ))
}

/// Parse "you control a/an [type]" → IsPresent with filter.
///
/// Generalized: uses `parse_type_phrase` so any type phrase is supported,
/// not just hardcoded creature/artifact/enchantment/planeswalker.
fn parse_you_control_a(input: &str) -> OracleResult<'_, StaticCondition> {
    let (rest, _) = alt((tag("you control a "), tag("you control an "))).parse(input)?;
    let (filter, remainder) = parse_type_phrase(rest);
    if matches!(filter, TargetFilter::Any) {
        return Err(nom::Err::Error(nom_language::error::VerboseError {
            errors: vec![(
                input,
                nom_language::error::VerboseErrorKind::Nom(nom::error::ErrorKind::Tag),
            )],
        }));
    }
    let filter = inject_controller_you(filter);
    let consumed = input.len() - remainder.len();
    Ok((
        &input[consumed..],
        StaticCondition::IsPresent {
            filter: Some(filter),
        },
    ))
}

/// Parse "you don't control a/an [type]" → Not(IsPresent).
fn parse_you_dont_control_a(input: &str) -> OracleResult<'_, StaticCondition> {
    let (rest, _) = alt((tag("you don't control a "), tag("you don't control an ")))
        .parse(input)?;
    let (filter, remainder) = parse_type_phrase(rest);
    if matches!(filter, TargetFilter::Any) {
        return Err(nom::Err::Error(nom_language::error::VerboseError {
            errors: vec![(
                input,
                nom_language::error::VerboseErrorKind::Nom(nom::error::ErrorKind::Tag),
            )],
        }));
    }
    let filter = inject_controller_you(filter);
    let consumed = input.len() - remainder.len();
    Ok((
        &input[consumed..],
        StaticCondition::Not {
            condition: Box::new(StaticCondition::IsPresent {
                filter: Some(filter),
            }),
        },
    ))
}

/// Inject `ControllerRef::You` into a TargetFilter produced by `parse_type_phrase`.
fn inject_controller_you(filter: TargetFilter) -> TargetFilter {
    match filter {
        TargetFilter::Typed(tf) => TargetFilter::Typed(tf.controller(ControllerRef::You)),
        other => other,
    }
}


/// Parse "your life total is N or less/greater" conditions.
///
/// Note: "you have N or more life" is handled by `parse_you_have_conditions`.
fn parse_life_conditions(input: &str) -> OracleResult<'_, StaticCondition> {
    let (rest, _) = tag("your life total is ").parse(input)?;
    let (rest, n) = parse_number(rest)?;
    // Try "or less" then "or greater"
    if let Ok((rest, _)) =
        tag::<_, _, nom_language::error::VerboseError<&str>>(" or less").parse(rest)
    {
        return Ok((
            rest,
            StaticCondition::QuantityComparison {
                lhs: QuantityExpr::Ref {
                    qty: QuantityRef::LifeTotal,
                },
                comparator: Comparator::LE,
                rhs: QuantityExpr::Fixed { value: n as i32 },
            },
        ));
    }
    let (rest, _) = tag(" or greater").parse(rest)?;
    Ok((
        rest,
        StaticCondition::QuantityComparison {
            lhs: QuantityExpr::Ref {
                qty: QuantityRef::LifeTotal,
            },
            comparator: Comparator::GE,
            rhs: QuantityExpr::Fixed { value: n as i32 },
        },
    ))
}

/// Parse zone-related conditions ("~ is in your graveyard").
fn parse_zone_conditions(input: &str) -> OracleResult<'_, StaticCondition> {
    alt((
        value(
            StaticCondition::SourceInZone {
                zone: crate::types::zones::Zone::Graveyard,
            },
            tag("~ is in your graveyard"),
        ),
        value(
            StaticCondition::SourceInZone {
                zone: crate::types::zones::Zone::Graveyard,
            },
            tag("this card is in your graveyard"),
        ),
        value(
            StaticCondition::SourceInZone {
                zone: crate::types::zones::Zone::Exile,
            },
            tag("~ is in exile"),
        ),
    ))
    .parse(input)
}

/// Parse an "unless" condition, wrapping the inner condition in `Not`.
fn parse_unless_condition(input: &str) -> OracleResult<'_, StaticCondition> {
    let (rest, inner) = parse_inner_condition(input)?;
    Ok((
        rest,
        StaticCondition::Not {
            condition: Box::new(inner),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_condition_your_turn() {
        let (rest, c) = parse_condition("if it's your turn, do").unwrap();
        assert_eq!(rest, ", do");
        assert_eq!(c, StaticCondition::DuringYourTurn);
    }

    #[test]
    fn test_parse_condition_as_long_as_tapped() {
        let (rest, c) = parse_condition("as long as ~ is tapped").unwrap();
        assert_eq!(rest, "");
        assert!(matches!(c, StaticCondition::SourceIsTapped));
    }

    #[test]
    fn test_parse_condition_no_cards() {
        let (rest, c) = parse_condition("if you have no cards in hand").unwrap();
        assert_eq!(rest, "");
        match c {
            StaticCondition::QuantityComparison {
                comparator, rhs, ..
            } => {
                assert_eq!(comparator, Comparator::EQ);
                assert_eq!(rhs, QuantityExpr::Fixed { value: 0 });
            }
            _ => panic!("expected QuantityComparison"),
        }
    }

    #[test]
    fn test_parse_condition_not_your_turn() {
        let (rest, c) = parse_condition("if it's not your turn").unwrap();
        assert_eq!(rest, "");
        match c {
            StaticCondition::Not { condition } => {
                assert_eq!(*condition, StaticCondition::DuringYourTurn);
            }
            _ => panic!("expected Not(DuringYourTurn)"),
        }
    }

    #[test]
    fn test_parse_condition_seven_cards() {
        let (rest, c) = parse_condition("if you have seven or more cards in hand").unwrap();
        assert_eq!(rest, "");
        match c {
            StaticCondition::QuantityComparison {
                comparator, rhs, ..
            } => {
                assert_eq!(comparator, Comparator::GE);
                assert_eq!(rhs, QuantityExpr::Fixed { value: 7 });
            }
            _ => panic!("expected QuantityComparison"),
        }
    }

    #[test]
    fn test_parse_condition_life_le() {
        let (rest, c) = parse_condition("if your life total is 5 or less").unwrap();
        assert_eq!(rest, "");
        match c {
            StaticCondition::QuantityComparison {
                comparator, rhs, ..
            } => {
                assert_eq!(comparator, Comparator::LE);
                assert_eq!(rhs, QuantityExpr::Fixed { value: 5 });
            }
            _ => panic!("expected QuantityComparison"),
        }
    }

    #[test]
    fn test_parse_condition_unless() {
        let (rest, c) = parse_condition("unless it's your turn").unwrap();
        assert_eq!(rest, "");
        match c {
            StaticCondition::Not { condition } => {
                assert_eq!(*condition, StaticCondition::DuringYourTurn);
            }
            _ => panic!("expected Not(DuringYourTurn)"),
        }
    }

    #[test]
    fn test_parse_condition_source_in_graveyard() {
        let (rest, c) = parse_condition("as long as ~ is in your graveyard").unwrap();
        assert_eq!(rest, "");
        assert!(matches!(
            c,
            StaticCondition::SourceInZone {
                zone: crate::types::zones::Zone::Graveyard
            }
        ));
    }

    #[test]
    fn test_parse_condition_ring_bearer() {
        let (rest, c) = parse_condition("as long as ~ is your ring-bearer").unwrap();
        assert_eq!(rest, "");
        assert_eq!(c, StaticCondition::IsRingBearer);
    }

    #[test]
    fn test_parse_condition_failure() {
        assert!(parse_condition("when something happens").is_err());
    }

    // -- Generalized control conditions --

    #[test]
    fn test_you_control_a_creature() {
        let (rest, c) = parse_inner_condition("you control a creature").unwrap();
        assert_eq!(rest, "");
        assert!(matches!(c, StaticCondition::IsPresent { filter: Some(_) }));
    }

    #[test]
    fn test_you_control_an_artifact() {
        let (rest, c) = parse_inner_condition("you control an artifact").unwrap();
        assert_eq!(rest, "");
        assert!(matches!(c, StaticCondition::IsPresent { filter: Some(_) }));
    }

    #[test]
    fn test_you_control_a_land() {
        // Generalized: works for any type phrase, not just hardcoded types
        let (rest, c) = parse_inner_condition("you control a land").unwrap();
        assert_eq!(rest, "");
        assert!(matches!(c, StaticCondition::IsPresent { filter: Some(_) }));
    }

    #[test]
    fn test_you_dont_control_a_creature() {
        let (rest, c) = parse_inner_condition("you don't control a creature").unwrap();
        assert_eq!(rest, "");
        assert!(matches!(c, StaticCondition::Not { .. }));
    }

    #[test]
    fn test_you_dont_control_an_artifact() {
        let (rest, c) = parse_inner_condition("you don't control an artifact").unwrap();
        assert_eq!(rest, "");
        assert!(matches!(c, StaticCondition::Not { .. }));
    }

    #[test]
    fn test_control_count_ge() {
        let (rest, c) =
            parse_inner_condition("you control three or more creatures").unwrap();
        assert_eq!(rest, "");
        match c {
            StaticCondition::QuantityComparison {
                comparator,
                rhs: QuantityExpr::Fixed { value: 3 },
                ..
            } => assert_eq!(comparator, Comparator::GE),
            other => panic!("expected QuantityComparison GE 3, got {other:?}"),
        }
    }

    #[test]
    fn test_control_count_ge_artifacts() {
        let (rest, c) =
            parse_inner_condition("you control two or more artifacts").unwrap();
        assert_eq!(rest, "");
        assert!(matches!(
            c,
            StaticCondition::QuantityComparison {
                comparator: Comparator::GE,
                ..
            }
        ));
    }

    #[test]
    fn test_graveyard_count_ge() {
        let (rest, c) =
            parse_inner_condition("you have five or more cards in your graveyard").unwrap();
        assert_eq!(rest, "");
        match c {
            StaticCondition::QuantityComparison {
                lhs:
                    QuantityExpr::Ref {
                        qty: QuantityRef::GraveyardSize,
                    },
                comparator: Comparator::GE,
                rhs: QuantityExpr::Fixed { value: 5 },
            } => {}
            other => panic!("expected GraveyardSize GE 5, got {other:?}"),
        }
    }
}
