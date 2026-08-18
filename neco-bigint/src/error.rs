use core::fmt;

use crate::BigUint;

#[derive(Debug, Eq, PartialEq)]
pub enum BigintError {
    CapacityOverflow,
    AllocationFailure { requested_limbs: usize },
    UnsignedUnderflow,
    DivisionByZero,
    NonExactDivision,
    ZeroDenominator,
    NonFiniteFloat,
    FloatOutOfRange,
    InvalidInterval,
    ExponentOverflow { required: BigUint, maximum: u32 },
}

impl fmt::Display for BigintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CapacityOverflow => "required limb capacity exceeds usize",
            Self::AllocationFailure { .. } => "limb allocation failed",
            Self::UnsignedUnderflow => "unsigned subtraction underflow",
            Self::DivisionByZero => "division by zero",
            Self::NonExactDivision => "division has a nonzero remainder",
            Self::ZeroDenominator => "rational denominator is zero",
            Self::NonFiniteFloat => "floating-point value is not finite",
            Self::FloatOutOfRange => "exact value is outside finite f64 range",
            Self::InvalidInterval => "dyadic enclosure endpoints are reversed",
            Self::ExponentOverflow { .. } => "dyadic exponent exceeds u32",
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BigintError {}
