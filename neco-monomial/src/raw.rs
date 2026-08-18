use alloc::vec::Vec;
use core::fmt;

use neco_bigint::{BigUint, RawRational, Sign};

use crate::error::{
    reserve_elements_for, AllocationTarget, MonomialErrorKind, NormalizationErrors,
};
use crate::monomial::Monomial;

#[derive(Debug, Eq, PartialEq)]
pub struct RawPower {
    base: BigUint,
    exponent: RawRational,
}

impl RawPower {
    pub fn new(base: BigUint, exponent: RawRational) -> Self {
        Self { base, exponent }
    }

    pub fn base(&self) -> &BigUint {
        &self.base
    }

    pub(crate) fn base_internal(&self) -> &BigUint {
        &self.base
    }

    pub fn exponent(&self) -> &RawRational {
        &self.exponent
    }

    pub(crate) fn exponent_internal(&self) -> &RawRational {
        &self.exponent
    }

    pub fn try_clone(&self) -> Result<Self, MonomialErrorKind> {
        self.try_clone_internal()
    }

    pub(crate) fn try_clone_internal(&self) -> Result<Self, MonomialErrorKind> {
        Ok(Self {
            base: self.base.try_clone()?,
            exponent: self.exponent.try_clone()?,
        })
    }
}

pub struct RawMonomial {
    zero: bool,
    sign: Sign,
    powers: Vec<RawPower>,
}

impl fmt::Debug for RawMonomial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RawMonomial")
            .field("zero", &self.zero)
            .field("sign", &self.sign)
            .field("power_count", &self.powers.len())
            .finish()
    }
}

impl PartialEq for RawMonomial {
    fn eq(&self, other: &Self) -> bool {
        self.zero == other.zero
            && self.sign == other.sign
            && self.powers.len() == other.powers.len()
            && self
                .powers
                .iter()
                .zip(&other.powers)
                .all(|(left, right)| left.base == right.base && left.exponent == right.exponent)
    }
}

impl Eq for RawMonomial {}

impl RawMonomial {
    pub fn zero() -> Self {
        Self::zero_internal()
    }

    pub(crate) fn zero_internal() -> Self {
        Self {
            zero: true,
            sign: Sign::Zero,
            powers: Vec::new(),
        }
    }

    pub fn positive(powers: Vec<RawPower>) -> Self {
        Self {
            zero: false,
            sign: Sign::Positive,
            powers,
        }
    }

    pub fn negative(powers: Vec<RawPower>) -> Self {
        Self {
            zero: false,
            sign: Sign::Negative,
            powers,
        }
    }

    pub fn is_zero(&self) -> bool {
        self.zero
    }

    pub(crate) fn is_zero_internal(&self) -> bool {
        self.zero
    }

    pub fn sign(&self) -> Sign {
        self.sign
    }

    pub(crate) fn sign_internal(&self) -> Sign {
        self.sign
    }

    pub fn powers(&self) -> &[RawPower] {
        &self.powers
    }

    pub(crate) fn powers_internal(&self) -> &[RawPower] {
        &self.powers
    }

    pub fn try_clone(&self) -> Result<Self, MonomialErrorKind> {
        self.try_clone_internal()
    }

    pub(crate) fn try_clone_internal(&self) -> Result<Self, MonomialErrorKind> {
        if self.zero {
            return Ok(Self::zero_internal());
        }
        let total = self.powers.len();
        let mut powers = Vec::new();
        reserve_elements_for(&mut powers, total, AllocationTarget::RawPowerClone)?;
        for power in &self.powers {
            powers.push(power.try_clone_internal()?);
        }
        Ok(Self {
            zero: false,
            sign: self.sign,
            powers,
        })
    }

    pub fn normalize(&self) -> Result<Monomial, NormalizationErrors<MonomialErrorKind>> {
        crate::monomial::normalize_raw(self)
    }
}
