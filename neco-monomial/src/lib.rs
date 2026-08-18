#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

extern crate alloc;
#[cfg(test)]
extern crate std;

mod basis;
mod error;
mod monomial;
mod prime;
mod raw;

pub use basis::RadicalBasis;
pub use error::{MonomialErrorKind, NormalizationErrors};
pub use monomial::Monomial;
pub use prime::ProvenPrime;
pub use raw::{RawMonomial, RawPower};
