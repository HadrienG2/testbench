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

#![deny(missing_docs)]

use std::sync::{Arc, Barrier};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};


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
pub fn concurrent_test_2<F, G>(f1: F, f2: G)
    where F: FnOnce() + Send + 'static,
          G: FnOnce() + Send + 'static
{
    // Setup a barrier to synchronize thread startup
    let barrier1 = Arc::new(Barrier::new(2));
    let barrier2 = barrier1.clone();

    // Start the first task
    let thread1 = thread::spawn(move || {
        barrier1.wait();
        f1();
    });

    // Run the second task
    barrier2.wait();
    f2();

    // Make sure that the first task completed properly
    thread1.join().unwrap();
}


/// Test that running three operations concurrently works
///
/// This is a variant of concurrent_test_2 that works with three functors
/// instead of two. It is hoped that future evolutions of Rust will render this
/// (light) code duplication obsolete, in favor of some variadic design.
///
pub fn concurrent_test_3<F, G, H>(f1: F, f2: G, f3: H)
    where F: FnOnce() + Send + 'static,
          G: FnOnce() + Send + 'static,
          H: FnOnce() + Send + 'static
{
    // Setup a barrier to synchronize thread startup
    let barrier1 = Arc::new(Barrier::new(3));
    let barrier2 = barrier1.clone();
    let barrier3 = barrier1.clone();

    // Start the first task
    let thread1 = thread::spawn(move || {
        barrier1.wait();
        f1();
    });

    // Start the second task
    let thread2 = thread::spawn(move || {
        barrier2.wait();
        f2();
    });

    // Run the third task
    barrier3.wait();
    f3();

    // Make sure that the first two tasks completed properly
    thread1.join().unwrap();
    thread2.join().unwrap();
}


/// Microbenchmark some simple operation by running it N times
///
/// This simple benchmark harness is meant as a cheap and hackish substitute for
/// cargo benchmarks in Stable Rust. It runs a user-provided operation a certain
/// number of times and measures how much time it takes.
///
/// To use it, write your benchmark as an ignored cargo test, put a call to
/// benchmark() or counting_benchmark() as the last operation, and tell your
/// user to run the benchmarks via:
///
///   $ cargo test --release -- --ignored --nocapture --test-threads=1
///
/// This is a dreadful hack. But for now, it's the best that I've thought of.
///
pub fn benchmark<F: FnMut()>(num_iterations: u32, mut iteration: F) {
    // Run the user-provided operation in a loop
    let start_time = Instant::now();
    for _ in 0..num_iterations {
        iteration()
    }
    let total_duration = start_time.elapsed();

    // Reproducible benchmarks (<10% variance) usually take between a couple of
    // seconds and a couple of minutes, so miliseconds are the right timing unit
    // for the duration of the whole benchmark.
    let total_ms = (total_duration.as_secs() as u32) * 1000 +
                   total_duration.subsec_nanos() / 1_000_000;

    // This tool is designed for microbenchmarking, so iterations are assumed
    // to last from one CPU cycle (a fraction of a nanosecond) to a fraction of
    // a second. Longer durations will require multi-resolution formatting.
    let hundred_iter_duration = total_duration * 100 / num_iterations;
    let iter_duration = hundred_iter_duration / 100;
    assert_eq!(iter_duration.as_secs(), 0);
    let iter_ns = iter_duration.subsec_nanos();
    let iter_ns_fraction = hundred_iter_duration.subsec_nanos() % 100;

    // Display the benchmark results, in a fashion that will fit in the output
    // of cargo test in nocapture mode.
    print!("{} ms ({} iters, ~{}.{} ns/iter): ",
           total_ms,
           num_iterations,
           iter_ns,
           iter_ns_fraction);
}


/// Microbenchmark some operation while another is running in a loop
///
/// For multithreaded code, benchmarking the performance of isolated operations
/// is usually only half of the story. Synchronization and memory contention can
/// also have a large impact on performance.
///
/// For this reason, it is often useful to also measure the performance of one
/// operation as another "antagonist" operation is also running in a background
/// thread. This function implements such concurrent benchmarking.
///
pub fn concurrent_benchmark<F, A>(num_iterations: u32,
                                  iteration_func: F,
                                  mut antagonist_func: A)
    where F: FnMut(),
          A: FnMut() + Send + 'static
{
    // Setup a barrier to synchronize benchmark and antagonist startup
    let barrier = Arc::new(Barrier::new(2));

    // Setup an atomic "continue" flag to shut down the antagonist at
    // the end of the benchmarking procedure
    let run_flag = Arc::new(AtomicBool::new(true));

    // Schedule the antagonist thread
    let (a_barrier, a_run_flag) = (barrier.clone(), run_flag.clone());
    let antagonist = thread::spawn(
        move || {
            a_barrier.wait();
            while a_run_flag.load(Ordering::Relaxed) {
                antagonist_func();
            }
        }
    );

    // Wait for the antagonist to be running, and give it some headstart
    barrier.wait();
    thread::sleep(Duration::from_millis(10));

    // Run the benchmark
    benchmark(num_iterations, iteration_func);

    // Stop the antagonist and check that nothing went wrong on its side
    run_flag.store(false, Ordering::Relaxed);
    antagonist.join().unwrap();
}


/// Examples of concurrent testing code
#[cfg(test)]
mod tests {
    use std::ops::BitAnd;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Check the behaviour of concurrent atomic swaps and fetch-adds
    #[test]
    fn swap_and_fetch_add() {
        // Amount of atomic operations to check
        const ATOMIC_OPS_COUNT: usize = 100_000_000;

        // Create a shared atomic variable
        let atom = Arc::new(AtomicUsize::new(0));
        let atom2 = atom.clone();

        // Check that concurrent atomic operations work correctly
        let mut last_value = 0;
        ::concurrent_test_2(
            move || {
                // One thread continuously increments the atomic variable...
                for _ in 1..(ATOMIC_OPS_COUNT + 1) {
                    let former_atom = atom.fetch_add(1, Ordering::Relaxed);
                    assert!((former_atom == 0) || (former_atom == last_value));
                    last_value = former_atom+1;
                }
            },
            move || {
                // ...as another continuously resets it to zero
                for _ in 1..(ATOMIC_OPS_COUNT + 1) {
                    let former_atom = atom2.swap(0, Ordering::Relaxed);
                    assert!(former_atom <= ATOMIC_OPS_COUNT);
                }
            }
        );
    }

    // Check the behaviour of concurrent fetch-and/or/xors
    #[test]
    fn fetch_and_or_xor() {
        // Amount of atomic operations to check
        const ATOMIC_OPS_COUNT: usize = 30_000_000;

        // Create a shared atomic variable. Even though this is an atomic Usize,
        // we will only use the 16 low-order bits for maximal portability.
        let atom = Arc::new(AtomicUsize::new(0));
        let atom2 = atom.clone();
        let atom3 = atom.clone();

        // Masks used by each atomic operation
        const AND_MASK: usize = 0b0000_0000_0000_0000; // Clear all bits
        const XOR_MASK: usize = 0b0000_1111_0000_1111; // Flip some bits
        const OR_MASK: usize  = 0b1111_0000_1111_0000; // Set other bits

        // Check that concurrent atomic operations work correctly by ensuring
        // that at any point in time, only the 16 low-order bits can be set, and
        // the grouped sets of bits in the masks above are either all set or
        // all cleared in any observable state.
        ::concurrent_test_3(
            move || {
                // One thread runs fetch-ands in a loop...
                for _ in 1..(ATOMIC_OPS_COUNT + 1) {
                    let old_val = atom.fetch_and(AND_MASK, Ordering::Relaxed);
                    assert_eq!(old_val.bitand(0b1111_1111_1111_1111), old_val);
                    assert!((old_val.bitand(XOR_MASK) == XOR_MASK) ||
                            (old_val.bitand(XOR_MASK) == 0));
                    assert!((old_val.bitand(OR_MASK) == OR_MASK) ||
                            (old_val.bitand(OR_MASK) == 0));
                }
            },
            move || {
                // ...another runs fetch-ors in a loop...
                for _ in 1..(ATOMIC_OPS_COUNT + 1) {
                    let old_val = atom2.fetch_or(OR_MASK, Ordering::Relaxed);
                    assert_eq!(old_val.bitand(0b1111_1111_1111_1111), old_val);
                    assert!((old_val.bitand(XOR_MASK) == XOR_MASK) ||
                            (old_val.bitand(XOR_MASK) == 0));
                    assert!((old_val.bitand(OR_MASK) == OR_MASK) ||
                            (old_val.bitand(OR_MASK) == 0));
                }
            },
            move || {
                // ...and the last one runs fetch-xors in a loop...
                for _ in 1..(ATOMIC_OPS_COUNT + 1) {
                    let old_val = atom3.fetch_xor(XOR_MASK, Ordering::Relaxed);
                    assert_eq!(old_val.bitand(0b1111_1111_1111_1111), old_val);
                    assert!((old_val.bitand(XOR_MASK) == XOR_MASK) ||
                            (old_val.bitand(XOR_MASK) == 0));
                    assert!((old_val.bitand(OR_MASK) == OR_MASK) ||
                            (old_val.bitand(OR_MASK) == 0));
                }
            }
        );
    }
}


/// Exemples of benchmarking code
///
/// As discussed before, these should be run via the following command:
///
///   $ cargo test --release -- --ignored --nocapture --test-threads=1
#[cfg(test)]
mod benchs {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Benchmark relaxed atomics in sequential code (best case)
    #[test]
    #[ignore]
    fn bench_relaxed() {
        let atom = AtomicUsize::new(0);
        ::benchmark(500_000_000, || { atom.fetch_add(1, Ordering::Relaxed); });
        assert_eq!(atom.load(Ordering::Relaxed), 500_000_000);
    }

    // Benchmark sequentially consistent atomics in concurrent code (worst case)
    #[test]
    #[ignore]
    fn bench_seqcst() {
        let atom = Arc::new(AtomicUsize::new(0));
        let atom2 = atom.clone();
        ::concurrent_benchmark(
            100_000_000,
            move || { atom.fetch_add(1, Ordering::SeqCst); },
            move || { atom2.fetch_add(1, Ordering::SeqCst); }
        );
    }
}
