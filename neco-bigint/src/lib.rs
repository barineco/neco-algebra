#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

extern crate alloc;

mod dyadic;
mod error;
mod integer;
mod natural;
mod rational;

pub use dyadic::{Dyadic, DyadicEnclosure};
pub use error::BigintError;
pub use integer::{BigInt, ExtendedGcd, Sign};
pub use natural::BigUint;
pub use rational::{RationalReduction, RawRational, ReducedRational};
