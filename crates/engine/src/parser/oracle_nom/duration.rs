//! Duration combinators for Oracle text parsing.
//!
//! Parses duration phrases: "until end of turn", "until your next turn",
//! "until end of combat", "for as long as [condition]".

use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::combinator::value;
use nom::Parser;

use super::error::OracleResult;
use crate::types::ability::Duration;

/// Parse a duration phrase from Oracle text.
///
/// Matches "until end of turn", "until your next turn", "until end of combat".
/// Does not yet handle "for as long as [condition]" -- that requires composing
/// with `parse_condition` and will be wired in migration plans.
pub fn parse_duration(input: &str) -> OracleResult<'_, Duration> {
    alt((
        value(Duration::UntilEndOfTurn, tag("until end of turn")),
        value(Duration::UntilEndOfCombat, tag("until end of combat")),
        value(Duration::UntilYourNextTurn, tag("until your next turn")),
    ))
    .parse(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_end_of_turn() {
        let (rest, d) = parse_duration("until end of turn.").unwrap();
        assert_eq!(d, Duration::UntilEndOfTurn);
        assert_eq!(rest, ".");
    }

    #[test]
    fn test_parse_duration_end_of_combat() {
        let (rest, d) = parse_duration("until end of combat").unwrap();
        assert_eq!(d, Duration::UntilEndOfCombat);
        assert_eq!(rest, "");
    }

    #[test]
    fn test_parse_duration_next_turn() {
        let (rest, d) = parse_duration("until your next turn and").unwrap();
        assert_eq!(d, Duration::UntilYourNextTurn);
        assert_eq!(rest, " and");
    }

    #[test]
    fn test_parse_duration_failure() {
        assert!(parse_duration("permanently").is_err());
    }
}
