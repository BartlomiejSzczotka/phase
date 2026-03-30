//! Filter combinators for Oracle text parsing.
//!
//! Parses zone filters ("on the battlefield", "in your graveyard") and
//! property filters ("tapped", "untapped", "attacking", "blocking").

use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::combinator::value;
use nom::Parser;

use super::error::OracleResult;
use crate::types::ability::FilterProp;
use crate::types::zones::Zone;

/// Parse a zone filter phrase from Oracle text.
///
/// Matches "on the battlefield", "in your graveyard", "in your hand",
/// "in exile", "in your library".
pub fn parse_zone_filter(input: &str) -> OracleResult<'_, Zone> {
    alt((
        value(Zone::Battlefield, tag("on the battlefield")),
        value(Zone::Graveyard, tag("in your graveyard")),
        value(Zone::Graveyard, tag("in a graveyard")),
        value(Zone::Hand, tag("in your hand")),
        value(Zone::Hand, tag("in a player's hand")),
        value(Zone::Exile, tag("in exile")),
        value(Zone::Library, tag("in your library")),
        value(Zone::Stack, tag("on the stack")),
    ))
    .parse(input)
}

/// Parse a property filter from Oracle text.
///
/// Matches object property keywords: "tapped", "untapped", "attacking",
/// "blocking", "token", "face down".
pub fn parse_property_filter(input: &str) -> OracleResult<'_, FilterProp> {
    alt((
        value(FilterProp::Tapped, tag("tapped")),
        value(FilterProp::Untapped, tag("untapped")),
        value(FilterProp::Attacking, tag("attacking")),
        value(FilterProp::Token, tag("token")),
        value(FilterProp::FaceDown, tag("face down")),
        value(FilterProp::Unblocked, tag("unblocked")),
    ))
    .parse(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_zone_filter_battlefield() {
        let (rest, z) = parse_zone_filter("on the battlefield this turn").unwrap();
        assert_eq!(z, Zone::Battlefield);
        assert_eq!(rest, " this turn");
    }

    #[test]
    fn test_parse_zone_filter_graveyard() {
        let (rest, z) = parse_zone_filter("in your graveyard").unwrap();
        assert_eq!(z, Zone::Graveyard);
        assert_eq!(rest, "");
    }

    #[test]
    fn test_parse_zone_filter_exile() {
        let (rest, z) = parse_zone_filter("in exile").unwrap();
        assert_eq!(z, Zone::Exile);
        assert_eq!(rest, "");
    }

    #[test]
    fn test_parse_zone_filter_failure() {
        assert!(parse_zone_filter("under the rug").is_err());
    }

    #[test]
    fn test_parse_property_filter_tapped() {
        let (rest, p) = parse_property_filter("tapped creatures").unwrap();
        assert_eq!(p, FilterProp::Tapped);
        assert_eq!(rest, " creatures");
    }

    #[test]
    fn test_parse_property_filter_attacking() {
        let (rest, p) = parse_property_filter("attacking").unwrap();
        assert_eq!(p, FilterProp::Attacking);
        assert_eq!(rest, "");
    }

    #[test]
    fn test_parse_property_filter_face_down() {
        let (rest, p) = parse_property_filter("face down").unwrap();
        assert_eq!(p, FilterProp::FaceDown);
        assert_eq!(rest, "");
    }

    #[test]
    fn test_parse_property_filter_failure() {
        assert!(parse_property_filter("flying").is_err());
    }
}
