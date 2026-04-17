//! Regression tests for CR (Comprehensive Rules) annotations in engine source.
//!
//! CLAUDE.md requires every rules-related code site to cite the correct CR
//! number. These tests lock in specific historically-incorrect sites so
//! future edits cannot silently reintroduce the wrong rule number.
//!
//! The target files are read via `include_str!` so these tests do not depend
//! on runtime filesystem state.

#[test]
fn cr_annotation_for_saddle_keyword_is_702_171a() {
    let src = include_str!("../../src/types/keywords.rs");
    assert!(
        !src.contains("CR 702.173: Saddle"),
        "types/keywords.rs must not annotate Saddle with CR 702.173 (Freerunning)"
    );
    assert!(
        !src.contains("CR 702.173\n"),
        "types/keywords.rs must not annotate Saddle parse arm with bare CR 702.173"
    );
    assert!(
        !src.contains("/ CR 702.173 /"),
        "types/keywords.rs must not include CR 702.173 in composite annotations"
    );
    assert!(
        src.contains("CR 702.171a: Saddle"),
        "types/keywords.rs must annotate Saddle N with CR 702.171a"
    );
    assert!(
        src.contains("CR 702.171a\n"),
        "types/keywords.rs Saddle parse arm must cite CR 702.171a"
    );
}

#[test]
fn cr_annotation_for_saddled_trigger_mode_is_702_171b() {
    let src = include_str!("../../src/types/triggers.rs");
    assert!(
        !src.contains("CR 702.174: Triggers when a creature becomes saddled"),
        "types/triggers.rs Saddled must use CR 702.171b, not CR 702.174 (Gift)"
    );
    assert!(
        src.contains("CR 702.171b: Triggers when a creature becomes saddled"),
        "types/triggers.rs Saddled trigger mode must cite CR 702.171b"
    );
}

#[test]
fn cr_annotation_for_station_tap_cost_is_701_26a() {
    let src = include_str!("../../src/game/engine.rs");
    assert!(
        !src.contains("CR 701.21a"),
        "game/engine.rs must not annotate any Tap with CR 701.21a (Sacrifice) — CR 701.26a is Tap"
    );
    assert!(
        src.contains("CR 701.26a: Tap the creature as cost payment"),
        "game/engine.rs Station announcement must annotate the tap with CR 701.26a"
    );
}

#[test]
fn cr_annotation_for_oracle_keyword_saddle_parser_is_702_171a() {
    let src = include_str!("../../src/parser/oracle_keyword.rs");
    assert!(
        !src.contains("CR 702.173: Saddle"),
        "parser/oracle_keyword.rs must not annotate Saddle with CR 702.173 (Freerunning)"
    );
    assert!(
        src.contains("CR 702.171a: Saddle"),
        "parser/oracle_keyword.rs must annotate Saddle N with CR 702.171a"
    );
}
