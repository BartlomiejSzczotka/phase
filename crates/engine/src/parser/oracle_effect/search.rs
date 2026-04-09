use nom::bytes::complete::tag;
use nom::Parser;
use nom_language::error::VerboseError;

use super::super::oracle_nom::primitives as nom_primitives;
use super::super::oracle_target::{parse_mana_value_suffix, parse_type_phrase};
use super::super::oracle_util::{contains_possessive, strip_after};
use super::types::{SearchLibraryDetails, SeekDetails};
use super::{capitalize, scan_contains_phrase};
use crate::parser::oracle_warnings::push_warning;
use crate::types::ability::{
    FilterProp, QuantityExpr, QuantityRef, TargetFilter, TypeFilter, TypedFilter,
};
use crate::types::zones::Zone;

pub(super) fn parse_search_library_details(lower: &str) -> SearchLibraryDetails {
    let reveal = scan_contains_phrase(lower, "reveal");

    // Extract count from "up to N" (must be done before filter extraction since
    // "for up to five creature cards" needs to skip the count to find the type).
    // Delegate to nom combinator (input already lowercase).
    let (count, count_end_in_for) = if let Some(after_up_to) = strip_after(lower, "up to ") {
        if let Ok((rest, n)) = nom_primitives::parse_number.parse(after_up_to) {
            // Calculate the byte offset where the type text begins after "up to N "
            let type_start = lower.len() - rest.len();
            (n, Some(type_start))
        } else {
            (1, None)
        }
    } else {
        (1, None)
    };

    // Extract the type filter from after "for a/an" or "for up to N".
    let filter = if let Some(after_for) = strip_after(lower, "for a ") {
        parse_search_filter(after_for)
    } else if let Some(after_for) = strip_after(lower, "for an ") {
        parse_search_filter(after_for)
    } else if let Some(type_start) = count_end_in_for {
        // "for up to five creature cards" — type text starts after the number
        parse_search_filter(&lower[type_start..])
    } else {
        TargetFilter::Any
    };

    SearchLibraryDetails {
        filter,
        count,
        reveal,
    }
}

/// Parse "seek [count] [filter] card(s) [and put onto battlefield [tapped]]".
/// Seek grammar is simpler than search: no "your library", no "for", no shuffle.
pub(super) fn parse_seek_details(lower: &str) -> SeekDetails {
    let after_seek = tag::<_, _, VerboseError<&str>>("seek ")
        .parse(lower)
        .map(|(rest, _)| rest)
        .unwrap_or(lower);

    // Extract destination clause before filter parsing, so it doesn't pollute the filter.
    let (filter_text, destination, enter_tapped) = {
        let put_idx = after_seek
            .find(" and put")
            .or_else(|| after_seek.find(", put"));
        if let Some(idx) = put_idx {
            let dest_clause = &after_seek[idx..];
            let dest = parse_search_destination(dest_clause);
            let tapped = scan_contains_phrase(dest_clause, "battlefield tapped");
            (&after_seek[..idx], dest, tapped)
        } else {
            (after_seek, Zone::Hand, false)
        }
    };

    // Extract count: "two nonland cards" → (2, "nonland cards")
    // Delegate to nom combinator (input already lowercase).
    let (count, remaining) = if let Ok((rest, n)) = nom_primitives::parse_number.parse(filter_text)
    {
        (QuantityExpr::Fixed { value: n as i32 }, rest.trim_start())
    } else if let Ok((rest, _)) = tag::<_, _, VerboseError<&str>>("x ").parse(filter_text) {
        (
            QuantityExpr::Ref {
                qty: QuantityRef::Variable {
                    name: "X".to_string(),
                },
            },
            rest.trim_start(),
        )
    } else {
        (QuantityExpr::Fixed { value: 1 }, filter_text)
    };

    // Strip leading article "a "/"an "
    let remaining = nom_primitives::parse_article
        .parse(remaining)
        .map(|(rest, _)| rest)
        .unwrap_or(remaining);

    let filter = parse_search_filter(remaining);

    SeekDetails {
        filter,
        count,
        destination,
        enter_tapped,
    }
}

/// Parse the card type filter from search text like "basic land card, ..."
/// or "creature card with ..." into a TargetFilter.
pub(super) fn parse_search_filter(text: &str) -> TargetFilter {
    // Find the end of the type description (before comma, period, or "and put")
    let type_end = text
        .find(',')
        .or_else(|| text.find('.'))
        .or_else(|| text.find(" and put"))
        .or_else(|| text.find(" and shuffle"))
        .unwrap_or(text.len());
    let type_text = text[..type_end].trim();

    // Strip trailing "card" or "cards"
    let type_text = type_text
        .strip_suffix(" cards")
        .or_else(|| type_text.strip_suffix(" card"))
        .unwrap_or(type_text)
        .trim();

    // Intentional: "a card" means any card type — no warning needed.
    if type_text == "card" || type_text.is_empty() {
        return TargetFilter::Any;
    }

    let is_basic = scan_contains_phrase(type_text, "basic");
    let clean = type_text.replace("basic ", "");

    let (type_word, suffix_text) = {
        let lower = clean.to_lowercase();
        if let Some(pos) = lower.find(" with ") {
            let mut type_word = clean[..pos].trim();
            type_word = type_word
                .strip_suffix(" cards")
                .or_else(|| type_word.strip_suffix(" card"))
                .unwrap_or(type_word)
                .trim();
            (type_word.to_string(), &clean[pos..])
        } else {
            (clean.trim().to_string(), "")
        }
    };

    let (card_type, subtype): (Option<TypeFilter>, Option<String>) = match type_word.as_str() {
        "land" => (Some(TypeFilter::Land), None),
        "creature" => (Some(TypeFilter::Creature), None),
        "artifact" => (Some(TypeFilter::Artifact), None),
        "enchantment" => (Some(TypeFilter::Enchantment), None),
        "instant" => (Some(TypeFilter::Instant), None),
        "sorcery" => (Some(TypeFilter::Sorcery), None),
        "planeswalker" => (Some(TypeFilter::Planeswalker), None),
        "instant or sorcery" => {
            let mut properties = vec![];
            if is_basic {
                properties.push(FilterProp::HasSupertype {
                    value: crate::types::card_type::Supertype::Basic,
                });
            }
            parse_search_filter_suffixes(suffix_text, &mut properties);
            return TargetFilter::Or {
                filters: vec![
                    TargetFilter::Typed(
                        TypedFilter::new(TypeFilter::Instant).properties(properties.clone()),
                    ),
                    TargetFilter::Typed(
                        TypedFilter::new(TypeFilter::Sorcery).properties(properties),
                    ),
                ],
            };
        }
        other => {
            let negated_types: &[(&str, TypeFilter)] = &[
                ("noncreature", TypeFilter::Creature),
                ("nonland", TypeFilter::Land),
                ("nonartifact", TypeFilter::Artifact),
                ("nonenchantment", TypeFilter::Enchantment),
            ];
            for &(prefix, ref inner) in negated_types {
                if other == prefix {
                    let mut properties = vec![];
                    if is_basic {
                        properties.push(FilterProp::HasSupertype {
                            value: crate::types::card_type::Supertype::Basic,
                        });
                    }
                    parse_search_filter_suffixes(suffix_text, &mut properties);
                    return TargetFilter::Typed(
                        TypedFilter::new(TypeFilter::Non(Box::new(inner.clone())))
                            .properties(properties),
                    );
                }
            }

            let land_subtypes = ["plains", "island", "swamp", "mountain", "forest"];
            if land_subtypes.contains(&other) {
                let mut properties = vec![];
                if is_basic {
                    properties.push(FilterProp::HasSupertype {
                        value: crate::types::card_type::Supertype::Basic,
                    });
                }
                parse_search_filter_suffixes(suffix_text, &mut properties);
                return TargetFilter::Typed(
                    TypedFilter::land()
                        .subtype(capitalize(other))
                        .properties(properties),
                );
            }
            if other == "equipment" {
                let mut properties = vec![];
                parse_search_filter_suffixes(suffix_text, &mut properties);
                return TargetFilter::Typed(
                    TypedFilter::new(TypeFilter::Artifact)
                        .subtype("Equipment".to_string())
                        .properties(properties),
                );
            }
            if other == "aura" {
                let mut properties = vec![];
                parse_search_filter_suffixes(suffix_text, &mut properties);
                return TargetFilter::Typed(
                    TypedFilter::new(TypeFilter::Enchantment)
                        .subtype("Aura".to_string())
                        .properties(properties),
                );
            }
            if other == "card" && !suffix_text.is_empty() {
                let mut properties = vec![];
                parse_search_filter_suffixes(suffix_text, &mut properties);
                if !properties.is_empty() {
                    return TargetFilter::Typed(TypedFilter::default().properties(properties));
                }
            }
            if !other.is_empty()
                && other != "card"
                && other != "permanent"
                && other.chars().all(|c| c.is_alphabetic())
            {
                let mut properties = vec![];
                if is_basic {
                    properties.push(FilterProp::HasSupertype {
                        value: crate::types::card_type::Supertype::Basic,
                    });
                }
                parse_search_filter_suffixes(suffix_text, &mut properties);
                return TargetFilter::Typed(
                    TypedFilter::default()
                        .subtype(capitalize(other))
                        .properties(properties),
                );
            }
            let (filter, _) = parse_type_phrase(other);
            if !matches!(filter, TargetFilter::Any) {
                let mut properties = vec![];
                if is_basic {
                    properties.push(FilterProp::HasSupertype {
                        value: crate::types::card_type::Supertype::Basic,
                    });
                }
                parse_search_filter_suffixes(suffix_text, &mut properties);
                return if properties.is_empty() {
                    filter
                } else {
                    match filter {
                        TargetFilter::Typed(mut typed_filter) => {
                            typed_filter.properties.extend(properties);
                            TargetFilter::Typed(typed_filter)
                        }
                        _ => filter,
                    }
                };
            }
            push_warning(format!(
                "target-fallback: unrecognized search filter '{}'",
                other
            ));
            return TargetFilter::Any;
        }
    };

    let mut properties = vec![];
    if is_basic {
        properties.push(FilterProp::HasSupertype {
            value: crate::types::card_type::Supertype::Basic,
        });
    }
    parse_search_filter_suffixes(suffix_text, &mut properties);

    let mut typed_filter = TypedFilter::default();
    if let Some(card_type) = card_type {
        typed_filter = typed_filter.with_type(card_type);
    }
    if let Some(subtype) = subtype {
        typed_filter = typed_filter.subtype(subtype);
    }
    typed_filter.properties = properties;
    TargetFilter::Typed(typed_filter)
}

/// Parse property suffixes from search filter text ("with mana value ...", "with a different name ...").
/// Reuses the existing suffix parsers from oracle_target.
fn parse_search_filter_suffixes(text: &str, properties: &mut Vec<FilterProp>) {
    let lower = text.to_lowercase();
    let mut remaining = lower.as_str();

    while !remaining.is_empty() {
        remaining = remaining.trim_start();
        if let Ok((rest, _)) = tag::<_, _, VerboseError<&str>>("and ").parse(remaining) {
            remaining = rest.trim_start();
        }

        if let Ok((rest, _)) = tag::<_, _, VerboseError<&str>>("with that name").parse(remaining) {
            properties.push(FilterProp::SameName);
            remaining = rest.trim_start();
            continue;
        }

        if let Some((prop, consumed)) = parse_mana_value_suffix(remaining) {
            properties.push(prop);
            remaining = remaining[consumed..].trim_start();
            continue;
        }

        if let Ok((rest, _)) =
            tag::<_, _, VerboseError<&str>>("with a different name than each ").parse(remaining)
        {
            let end = rest
                .find(" you control")
                .unwrap_or_else(|| rest.find(',').unwrap_or(rest.len()));
            let inner_type = rest[..end].trim();
            let inner_filter = match inner_type {
                "aura" => TargetFilter::Typed(
                    TypedFilter::new(TypeFilter::Enchantment).subtype("Aura".to_string()),
                ),
                "creature" => TargetFilter::Typed(TypedFilter::creature()),
                "enchantment" => TargetFilter::Typed(TypedFilter::new(TypeFilter::Enchantment)),
                "artifact" => TargetFilter::Typed(TypedFilter::new(TypeFilter::Artifact)),
                _ => {
                    push_warning(format!(
                        "target-fallback: unrecognized inner type '{}' in different-name filter",
                        inner_type
                    ));
                    TargetFilter::Any
                }
            };
            properties.push(FilterProp::DifferentNameFrom {
                filter: Box::new(inner_filter),
            });
            let skip = rest
                .find(" you control")
                .map_or(end, |position| position + " you control".len());
            remaining = rest[skip..].trim_start();
            continue;
        }

        break;
    }
}

/// Parse the destination zone from search Oracle text.
/// Looks for "put it into your hand", "put it onto the battlefield", etc.
pub(super) fn parse_search_destination(lower: &str) -> Zone {
    if scan_contains_phrase(lower, "onto the battlefield") {
        Zone::Battlefield
    } else if contains_possessive(lower, "into", "hand") {
        Zone::Hand
    } else if contains_possessive(lower, "on top of", "library") {
        Zone::Library
    } else if contains_possessive(lower, "into", "graveyard") {
        Zone::Graveyard
    } else {
        Zone::Hand
    }
}
