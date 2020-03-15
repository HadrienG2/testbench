//! Inlining barriers for function calls
//!
//! Inlining is great for optimization. But it can cause problems in micro-
//! benchmarking and multi-threaded validation as it leads some testing and
//! benchmarking constructs to be optimized out. This module can be used to
//! avoid this outcome without altering the function being called itself.

/// Inlining barrier for FnOnce
///
/// # Panics
///
/// This function will propagate panics from the inner callable.
#[inline(never)]
pub fn call_once(callable: impl FnOnce()) {
    callable()
}

/// Inlining barrier for FnMut
///
/// # Panics
///
/// This function will propagate panics from the inner callable.
#[inline(never)]
pub fn call_mut(callable: &mut impl FnMut()) {
    callable()
}

/// Inlining barrier for Fn
///
/// # Panics
///
/// This function will propagate panics from the inner callable.
#[inline(never)]
pub fn call(callable: &impl Fn()) {
    callable()
}
