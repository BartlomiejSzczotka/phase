//! Parse context for pronoun and reference resolution threading.

use crate::types::ability::{QuantityRef, TargetFilter};

/// Parsing context threaded through nom combinators for pronoun/reference resolution.
///
/// Registers resolved references so subsequent combinators can resolve
/// "it", "that creature", "that many", etc. The existing `ParseContext` in
/// `oracle_effect/mod.rs` serves the strip_prefix parser; this struct mirrors
/// its semantics for the nom combinator pipeline.
#[derive(Debug, Clone, Default)]
pub struct ParseContext {
    /// The current subject (resolved target -- "it", "that creature").
    pub subject: Option<TargetFilter>,
    /// Resolved quantity reference ("that many", "that much").
    pub quantity_ref: Option<QuantityRef>,
    /// Card name for self-reference (~) normalization.
    pub card_name: Option<String>,
    /// Whether we are inside a trigger effect (enables event context refs).
    pub in_trigger: bool,
    /// Whether we are inside a replacement effect.
    pub in_replacement: bool,
}
