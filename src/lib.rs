//! Testing and benchmarking tools for concurrent Rust code
//!
//! This crate groups together a bunch of utilities which I've found useful when
//! testing and benchmarking Rust concurrency primitives in the triple_buffer
//! and spmc_buffer crates.
//!
//! If it proves popular, other testing and benchmarking tools may be added,
//! based on user demand.
//!
//! # Examples
//!
//! For examples of this crate at work, look at its "tests" and "benchs"
//! submodules, which showcase expected usage.

#![warn(
    anonymous_parameters,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    nonstandard_style,
    rust_2018_idioms,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unused_extern_crates,
    unused_qualifications,
    variant_size_differences
)]

pub mod noinline;
pub mod race_cell;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Barrier,
};

/// Test that running two operations concurrently works
///
/// When testing multi-threaded constructs, such as synchronization primitives,
/// checking that the code works when run sequentially is insufficient.
/// Concurrent interactions must also be tested, by running some operations
/// in parallel across multiple threads.
///
/// Ideally, this function would take as input a variable-size set of functors
/// to be run in parallel. Since Rust does not support variadic generics yet,
/// however, multiple versions of this function must be provided, each
/// associated with a different functor tuple size.
///
/// # Panics
///
/// This function will propagate panics from the inner functors.
///
pub fn concurrent_test_2(f1: impl FnOnce() + Send, f2: impl FnOnce() + Send) {
    let barrier = Barrier::new(2);
    std::thread::scope(|s| {
        s.spawn(|| {
            barrier.wait();
            noinline::call_once(f1);
        });
        barrier.wait();
        noinline::call_once(f2);
    })
}

/// Test that running three operations concurrently works
///
/// This is a variant of concurrent_test_2 that works with three functors
/// instead of two. It is hoped that future evolutions of Rust will render this
/// (light) code duplication obsolete, in favor of some variadic design.
///
/// # Panics
///
/// This function will propagate panics from the inner functors.
///
pub fn concurrent_test_3(
    f1: impl FnOnce() + Send,
    f2: impl FnOnce() + Send,
    f3: impl FnOnce() + Send,
) {
    let barrier = Barrier::new(3);
    std::thread::scope(|s| {
        s.spawn(|| {
            barrier.wait();
            noinline::call_once(f1);
        });
        s.spawn(|| {
            barrier.wait();
            noinline::call_once(f2);
        });
        barrier.wait();
        noinline::call_once(f3);
    })
}

/// Perform some operation while another is running in a loop in another thread
///
/// For multithreaded code, benchmarking the performance of isolated operations
/// is usually only half of the story. Synchronization and memory contention can
/// also have a large impact on performance.
///
/// For this reason, it is often useful to also measure the performance of one
/// operation as another "antagonist" operation is also running in a background
/// thread. This function helps you with setting up such an antagonist thread.
///
/// Note that the antagonist function must be designed in such a way as not to
/// be optimized out by the compiler when run in a tight loop. Here are some
/// ways to do this:
///
/// - You can hide the fact that the code is run in a loop by preventing the
///   compiler from inlining it there, see this crate's `noinline::call_mut()`.
/// - You can obscure the fact that inputs are always the same and outputs are
///   are not used by using `core::hint::black_box()` on nightly Rust, or its
///   emulation by the Criterion benchmarking crate.
/// - You can generate inputs that the compiler cannot guess using a random
///   number generator, and use your outputs by sending them through some sort
///   of reduction function (sum, min, max...) and checking the result.
///
pub fn run_under_contention<AntagonistResult, BenchmarkResult>(
    mut antagonist: impl FnMut() -> AntagonistResult + Send,
    mut benchmark: impl FnMut() -> BenchmarkResult,
) -> BenchmarkResult {
    let start_barrier = Barrier::new(2);
    let continue_flag = AtomicBool::new(true);
    std::thread::scope(|s| {
        s.spawn(|| {
            start_barrier.wait();
            while continue_flag.load(Ordering::Relaxed) {
                antagonist();
            }
        });
        start_barrier.wait();
        let result = benchmark();
        continue_flag.store(false, Ordering::Relaxed);
        result
    })
}

/// Examples of concurrent testing code
#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };

    // Check the behaviour of concurrent atomic swaps and fetch-adds
    #[test]
    fn swap_and_fetch_add() {
        // Amount of atomic operations to check
        const ATOMIC_OPS_COUNT: usize = 100_000_000;

        // Create a shared atomic variable
        let atom = AtomicUsize::new(0);

        // Check that concurrent atomic operations work correctly
        let mut last_value = 0;
        super::concurrent_test_2(
            || {
                // One thread continuously increments the atomic variable...
                for _ in 1..=ATOMIC_OPS_COUNT {
                    let former_atom = atom.fetch_add(1, Ordering::Relaxed);
                    assert!((former_atom == 0) || (former_atom == last_value));
                    last_value = former_atom + 1;
                }
            },
            || {
                // ...as another continuously resets it to zero
                for _ in 1..=ATOMIC_OPS_COUNT {
                    let former_atom = atom.swap(0, Ordering::Relaxed);
                    assert!(former_atom <= ATOMIC_OPS_COUNT);
                }
            },
        );
    }

    // Check the behaviour of concurrent fetch-and/or/xors
    #[test]
    fn fetch_and_or_xor() {
        // Amount of atomic operations to check
        const ATOMIC_OPS_COUNT: usize = 30_000_000;

        // Create a shared atomic variable. Even though this is an atomic Usize,
        // we will only use the 16 low-order bits for maximal portability.
        let atom = AtomicUsize::new(0);

        // Masks used by each atomic operation
        const AND_MASK: usize = 0b0000_0000_0000_0000; // Clear all bits
        const XOR_MASK: usize = 0b0000_1111_0000_1111; // Flip some bits
        const OR_MASK: usize = 0b1111_0000_1111_0000; // Set other bits

        // Check that concurrent atomic operations work correctly by ensuring
        // that at any point in time, only the 16 low-order bits can be set, and
        // the grouped sets of bits in the masks above are either all set or
        // all cleared in any observable state.
        super::concurrent_test_3(
            || {
                // One thread runs fetch-ands in a loop...
                for _ in 1..=ATOMIC_OPS_COUNT {
                    let old_val = atom.fetch_and(AND_MASK, Ordering::Relaxed);
                    assert_eq!(old_val & 0b1111_1111_1111_1111, old_val);
                    assert!((old_val & XOR_MASK == XOR_MASK) || (old_val & XOR_MASK == 0));
                    assert!((old_val & OR_MASK == OR_MASK) || (old_val & OR_MASK == 0));
                }
            },
            || {
                // ...another runs fetch-ors in a loop...
                for _ in 1..=ATOMIC_OPS_COUNT {
                    let old_val = atom.fetch_or(OR_MASK, Ordering::Relaxed);
                    assert_eq!(old_val & 0b1111_1111_1111_1111, old_val);
                    assert!((old_val & XOR_MASK == XOR_MASK) || (old_val & XOR_MASK == 0));
                    assert!((old_val & OR_MASK == OR_MASK) || (old_val & OR_MASK == 0));
                }
            },
            || {
                // ...and the last one runs fetch-xors in a loop...
                for _ in 1..=ATOMIC_OPS_COUNT {
                    let old_val = atom.fetch_xor(XOR_MASK, Ordering::Relaxed);
                    assert_eq!(old_val & 0b1111_1111_1111_1111, old_val);
                    assert!((old_val & XOR_MASK == XOR_MASK) || (old_val & XOR_MASK == 0));
                    assert!((old_val & OR_MASK == OR_MASK) || (old_val & OR_MASK == 0));
                }
            },
        );
    }

    // Show how adversarial code is actually run in concurrent "benchmarking"
    #[test]
    fn antagonist_showcase() {
        let atom = AtomicUsize::new(0);
        super::run_under_contention(
            || atom.fetch_add(1, Ordering::Relaxed),
            || std::thread::sleep(Duration::from_millis(1000)),
        );
        assert!(atom.load(Ordering::Relaxed) > 100000);
    }
}
