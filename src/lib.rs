use std::time::Instant;

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
/// This is most certainly ugly. But for now, it's the best that I thought of.
///
pub fn benchmark<F: FnMut()>(num_iterations: u32, mut iteration: F) {
    // Run the user-provided operation in a loop
    let start_time = Instant::now();
    for iter in 0..num_iterations {
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


/// TODO: Concurrent benchmark


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
