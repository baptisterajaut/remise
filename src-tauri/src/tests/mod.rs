//! All tests live here, away from the code they check.
//!   cargo test                          unit tests (pure, fast)
//!   cargo test -- --ignored smoke       every tool against the real binaries

mod bins;
mod helpers;
mod runner;
mod smoke;
mod tools;
