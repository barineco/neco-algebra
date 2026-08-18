#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

extern crate alloc;
#[cfg(test)]
extern crate std;

mod algebraic;
mod error;
mod factor;
mod polynomial;
mod resultant;
mod sturm;

pub use algebraic::{
    CertifiedAlgebraic, GeneratorRepresentative, IsolatingInterval, MinimalPolynomial,
    PolynomialQuotient, RationalCoefficientConversion, RealAlgebraic, RootIndex,
};
pub use error::{AlgnumError, AllocationResource, RepresentationResource};
pub use factor::IrreduciblePolynomial;
pub use polynomial::{CandidatePolynomial, Polynomial, RationalPolynomial, SquareFreePolynomial};
