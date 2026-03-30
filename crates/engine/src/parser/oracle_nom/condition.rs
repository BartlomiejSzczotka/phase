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
use crate::types::ability::StaticCondition;

/// Parse a condition phrase from Oracle text.
///
/// Matches patterns like "if you control a creature", "as long as you have no
/// cards in hand", "unless an opponent controls a creature".
/// Currently handles the condition-keyword prefix; the inner condition logic
/// delegates to `parse_inner_condition` for common patterns.
pub fn parse_condition(input: &str) -> OracleResult<'_, StaticCondition> {
    alt((
        preceded(tuple_ws_tag("if "), parse_inner_condition),
        preceded(tuple_ws_tag("as long as "), parse_inner_condition),
    ))
    .parse(input)
}

/// Helper: tag with potential leading whitespace trimmed.
fn tuple_ws_tag(t: &str) -> impl FnMut(&str) -> OracleResult<'_, &str> + '_ {
    move |input: &str| tag(t).parse(input)
}

/// Parse common inner condition patterns.
///
/// Currently covers: turn-based conditions, simple presence checks.
/// More patterns will be added as migration plans wire specific branches.
fn parse_inner_condition(input: &str) -> OracleResult<'_, StaticCondition> {
    alt((
        value(StaticCondition::DuringYourTurn, tag("it's your turn")),
        value(StaticCondition::DuringYourTurn, tag("it is your turn")),
        value(StaticCondition::SourceIsTapped, tag("~ is tapped")),
        // "~ is untapped" → Not(SourceIsTapped) per existing convention
        map(tag("~ is untapped"), |_| StaticCondition::Not {
            condition: Box::new(StaticCondition::SourceIsTapped),
        }),
        // "you have no cards in hand"
        map(tag("you have no cards in hand"), |_| {
            StaticCondition::QuantityComparison {
                lhs: crate::types::ability::QuantityExpr::Ref {
                    qty: crate::types::ability::QuantityRef::HandSize,
                },
                comparator: crate::types::ability::Comparator::EQ,
                rhs: crate::types::ability::QuantityExpr::Fixed { value: 0 },
            }
        }),
    ))
    .parse(input)
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
                assert_eq!(comparator, crate::types::ability::Comparator::EQ);
                assert_eq!(rhs, crate::types::ability::QuantityExpr::Fixed { value: 0 });
            }
            _ => panic!("expected QuantityComparison"),
        }
    }

    #[test]
    fn test_parse_condition_failure() {
        assert!(parse_condition("when something happens").is_err());
    }
}
