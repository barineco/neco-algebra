use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;

use neco_bigint::ReducedRational;
use neco_bigint::Sign;

use crate::error::{reserve_elements_for, AllocationTarget, MonomialErrorKind};
use crate::prime::ProvenPrime;

pub struct RadicalBasis {
    factors: Vec<(ProvenPrime, ReducedRational)>,
}

impl fmt::Debug for RadicalBasis {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RadicalBasis")
            .field("factor_count", &self.factors.len())
            .finish()
    }
}

impl PartialEq for RadicalBasis {
    fn eq(&self, other: &Self) -> bool {
        compare_basis(self, other) == Ordering::Equal
    }
}

impl Eq for RadicalBasis {}

impl Ord for RadicalBasis {
    fn cmp(&self, other: &Self) -> Ordering {
        compare_basis(self, other)
    }
}

impl PartialOrd for RadicalBasis {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn compare_basis(left: &RadicalBasis, right: &RadicalBasis) -> Ordering {
    for ((left_prime, left_exponent), (right_prime, right_exponent)) in
        left.factors.iter().zip(&right.factors)
    {
        let ordering = left_prime
            .value_internal()
            .cmp(right_prime.value_internal())
            .then_with(|| left_exponent.cmp(right_exponent));
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.factors.len().cmp(&right.factors.len())
}

impl RadicalBasis {
    pub fn one() -> Self {
        Self::one_internal()
    }

    pub(crate) fn one_internal() -> Self {
        Self {
            factors: Vec::new(),
        }
    }

    pub fn factors(&self) -> &[(ProvenPrime, ReducedRational)] {
        &self.factors
    }

    pub fn try_clone(&self) -> Result<Self, MonomialErrorKind> {
        let mut factors = Vec::new();
        reserve_elements_for(
            &mut factors,
            self.factors.len(),
            AllocationTarget::RadicalBasis,
        )?;
        for (prime, exponent) in &self.factors {
            factors.push((prime.try_clone_internal()?, exponent.try_clone()?));
        }
        Ok(Self { factors })
    }

    pub fn try_from_sorted_factors(
        factors: Vec<(ProvenPrime, ReducedRational)>,
    ) -> Result<Self, MonomialErrorKind> {
        let valid_exponents = factors.iter().all(|(_, exponent)| {
            exponent.numerator().sign() == Sign::Positive
                && exponent.numerator().magnitude() < exponent.denominator()
        });
        let sorted_primes = factors
            .windows(2)
            .all(|pair| pair[0].0.value_internal() < pair[1].0.value_internal());
        if !valid_exponents || !sorted_primes {
            return Err(MonomialErrorKind::InvalidRadicalBasis);
        }
        Ok(Self { factors })
    }
}
