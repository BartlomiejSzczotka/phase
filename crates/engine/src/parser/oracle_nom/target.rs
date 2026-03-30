//! Target phrase combinators for Oracle text parsing.
//!
//! Parses "target creature", "target creature or planeswalker you control", etc.
//! into typed `TargetFilter` values using nom 8.0 combinators.

use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::space1;
use nom::combinator::{opt, value};
use nom::sequence::preceded;
use nom::Parser;

use super::error::OracleResult;
use super::primitives::parse_color;
use crate::types::ability::{ControllerRef, TargetFilter, TypeFilter, TypedFilter};
use crate::types::mana::ManaColor;

/// Parse a "target <type phrase>" from Oracle text.
///
/// Matches "target creature", "target artifact or enchantment you control", etc.
pub fn parse_target_phrase(input: &str) -> OracleResult<'_, TargetFilter> {
    preceded((tag("target"), space1), parse_type_phrase).parse(input)
}

/// Parse a type phrase into a `TargetFilter`.
///
/// Handles: optional color prefix, core type(s) joined by " or ",
/// and optional controller suffix. This is the nom equivalent of
/// `oracle_target::parse_type_phrase`.
pub fn parse_type_phrase(input: &str) -> OracleResult<'_, TargetFilter> {
    // Optional color prefix
    let (rest, color_opt) = opt(parse_color_prefix).parse(input)?;

    // Core type(s) joined by " or "
    let (rest, types) = parse_type_list(rest)?;

    // Optional controller suffix
    let (rest, controller) = opt(preceded(space1, parse_controller_suffix)).parse(rest)?;

    let filter = build_type_filter(types, color_opt, controller);
    Ok((rest, filter))
}

/// Parse a color word followed by a space, consuming both.
fn parse_color_prefix(input: &str) -> OracleResult<'_, ManaColor> {
    let (rest, c) = parse_color(input)?;
    let (rest, _) = space1.parse(rest)?;
    Ok((rest, c))
}

/// Parse a controller suffix: "you control", "an opponent controls".
pub fn parse_controller_suffix(input: &str) -> OracleResult<'_, ControllerRef> {
    alt((
        value(ControllerRef::You, tag("you control")),
        value(ControllerRef::Opponent, tag("an opponent controls")),
        value(ControllerRef::Opponent, tag("your opponents control")),
    ))
    .parse(input)
}

/// Parse a list of type filters joined by " or ".
fn parse_type_list(input: &str) -> OracleResult<'_, Vec<TypeFilter>> {
    let (rest, first) = parse_type_filter_word(input)?;
    let mut types = vec![first];

    let mut remaining = rest;
    loop {
        if let Ok((r, _)) =
            tag::<_, _, nom_language::error::VerboseError<&str>>(" or ").parse(remaining)
        {
            if let Ok((r2, t)) = parse_type_filter_word(r) {
                types.push(t);
                remaining = r2;
                continue;
            }
        }
        break;
    }

    Ok((remaining, types))
}

/// Parse a single type filter word.
fn parse_type_filter_word(input: &str) -> OracleResult<'_, TypeFilter> {
    alt((
        value(TypeFilter::Creature, tag("creature")),
        value(TypeFilter::Artifact, tag("artifact")),
        value(TypeFilter::Enchantment, tag("enchantment")),
        value(TypeFilter::Instant, tag("instant")),
        value(TypeFilter::Sorcery, tag("sorcery")),
        value(TypeFilter::Planeswalker, tag("planeswalker")),
        value(TypeFilter::Land, tag("land")),
        value(TypeFilter::Battle, tag("battle")),
        value(TypeFilter::Permanent, tag("permanent")),
        value(TypeFilter::Card, tag("card")),
    ))
    .parse(input)
}

/// Build a `TargetFilter` from parsed components.
fn build_type_filter(
    types: Vec<TypeFilter>,
    _color: Option<ManaColor>,
    controller: Option<ControllerRef>,
) -> TargetFilter {
    let type_filters: Vec<TypeFilter> = if types.len() == 1 {
        types
    } else {
        vec![TypeFilter::AnyOf(types)]
    };

    TargetFilter::Typed(TypedFilter {
        type_filters,
        controller,
        properties: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_target_phrase_creature() {
        let (rest, filter) = parse_target_phrase("target creature with power").unwrap();
        assert_eq!(rest, " with power");
        match filter {
            TargetFilter::Typed(tf) => {
                assert_eq!(tf.type_filters, vec![TypeFilter::Creature]);
            }
            _ => panic!("expected Typed filter"),
        }
    }

    #[test]
    fn test_parse_target_phrase_artifact_or_enchantment() {
        let (rest, filter) =
            parse_target_phrase("target artifact or enchantment you control").unwrap();
        assert_eq!(rest, "");
        match filter {
            TargetFilter::Typed(tf) => {
                assert_eq!(
                    tf.type_filters,
                    vec![TypeFilter::AnyOf(vec![
                        TypeFilter::Artifact,
                        TypeFilter::Enchantment
                    ])]
                );
                assert_eq!(tf.controller, Some(ControllerRef::You));
            }
            _ => panic!("expected Typed filter"),
        }
    }

    #[test]
    fn test_parse_target_phrase_no_target_prefix() {
        assert!(parse_target_phrase("creature").is_err());
    }

    #[test]
    fn test_parse_controller_suffix() {
        let (rest, c) = parse_controller_suffix("you control stuff").unwrap();
        assert_eq!(c, ControllerRef::You);
        assert_eq!(rest, " stuff");

        let (rest2, c2) = parse_controller_suffix("an opponent controls").unwrap();
        assert_eq!(c2, ControllerRef::Opponent);
        assert_eq!(rest2, "");
    }

    #[test]
    fn test_parse_type_phrase_single() {
        let (rest, filter) = parse_type_phrase("creature you control").unwrap();
        assert_eq!(rest, "");
        match filter {
            TargetFilter::Typed(tf) => {
                assert_eq!(tf.type_filters, vec![TypeFilter::Creature]);
                assert_eq!(tf.controller, Some(ControllerRef::You));
            }
            _ => panic!("expected Typed filter"),
        }
    }

    #[test]
    fn test_parse_type_phrase_multi() {
        let (rest, filter) = parse_type_phrase("instant or sorcery").unwrap();
        assert_eq!(rest, "");
        match filter {
            TargetFilter::Typed(tf) => {
                assert_eq!(
                    tf.type_filters,
                    vec![TypeFilter::AnyOf(vec![
                        TypeFilter::Instant,
                        TypeFilter::Sorcery
                    ])]
                );
            }
            _ => panic!("expected Typed filter"),
        }
    }
}
