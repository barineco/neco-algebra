use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;

use neco_bigint::{BigInt, BigUint, BigintError, RawRational, ReducedRational, Sign};

use crate::basis::RadicalBasis;
use crate::error::{
    compare_monomial_errors, reserve_elements_for, AllocationTarget, MonomialErrorKind,
    NormalizationErrors,
};
use crate::prime::{factor_biguint, pow_biguint_with, ProvenPrime};
use crate::raw::{RawMonomial, RawPower};

pub struct Monomial {
    zero: bool,
    sign: Sign,
    factors: Vec<(ProvenPrime, ReducedRational)>,
}

impl fmt::Debug for Monomial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Monomial")
            .field("zero", &self.zero)
            .field("sign", &self.sign)
            .field("factor_count", &self.factors.len())
            .finish()
    }
}

impl PartialEq for Monomial {
    fn eq(&self, other: &Self) -> bool {
        self.zero == other.zero
            && self.sign == other.sign
            && factor_slices_equal(&self.factors, &other.factors)
    }
}

impl Eq for Monomial {}

fn factor_slices_equal(
    left: &[(ProvenPrime, ReducedRational)],
    right: &[(ProvenPrime, ReducedRational)],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(
            |((left_prime, left_exponent), (right_prime, right_exponent))| {
                left_prime.value_internal() == right_prime.value_internal()
                    && left_exponent == right_exponent
            },
        )
}

impl Monomial {
    pub fn zero() -> Self {
        Self::zero_internal()
    }

    fn zero_internal() -> Self {
        Self {
            zero: true,
            sign: Sign::Zero,
            factors: Vec::new(),
        }
    }

    pub fn one() -> Self {
        Self::one_internal()
    }

    fn one_internal() -> Self {
        Self::from_parts(Sign::Positive, Vec::new())
    }

    pub fn is_zero(&self) -> bool {
        self.zero
    }

    pub fn sign(&self) -> Sign {
        self.sign
    }

    pub fn factors(&self) -> &[(ProvenPrime, ReducedRational)] {
        &self.factors
    }

    pub fn try_clone(&self) -> Result<Self, MonomialErrorKind> {
        if self.zero {
            return Ok(Self::zero_internal());
        }
        Ok(Self::from_parts(self.sign, clone_factors(&self.factors)?))
    }

    pub fn mul(&self, rhs: &Self) -> Result<Self, MonomialErrorKind> {
        if self.zero || rhs.zero {
            return Ok(Self::zero_internal());
        }
        let factors = merge_factors(&self.factors, &rhs.factors, false)?;
        Ok(Self::from_parts(
            multiply_sign(self.sign, rhs.sign),
            factors,
        ))
    }

    pub fn div(&self, rhs: &Self) -> Result<Self, MonomialErrorKind> {
        if rhs.zero {
            return Err(MonomialErrorKind::DivisionByZero);
        }
        if self.zero {
            return Ok(Self::zero_internal());
        }
        let factors = merge_factors(&self.factors, &rhs.factors, true)?;
        Ok(Self::from_parts(
            multiply_sign(self.sign, rhs.sign),
            factors,
        ))
    }

    pub fn pow(&self, exponent: &ReducedRational) -> Result<Self, MonomialErrorKind> {
        let exponent_sign = exponent.numerator().sign();
        if self.zero {
            return match exponent_sign {
                Sign::Positive => Ok(Self::zero_internal()),
                Sign::Zero => Err(MonomialErrorKind::UndefinedZeroPower),
                Sign::Negative => Err(MonomialErrorKind::ZeroToNegativePower),
            };
        }
        if exponent_sign == Sign::Zero {
            return Ok(Self::one_internal());
        }
        if self.sign == Sign::Negative && !exponent.denominator().bit(0) {
            return Err(MonomialErrorKind::EvenRootOfNegative);
        }
        let sign = if self.sign == Sign::Negative && exponent.numerator().magnitude().bit(0) {
            Sign::Negative
        } else {
            Sign::Positive
        };
        let mut factors = Vec::new();
        reserve_elements_for(
            &mut factors,
            self.factors.len(),
            AllocationTarget::NormalFactor,
        )?;
        for (prime, current) in &self.factors {
            factors.push((prime.try_clone_internal()?, current.mul(exponent)?));
        }
        Ok(Self::from_parts(sign, factors))
    }

    pub fn split_radical(&self) -> Result<(ReducedRational, RadicalBasis), MonomialErrorKind> {
        if self.zero {
            return Ok((reduced_integer(0)?, RadicalBasis::one_internal()));
        }
        let mut basis_count = 0_usize;
        visit_radical_parts(&self.factors, |_, _, remainder| {
            if !remainder.is_zero() {
                basis_count = basis_count
                    .checked_add(1)
                    .ok_or(MonomialErrorKind::CapacityOverflow)?;
            }
            Ok(())
        })?;
        let mut coefficient = reduced_integer(if self.sign == Sign::Negative { -1 } else { 1 })?;
        let mut basis = Vec::new();
        reserve_elements_for(&mut basis, basis_count, AllocationTarget::RadicalBasis)?;
        visit_radical_parts(&self.factors, |prime, floor, remainder| {
            if !floor.is_zero() {
                let power =
                    pow_biguint_with(prime.value_internal(), floor.magnitude(), BigUint::mul)?;
                let contribution = if floor.sign() == Sign::Positive {
                    RawRational::new(
                        BigInt::from_sign_magnitude(Sign::Positive, power),
                        BigUint::one()?,
                    )
                } else {
                    RawRational::new(BigInt::one()?, power)
                }
                .reduce()?
                .into_reduced();
                coefficient = coefficient.mul(&contribution)?;
            }
            if !remainder.is_zero() {
                basis.push((prime.try_clone_internal()?, remainder));
            }
            Ok(())
        })?;
        Ok((coefficient, RadicalBasis::try_from_sorted_factors(basis)?))
    }

    fn from_parts(sign: Sign, factors: Vec<(ProvenPrime, ReducedRational)>) -> Self {
        Self {
            zero: false,
            sign,
            factors,
        }
    }
}

fn visit_radical_parts<F>(
    factors: &[(ProvenPrime, ReducedRational)],
    mut emit: F,
) -> Result<(), MonomialErrorKind>
where
    F: FnMut(&ProvenPrime, BigInt, ReducedRational) -> Result<(), MonomialErrorKind>,
{
    for (prime, exponent) in factors {
        let floor = exponent.floor()?;
        let floor_for_remainder = floor.try_clone()?;
        let remainder = exponent.sub(&ReducedRational::from_bigint(floor_for_remainder)?)?;
        emit(prime, floor, remainder)?;
    }
    Ok(())
}

pub(crate) fn normalize_raw(
    raw: &RawMonomial,
) -> Result<Monomial, NormalizationErrors<MonomialErrorKind>> {
    if raw.is_zero_internal() {
        return Ok(Monomial::zero_internal());
    }
    let invalid = semantic_errors(raw.powers_internal()).map_err(NormalizationErrors::from_one)?;
    if let Some(errors) = invalid {
        return Err(errors);
    }
    if raw.powers_internal().iter().any(|power| {
        power.base_internal().is_zero()
            && !power.exponent_internal().denominator().is_zero()
            && power.exponent_internal().numerator().sign() == Sign::Positive
    }) {
        return Ok(Monomial::zero_internal());
    }

    let mut reduced = Vec::new();
    let index_count = raw
        .powers_internal()
        .iter()
        .try_fold(0_usize, |count, power| {
            if power.base_internal().to_u32() == Some(1)
                || power.exponent_internal().numerator().is_zero()
            {
                Ok(count)
            } else {
                count
                    .checked_add(1)
                    .ok_or(MonomialErrorKind::CapacityOverflow)
            }
        })
        .map_err(NormalizationErrors::from_one)?;
    reserve_elements_for(
        &mut reduced,
        index_count,
        AllocationTarget::NormalizationIndex,
    )
    .map_err(NormalizationErrors::from_one)?;
    for rank in 0..index_count {
        let power = selected_power_at_rank(raw.powers_internal(), rank)
            .ok_or_else(|| NormalizationErrors::from_one(MonomialErrorKind::CapacityOverflow))?;
        let exponent = power
            .exponent_internal()
            .reduce()
            .map_err(|error| NormalizationErrors::from_one(error.into()))?
            .into_reduced();
        reduced.push((power, exponent));
    }

    let distinct = result_factor_count(&reduced).map_err(NormalizationErrors::from_one)?;
    let mut factors = Vec::new();
    reserve_elements_for(&mut factors, distinct, AllocationTarget::NormalFactor)
        .map_err(NormalizationErrors::from_one)?;
    for_each_combined_factor(&reduced, |prime, exponent| {
        factors.push((prime, exponent));
        Ok(())
    })
    .map_err(NormalizationErrors::from_one)?;
    Ok(Monomial::from_parts(raw.sign_internal(), factors))
}

fn selected_power_at_rank(powers: &[RawPower], rank: usize) -> Option<&RawPower> {
    powers.iter().enumerate().find_map(|(index, power)| {
        if power.base_internal().to_u32() == Some(1)
            || power.exponent_internal().numerator().is_zero()
        {
            return None;
        }
        let lower = powers
            .iter()
            .enumerate()
            .filter(|(other_index, other)| {
                other.base_internal().to_u32() != Some(1)
                    && !other.exponent_internal().numerator().is_zero()
                    && (compare_raw_power(other, power) == Ordering::Less
                        || (compare_raw_power(other, power) == Ordering::Equal
                            && *other_index < index))
            })
            .count();
        (lower == rank).then_some(power)
    })
}

fn semantic_errors(
    powers: &[RawPower],
) -> Result<Option<NormalizationErrors<MonomialErrorKind>>, MonomialErrorKind> {
    let mut zero_denominator = false;
    let mut undefined = false;
    let mut negative = false;
    for power in powers {
        if power.exponent_internal().denominator().is_zero() {
            zero_denominator = true;
        } else if power.base_internal().is_zero() {
            match power.exponent_internal().numerator().sign() {
                Sign::Zero => undefined = true,
                Sign::Negative => negative = true,
                Sign::Positive => {}
            }
        }
    }
    let count = usize::from(zero_denominator) + usize::from(negative) + usize::from(undefined);
    if count == 0 {
        return Ok(None);
    }
    if count == 1 {
        let error = if negative {
            MonomialErrorKind::ZeroToNegativePower
        } else if undefined {
            MonomialErrorKind::UndefinedZeroPower
        } else {
            MonomialErrorKind::Bigint(BigintError::ZeroDenominator)
        };
        return Ok(Some(NormalizationErrors::from_one(error)));
    }
    let mut errors = Vec::new();
    reserve_elements_for(&mut errors, count, AllocationTarget::ErrorSet)?;
    if negative {
        errors.push(MonomialErrorKind::ZeroToNegativePower);
    }
    if undefined {
        errors.push(MonomialErrorKind::UndefinedZeroPower);
    }
    if zero_denominator {
        errors.push(MonomialErrorKind::Bigint(BigintError::ZeroDenominator));
    }
    errors.sort_by(compare_monomial_errors);
    Ok(NormalizationErrors::from_errors(errors))
}

fn compare_raw_power(left: &RawPower, right: &RawPower) -> Ordering {
    left.base_internal()
        .cmp(right.base_internal())
        .then_with(|| {
            left.exponent_internal()
                .numerator()
                .cmp(right.exponent_internal().numerator())
        })
        .then_with(|| {
            left.exponent_internal()
                .denominator()
                .cmp(right.exponent_internal().denominator())
        })
}

fn result_factor_count(
    powers: &[(&RawPower, ReducedRational)],
) -> Result<usize, MonomialErrorKind> {
    let mut count = 0_usize;
    for_each_combined_factor(powers, |_, _| {
        count = count
            .checked_add(1)
            .ok_or(MonomialErrorKind::CapacityOverflow)?;
        Ok(())
    })?;
    Ok(count)
}

fn for_each_combined_factor<F>(
    powers: &[(&RawPower, ReducedRational)],
    mut emit: F,
) -> Result<(), MonomialErrorKind>
where
    F: FnMut(ProvenPrime, ReducedRational) -> Result<(), MonomialErrorKind>,
{
    let mut previous: Option<ProvenPrime> = None;
    loop {
        let mut next: Option<ProvenPrime> = None;
        for (power, _) in powers {
            factor_biguint(power.base_internal(), |candidate, _| {
                let follows_previous = previous
                    .as_ref()
                    .is_none_or(|previous| candidate.value_internal() > previous.value_internal());
                let precedes_next = next
                    .as_ref()
                    .is_none_or(|next| candidate.value_internal() < next.value_internal());
                if follows_previous && precedes_next {
                    next = Some(candidate);
                }
                Ok(())
            })?;
        }
        let Some(prime) = next else {
            break;
        };
        let mut combined = reduced_integer(0)?;
        for (candidate_power, candidate_exponent) in powers {
            factor_biguint(candidate_power.base_internal(), |candidate, valuation| {
                if candidate.value_internal() == prime.value_internal() {
                    combined = combined.add(&scale_exponent(candidate_exponent, &valuation)?)?;
                }
                Ok(())
            })?;
        }
        if !combined.is_zero() {
            emit(prime.try_clone_internal()?, combined)?;
        }
        previous = Some(prime);
    }
    Ok(())
}

fn scale_exponent(
    exponent: &ReducedRational,
    valuation: &BigUint,
) -> Result<ReducedRational, MonomialErrorKind> {
    let multiplier = BigInt::from_sign_magnitude(Sign::Positive, valuation.try_clone()?);
    RawRational::new(
        exponent.numerator().mul(&multiplier)?,
        exponent.denominator().try_clone()?,
    )
    .reduce()
    .map(|value| value.into_reduced())
    .map_err(Into::into)
}

fn clone_factors(
    source: &[(ProvenPrime, ReducedRational)],
) -> Result<Vec<(ProvenPrime, ReducedRational)>, MonomialErrorKind> {
    let mut result = Vec::new();
    reserve_elements_for(&mut result, source.len(), AllocationTarget::NormalFactor)?;
    for (prime, exponent) in source {
        result.push((prime.try_clone_internal()?, exponent.try_clone()?));
    }
    Ok(result)
}

fn merge_factors(
    left: &[(ProvenPrime, ReducedRational)],
    right: &[(ProvenPrime, ReducedRational)],
    subtract: bool,
) -> Result<Vec<(ProvenPrime, ReducedRational)>, MonomialErrorKind> {
    let mut count = 0_usize;
    visit_merged_factors(left, right, subtract, |_, _| {
        count = count
            .checked_add(1)
            .ok_or(MonomialErrorKind::CapacityOverflow)?;
        Ok(())
    })?;
    let mut result = Vec::new();
    reserve_elements_for(&mut result, count, AllocationTarget::MergeResult)?;
    visit_merged_factors(left, right, subtract, |prime, exponent| {
        result.push((prime.try_clone_internal()?, exponent));
        Ok(())
    })?;
    Ok(result)
}

fn visit_merged_factors<F>(
    left: &[(ProvenPrime, ReducedRational)],
    right: &[(ProvenPrime, ReducedRational)],
    subtract: bool,
    mut emit: F,
) -> Result<(), MonomialErrorKind>
where
    F: FnMut(&ProvenPrime, ReducedRational) -> Result<(), MonomialErrorKind>,
{
    let mut left_index = 0;
    let mut right_index = 0;
    while left_index < left.len() || right_index < right.len() {
        match (left.get(left_index), right.get(right_index)) {
            (Some((left_prime, left_exponent)), Some((right_prime, right_exponent))) => {
                match left_prime
                    .value_internal()
                    .cmp(right_prime.value_internal())
                {
                    Ordering::Less => {
                        emit(left_prime, left_exponent.try_clone()?)?;
                        left_index += 1;
                    }
                    Ordering::Greater => {
                        let exponent = if subtract {
                            negate_rational(right_exponent)?
                        } else {
                            right_exponent.try_clone()?
                        };
                        emit(right_prime, exponent)?;
                        right_index += 1;
                    }
                    Ordering::Equal => {
                        let exponent = if subtract {
                            left_exponent.sub(right_exponent)?
                        } else {
                            left_exponent.add(right_exponent)?
                        };
                        if !exponent.is_zero() {
                            emit(left_prime, exponent)?;
                        }
                        left_index += 1;
                        right_index += 1;
                    }
                }
            }
            (Some((prime, exponent)), None) => {
                emit(prime, exponent.try_clone()?)?;
                left_index += 1;
            }
            (None, Some((prime, exponent))) => {
                let exponent = if subtract {
                    negate_rational(exponent)?
                } else {
                    exponent.try_clone()?
                };
                emit(prime, exponent)?;
                right_index += 1;
            }
            (None, None) => break,
        }
    }
    Ok(())
}

fn negate_rational(value: &ReducedRational) -> Result<ReducedRational, MonomialErrorKind> {
    RawRational::new(
        value.numerator().negated()?,
        value.denominator().try_clone()?,
    )
    .reduce()
    .map(|reduction| reduction.into_reduced())
    .map_err(Into::into)
}

fn reduced_integer(value: i32) -> Result<ReducedRational, MonomialErrorKind> {
    ReducedRational::from_bigint(BigInt::try_from(value)?).map_err(Into::into)
}

fn multiply_sign(left: Sign, right: Sign) -> Sign {
    if left == right {
        Sign::Positive
    } else {
        Sign::Negative
    }
}
