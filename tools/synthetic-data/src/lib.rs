//! Synthetic Ugandan patients, facilities, stores, stock, prices and payer contracts for CI,
//! staging and load tests (testing.md § Data). Only synthetic data enters CI; every resource
//! carries the `urn:health:synthetic-data` tag.

pub mod load;
pub mod names;
pub mod output;
pub mod population;
pub mod rng;
pub mod world;
