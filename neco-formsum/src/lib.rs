#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

extern crate alloc;
#[cfg(test)]
extern crate std;

mod annihilating;
mod enclosure;
mod error;
mod extension;
mod formsum;
mod raw;

pub use annihilating::AnnihilatingCoefficients;
pub use error::{DimensionResource, FormSumErrorKind};
pub use extension::{RadicalCoordinates, RadicalExtension};
pub use formsum::FormSum;
pub use neco_monomial::NormalizationErrors;
pub use raw::{RawFormSum, RawTerm};
