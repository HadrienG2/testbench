use std::sync::{Arc, Barrier};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

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


/// TODO: Just an usage example, should probably be improved
#[cfg(test)]
mod benchs {
    use std::time::Instant;

    #[test]
    #[ignore]
    fn it_works() {
        let initial = Instant::now();
        ::benchmark(50000000, || { assert!( Instant::now() > initial ) });
    }
}
