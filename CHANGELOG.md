# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [Unreleased]

_No unreleased changes in the pipeline at the moment._


## [0.8] - 2021-01-16

### Added

- Extracted "run under contention" part of contended_benchmark

### Changed

- Concurrent testing and benchmarking tools now used scoped threads, eliminating
  the need for Arc'ing shared data.
- Since criterion has become good enough these days, this crate does not provide
  benchmarking tools anymore aside from the aforementioned one.


## [0.7.3] - 2020-03-15

### Added

- Add a changelog to the repository.

### Fixed

- Improve conformance to the Rust API guidelines.
- Minor doc tweaks.


## [0.7.2] - 2020-02-07

### Changed

- Cleanup and deduplicate GitHub Actions configuration.


## [0.7.1] - 2020-01-29

### Changed

- Move continuous integration to GitHub Actions.

### Fixed

- Minor code formatting and doc comment cleanup.


## [0.7.0] - 2019-04-12

### Added

- RaceCell now supports all the fixed-size atomic types introduced by Rust 1.34.

### Changed

- Bump travis CI configuration to Ubuntu Xenial.
- Bump minimal supported Rust version to 1.34.0.

### Fixed

- Documentation cleanup: remove reference to obsolete feature, don't use doc
  comments on non-documentable macro expansions...
- Minimal supported Rust version is now advertised in README.


## [0.6.0] - 2018-12-18

### Added

- Introduce Clippy checks in continuous integration.

### Changed

- Finish migration to Rust 2018.
- Bump minimal supported Rust version to 1.31.0.
- Make RaceCell easier to use by hiding extra atomic traits from the generic
  parameter interface.

### Fixed

- Adopt the `rustfmt` coding style.


## [0.5.0] - 2018-08-27

### Added

- Inlining barriers are provided to reduce odds of benchmark over-optimization.

### Changed

- Microbenchmarking tools use the aforementioned inlining barriers.
- Start migrating to Rust 2018 features where it clarifies code.
- Bump minimal supported Rust version to 1.26.0.


## [0.4.1] - 2018-02-21

### Changed

- RaceCell now implements Default.


## [0.4.0] - 2018-02-11

### Changed

- Switch license to MPLv2, which is a better match to Rust's static linking
  philosophy than LGPL.


## [0.3.1] - 2017-06-25

### Changed

- RaceCell now implements Debug and Clone.


## [0.3.0] - 2017-06-21

### Added

- Introduce the RaceCell, a primitive to detect the possibility of data races in
  thread synchronization code.


## [0.2.0] - 2017-06-14

### Added

- Introduce Travis CI continuous integration.

### Fixed

- Use CI to clarify minimal supported Rust version (currently 1.12.0).


## [0.1.1] - 2017-04-10

### Fixed

- Address copy-paste mistakes in README.


## [0.1.0] - 2017-04-04

### Added

- Microbenchmarking hack for stable Rust, which satisfies different tradeoffs
  than the de facto standards of Criterion and unstable `#[bench]`.
- Sister API to microbenchmark how multi-threaded pressure affects the
  performance of a synchronization primitive.
- Testing tool to evaluate if a synchronization primitive keeps behaving
  correctly under multi-threaded pressure of one or two adversaries.
- Release to crates.io under LGPLv3 license.



[Unreleased]: https://github.com/HadrienG2/testbench/compare/v0.8.0...HEAD
[0.7.3]: https://github.com/HadrienG2/testbench/compare/v0.7.3...v0.8.0
[0.7.3]: https://github.com/HadrienG2/testbench/compare/v0.7.2...v0.7.3
[0.7.2]: https://github.com/HadrienG2/testbench/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/HadrienG2/testbench/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/HadrienG2/testbench/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/HadrienG2/testbench/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/HadrienG2/testbench/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/HadrienG2/testbench/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/HadrienG2/testbench/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/HadrienG2/testbench/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/HadrienG2/testbench/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/HadrienG2/testbench/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/HadrienG2/testbench/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/HadrienG2/testbench/releases/tag/v0.1.0