//! Thread-local parse warning accumulator.
//!
//! Collects diagnostic warnings during Oracle text parsing — silent fallbacks,
//! ignored remainders, bare filters — without changing any parse results.
//! Warnings are harvested after each card's parse and stored on `CardFace`.

use std::cell::RefCell;

thread_local! {
    static WARNINGS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Push a diagnostic warning for the card currently being parsed.
pub fn push_warning(msg: impl Into<String>) {
    WARNINGS.with(|w| w.borrow_mut().push(msg.into()));
}

/// Drain all accumulated warnings (returns them and clears the buffer).
pub fn take_warnings() -> Vec<String> {
    WARNINGS.with(|w| w.borrow_mut().drain(..).collect())
}

/// Discard any accumulated warnings (called at the start of each card parse).
pub fn clear_warnings() {
    WARNINGS.with(|w| w.borrow_mut().clear());
}
