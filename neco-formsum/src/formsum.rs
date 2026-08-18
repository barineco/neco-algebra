use alloc::vec::Vec;
use core::cmp::Ordering;

use neco_bigint::{BigInt, BigUint, BigintError, RawRational, ReducedRational};
use neco_monomial::{Monomial, MonomialErrorKind, NormalizationErrors, ProvenPrime, RadicalBasis};

use crate::error::{reserve_elements, AllocationTarget, DimensionResource, FormSumErrorKind};
use crate::raw::{RawFormSum, RawTerm};

#[derive(Debug, Eq, PartialEq)]
pub struct FormSum {
    terms: Vec<(RadicalBasis, ReducedRational)>,
}

impl FormSum {
    pub fn zero() -> Self {
        Self { terms: Vec::new() }
    }

    pub fn one() -> Result<Self, FormSumErrorKind> {
        let mut terms = Vec::new();
        reserve_elements(
            &mut terms,
            1,
            DimensionResource::BasisCount,
            AllocationTarget::NormalTerms,
        )?;
        terms.push((RadicalBasis::one(), rational_integer(1)?));
        Ok(Self { terms })
    }

    pub fn from_monomial(value: &Monomial) -> Result<Self, FormSumErrorKind> {
        if value.is_zero() {
            return Ok(Self::zero());
        }
        let (coefficient, basis) = value.split_radical()?;
        let mut terms = Vec::new();
        reserve_elements(
            &mut terms,
            1,
            DimensionResource::BasisCount,
            AllocationTarget::NormalTerms,
        )?;
        terms.push((basis, coefficient));
        Ok(Self { terms })
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn terms(&self) -> &[(RadicalBasis, ReducedRational)] {
        &self.terms
    }

    pub fn try_clone(&self) -> Result<Self, FormSumErrorKind> {
        let mut terms = Vec::new();
        reserve_elements(
            &mut terms,
            self.terms.len(),
            DimensionResource::BasisCount,
            AllocationTarget::NormalTerms,
        )?;
        for (basis, coefficient) in &self.terms {
            terms.push((basis.try_clone()?, coefficient.try_clone()?));
        }
        Ok(Self { terms })
    }

    pub fn add(&self, rhs: &Self) -> Result<Self, FormSumErrorKind> {
        self.merge(rhs, false)
    }

    pub fn sub(&self, rhs: &Self) -> Result<Self, FormSumErrorKind> {
        self.merge(rhs, true)
    }

    pub fn mul(&self, rhs: &Self) -> Result<Self, FormSumErrorKind> {
        if self.is_zero() || rhs.is_zero() {
            return Ok(Self::zero());
        }
        let count = visit_products(self, rhs, |_, _, _| Ok(()))?;
        let mut terms = Vec::new();
        reserve_elements(
            &mut terms,
            count,
            DimensionResource::BasisCount,
            AllocationTarget::ProductResult,
        )?;
        visit_products(self, rhs, |left_index, right_index, coefficient| {
            let basis = product_basis(&self.terms[left_index].0, &rhs.terms[right_index].0)?;
            terms.push((basis, coefficient));
            Ok(())
        })?;
        Ok(Self { terms })
    }

    fn merge(&self, rhs: &Self, subtract: bool) -> Result<Self, FormSumErrorKind> {
        let count = visit_merge(&self.terms, &rhs.terms, subtract, |_, _| Ok(()))?;
        let mut terms = Vec::new();
        reserve_elements(
            &mut terms,
            count,
            DimensionResource::BasisCount,
            AllocationTarget::MergeResult,
        )?;
        visit_merge(&self.terms, &rhs.terms, subtract, |basis, coefficient| {
            terms.push((basis.try_clone()?, coefficient));
            Ok(())
        })?;
        Ok(Self { terms })
    }

    pub(crate) fn from_sorted_terms(terms: Vec<(RadicalBasis, ReducedRational)>) -> Self {
        Self { terms }
    }
}

pub(crate) fn normalize_raw(
    raw: &RawFormSum,
) -> Result<FormSum, NormalizationErrors<FormSumErrorKind>> {
    let indices = raw
        .sorted_indices()
        .map_err(NormalizationErrors::from_one)?;
    let mut semantic = 0_u8;
    visit_semantic_errors(raw, &indices, 1, |error| {
        semantic |= semantic_mask(&error);
        Ok(())
    })
    .map_err(NormalizationErrors::from_one)?;
    if semantic != 0 {
        let count = semantic.count_ones() as usize;
        let mut errors = Vec::new();
        reserve_elements(
            &mut errors,
            count,
            DimensionResource::BasisCount,
            AllocationTarget::ErrorSet,
        )
        .map_err(NormalizationErrors::from_one)?;
        visit_semantic_errors(raw, &indices, 2, |error| {
            if !errors.contains(&error) {
                errors.push(error);
            }
            Ok(())
        })
        .map_err(NormalizationErrors::from_one)?;
        if let Some(errors) = NormalizationErrors::from_errors(errors) {
            return Err(errors);
        }
    }

    let count = visit_normalized_terms(raw, &indices, |_, _| Ok(()))
        .map_err(NormalizationErrors::from_one)?;
    let mut terms = Vec::new();
    reserve_elements(
        &mut terms,
        count,
        DimensionResource::BasisCount,
        AllocationTarget::NormalTerms,
    )
    .map_err(NormalizationErrors::from_one)?;
    visit_normalized_terms(raw, &indices, |basis, coefficient| {
        terms.push((basis, coefficient));
        Ok(())
    })
    .map_err(NormalizationErrors::from_one)?;
    Ok(FormSum::from_sorted_terms(terms))
}

const COEFFICIENT_ZERO_DENOMINATOR: u8 = 1;
const MONOMIAL_NEGATIVE_ZERO: u8 = 2;
const MONOMIAL_UNDEFINED_ZERO: u8 = 4;
const MONOMIAL_ZERO_DENOMINATOR: u8 = 8;

fn visit_semantic_errors<F>(
    raw: &RawFormSum,
    indices: &[usize],
    scan: u8,
    mut emit: F,
) -> Result<(), FormSumErrorKind>
where
    F: FnMut(FormSumErrorKind) -> Result<(), FormSumErrorKind>,
{
    for (position, index) in indices.iter().enumerate() {
        #[cfg(test)]
        observe_validation_position(scan, position)?;
        #[cfg(not(test))]
        let _ = (scan, position);
        let term = &raw.terms()[*index];
        match term.coefficient().reduce() {
            Ok(_) => {}
            Err(BigintError::ZeroDenominator) => {
                emit(FormSumErrorKind::Bigint(BigintError::ZeroDenominator))?
            }
            Err(error) => return Err(error.into()),
        }
        match term.monomial().normalize() {
            Ok(_) => {}
            Err(errors) => {
                let (first, additional) = errors.into_parts();
                visit_monomial_error(first, &mut emit)?;
                for error in additional {
                    visit_monomial_error(error, &mut emit)?;
                }
            }
        }
    }
    Ok(())
}

fn visit_monomial_error<F>(error: MonomialErrorKind, emit: &mut F) -> Result<(), FormSumErrorKind>
where
    F: FnMut(FormSumErrorKind) -> Result<(), FormSumErrorKind>,
{
    match error {
        MonomialErrorKind::ZeroToNegativePower
        | MonomialErrorKind::UndefinedZeroPower
        | MonomialErrorKind::Bigint(BigintError::ZeroDenominator) => {
            emit(FormSumErrorKind::Monomial(error))?
        }
        error => return Err(FormSumErrorKind::Monomial(error)),
    }
    Ok(())
}

fn semantic_mask(error: &FormSumErrorKind) -> u8 {
    match error {
        FormSumErrorKind::Bigint(BigintError::ZeroDenominator) => COEFFICIENT_ZERO_DENOMINATOR,
        FormSumErrorKind::Monomial(MonomialErrorKind::ZeroToNegativePower) => {
            MONOMIAL_NEGATIVE_ZERO
        }
        FormSumErrorKind::Monomial(MonomialErrorKind::UndefinedZeroPower) => {
            MONOMIAL_UNDEFINED_ZERO
        }
        FormSumErrorKind::Monomial(MonomialErrorKind::Bigint(BigintError::ZeroDenominator)) => {
            MONOMIAL_ZERO_DENOMINATOR
        }
        _ => 0,
    }
}

#[cfg(test)]
std::thread_local! {
    static VALIDATION_FAILURE: core::cell::Cell<Option<(u8, usize)>> = const { core::cell::Cell::new(None) };
    static VALIDATION_VISITS: core::cell::RefCell<Vec<(u8, usize)>> = const { core::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
fn observe_validation_position(scan: u8, position: usize) -> Result<(), FormSumErrorKind> {
    VALIDATION_VISITS.with(|visits| visits.borrow_mut().push((scan, position)));
    let injected = VALIDATION_FAILURE.with(|failure| failure.get() == Some((scan, position)));
    if injected {
        Err(FormSumErrorKind::Monomial(
            MonomialErrorKind::CapacityOverflow,
        ))
    } else {
        Ok(())
    }
}

fn visit_normalized_terms<F>(
    raw: &RawFormSum,
    indices: &[usize],
    mut emit: F,
) -> Result<usize, FormSumErrorKind>
where
    F: FnMut(RadicalBasis, ReducedRational) -> Result<(), FormSumErrorKind>,
{
    let mut previous: Option<RadicalBasis> = None;
    let mut total = 0_usize;
    while let Some(next) = next_raw_basis(raw, indices, previous.as_ref())? {
        let mut coefficient = rational_integer(0)?;
        for index in indices {
            if let Some((basis, value)) = normalize_term(&raw.terms()[*index])? {
                if basis == next {
                    coefficient = coefficient.add(&value)?;
                }
            }
        }
        if !coefficient.is_zero() {
            total = total.checked_add(1).ok_or_else(basis_count_overflow)?;
            emit(next.try_clone()?, coefficient)?;
        }
        previous = Some(next);
    }
    Ok(total)
}

fn next_raw_basis(
    raw: &RawFormSum,
    indices: &[usize],
    previous: Option<&RadicalBasis>,
) -> Result<Option<RadicalBasis>, FormSumErrorKind> {
    let mut next: Option<RadicalBasis> = None;
    for index in indices {
        if let Some((basis, _)) = normalize_term(&raw.terms()[*index])? {
            let after = previous.is_none_or(|previous| basis > *previous);
            let before = next.as_ref().is_none_or(|next| basis < *next);
            if after && before {
                next = Some(basis);
            }
        }
    }
    Ok(next)
}

fn normalize_term(
    raw: &RawTerm,
) -> Result<Option<(RadicalBasis, ReducedRational)>, FormSumErrorKind> {
    let coefficient = raw.coefficient().reduce()?.into_reduced();
    let monomial = raw.monomial().normalize().map_err(first_monomial_error)?;
    if coefficient.is_zero() || monomial.is_zero() {
        return Ok(None);
    }
    let (monomial_coefficient, basis) = monomial.split_radical()?;
    let coefficient = coefficient.mul(&monomial_coefficient)?;
    if coefficient.is_zero() {
        Ok(None)
    } else {
        Ok(Some((basis, coefficient)))
    }
}

fn first_monomial_error(errors: NormalizationErrors<MonomialErrorKind>) -> FormSumErrorKind {
    let (first, _) = errors.into_parts();
    first.into()
}

fn visit_merge<F>(
    left: &[(RadicalBasis, ReducedRational)],
    right: &[(RadicalBasis, ReducedRational)],
    subtract: bool,
    mut emit: F,
) -> Result<usize, FormSumErrorKind>
where
    F: FnMut(&RadicalBasis, ReducedRational) -> Result<(), FormSumErrorKind>,
{
    let mut left_index = 0;
    let mut right_index = 0;
    let mut total = 0_usize;
    while left_index < left.len() || right_index < right.len() {
        let (basis, coefficient) = match (left.get(left_index), right.get(right_index)) {
            (Some((lb, lc)), Some((rb, rc))) => match lb.cmp(rb) {
                Ordering::Less => {
                    left_index += 1;
                    (lb, lc.try_clone()?)
                }
                Ordering::Greater => {
                    right_index += 1;
                    (
                        rb,
                        if subtract {
                            negate(rc)?
                        } else {
                            rc.try_clone()?
                        },
                    )
                }
                Ordering::Equal => {
                    left_index += 1;
                    right_index += 1;
                    (lb, if subtract { lc.sub(rc)? } else { lc.add(rc)? })
                }
            },
            (Some((basis, coefficient)), None) => {
                left_index += 1;
                (basis, coefficient.try_clone()?)
            }
            (None, Some((basis, coefficient))) => {
                right_index += 1;
                (
                    basis,
                    if subtract {
                        negate(coefficient)?
                    } else {
                        coefficient.try_clone()?
                    },
                )
            }
            (None, None) => break,
        };
        if !coefficient.is_zero() {
            total = total.checked_add(1).ok_or_else(basis_count_overflow)?;
            emit(basis, coefficient)?;
        }
    }
    Ok(total)
}

fn visit_products<F>(
    left: &FormSum,
    right: &FormSum,
    mut emit: F,
) -> Result<usize, FormSumErrorKind>
where
    F: FnMut(usize, usize, ReducedRational) -> Result<(), FormSumErrorKind>,
{
    let mut previous: Option<(usize, usize)> = None;
    let mut count = 0_usize;
    loop {
        let mut next: Option<(usize, usize)> = None;
        for left_index in 0..left.terms.len() {
            for right_index in 0..right.terms.len() {
                let candidate = (left_index, right_index);
                let after = match previous {
                    Some(previous) => {
                        compare_product_basis(left, right, candidate, previous)?
                            == Ordering::Greater
                    }
                    None => true,
                };
                let before = match next {
                    Some(next) => {
                        compare_product_basis(left, right, candidate, next)? == Ordering::Less
                    }
                    None => true,
                };
                if after && before {
                    next = Some(candidate);
                }
            }
        }
        let Some(next_pair) = next else { break };
        let mut coefficient = rational_integer(0)?;
        for left_index in 0..left.terms.len() {
            for right_index in 0..right.terms.len() {
                let candidate = (left_index, right_index);
                if compare_product_basis(left, right, candidate, next_pair)? == Ordering::Equal {
                    coefficient = coefficient.add(&product_coefficient(
                        &left.terms[left_index],
                        &right.terms[right_index],
                    )?)?;
                }
            }
        }
        if !coefficient.is_zero() {
            count = count.checked_add(1).ok_or_else(basis_count_overflow)?;
            emit(next_pair.0, next_pair.1, coefficient)?;
        }
        previous = Some(next_pair);
    }
    Ok(count)
}

fn product_coefficient(
    left: &(RadicalBasis, ReducedRational),
    right: &(RadicalBasis, ReducedRational),
) -> Result<ReducedRational, FormSumErrorKind> {
    let mut coefficient = left.1.mul(&right.1)?;
    visit_basis_product(&left.0, &right.0, |prime, _, carry| {
        if carry {
            coefficient = coefficient.mul(&rational_biguint(prime.value().try_clone()?)?)?;
        }
        Ok(())
    })?;
    Ok(coefficient)
}

fn compare_product_basis(
    left: &FormSum,
    right: &FormSum,
    first: (usize, usize),
    second: (usize, usize),
) -> Result<Ordering, FormSumErrorKind> {
    let first_left = &left.terms[first.0].0;
    let first_right = &right.terms[first.1].0;
    let second_left = &left.terms[second.0].0;
    let second_right = &right.terms[second.1].0;
    let mut rank = 0_usize;
    loop {
        let first_factor = product_factor_at(first_left, first_right, rank)?;
        let second_factor = product_factor_at(second_left, second_right, rank)?;
        match (first_factor, second_factor) {
            (Some((first_prime, first_exponent)), Some((second_prime, second_exponent))) => {
                let ordering = first_prime
                    .cmp(second_prime)
                    .then_with(|| first_exponent.cmp(&second_exponent));
                if ordering != Ordering::Equal {
                    return Ok(ordering);
                }
            }
            (Some(_), None) => return Ok(Ordering::Greater),
            (None, Some(_)) => return Ok(Ordering::Less),
            (None, None) => return Ok(Ordering::Equal),
        }
        rank = rank.checked_add(1).ok_or_else(basis_count_overflow)?;
    }
}

fn product_factor_at<'a>(
    left: &'a RadicalBasis,
    right: &'a RadicalBasis,
    target_rank: usize,
) -> Result<Option<(&'a ProvenPrime, ReducedRational)>, FormSumErrorKind> {
    let mut left_index = 0_usize;
    let mut right_index = 0_usize;
    let mut rank = 0_usize;
    while let Some((prime, exponent, _)) =
        next_basis_product_factor(left, right, &mut left_index, &mut right_index)?
    {
        if !exponent.is_zero() {
            if rank == target_rank {
                return Ok(Some((prime, exponent)));
            }
            rank = rank.checked_add(1).ok_or_else(basis_count_overflow)?;
        }
    }
    Ok(None)
}

fn product_basis(
    left: &RadicalBasis,
    right: &RadicalBasis,
) -> Result<RadicalBasis, FormSumErrorKind> {
    let factor_count = visit_basis_product(left, right, |_, _, _| Ok(()))?;
    let mut factors = Vec::new();
    reserve_elements(
        &mut factors,
        factor_count,
        DimensionResource::BasisCount,
        AllocationTarget::ProductFactors,
    )?;
    visit_basis_product(left, right, |prime, exponent, _| {
        if !exponent.is_zero() {
            factors.push((prime.try_clone()?, exponent));
        }
        Ok(())
    })?;
    Ok(RadicalBasis::try_from_sorted_factors(factors)?)
}

fn visit_basis_product<F>(
    left: &RadicalBasis,
    right: &RadicalBasis,
    mut emit: F,
) -> Result<usize, FormSumErrorKind>
where
    F: FnMut(&ProvenPrime, ReducedRational, bool) -> Result<(), FormSumErrorKind>,
{
    let mut left_index = 0_usize;
    let mut right_index = 0_usize;
    let mut count = 0_usize;
    while let Some((prime, remainder, carry)) =
        next_basis_product_factor(left, right, &mut left_index, &mut right_index)?
    {
        if !remainder.is_zero() {
            count = count.checked_add(1).ok_or_else(basis_count_overflow)?;
        }
        emit(prime, remainder, carry)?;
    }
    Ok(count)
}

fn next_basis_product_factor<'a>(
    left: &'a RadicalBasis,
    right: &'a RadicalBasis,
    left_index: &mut usize,
    right_index: &mut usize,
) -> Result<Option<(&'a ProvenPrime, ReducedRational, bool)>, FormSumErrorKind> {
    let (prime, exponent) = match (
        left.factors().get(*left_index),
        right.factors().get(*right_index),
    ) {
        (Some((left_prime, left_exponent)), Some((right_prime, right_exponent))) => {
            match left_prime.cmp(right_prime) {
                Ordering::Less => {
                    *left_index += 1;
                    (left_prime, left_exponent.try_clone()?)
                }
                Ordering::Greater => {
                    *right_index += 1;
                    (right_prime, right_exponent.try_clone()?)
                }
                Ordering::Equal => {
                    *left_index += 1;
                    *right_index += 1;
                    (left_prime, left_exponent.add(right_exponent)?)
                }
            }
        }
        (Some((prime, exponent)), None) => {
            *left_index += 1;
            (prime, exponent.try_clone()?)
        }
        (None, Some((prime, exponent))) => {
            *right_index += 1;
            (prime, exponent.try_clone()?)
        }
        (None, None) => return Ok(None),
    };
    let carry = exponent.numerator().magnitude() >= exponent.denominator();
    let remainder = if carry {
        exponent.sub(&rational_integer(1)?)?
    } else {
        exponent
    };
    Ok(Some((prime, remainder, carry)))
}

fn negate(value: &ReducedRational) -> Result<ReducedRational, FormSumErrorKind> {
    Ok(RawRational::new(
        value.numerator().negated()?,
        value.denominator().try_clone()?,
    )
    .reduce()?
    .into_reduced())
}

fn rational_integer(value: i32) -> Result<ReducedRational, FormSumErrorKind> {
    Ok(ReducedRational::from_bigint(BigInt::try_from(value)?)?)
}

fn rational_biguint(value: BigUint) -> Result<ReducedRational, FormSumErrorKind> {
    Ok(ReducedRational::from_bigint(BigInt::from_sign_magnitude(
        neco_bigint::Sign::Positive,
        value,
    ))?)
}

fn basis_count_overflow() -> FormSumErrorKind {
    let required = BigUint::try_from(usize::MAX).and_then(|maximum| maximum.add(&BigUint::one()?));
    match required {
        Ok(required) => match BigUint::try_from(usize::MAX) {
            Ok(maximum) => FormSumErrorKind::DimensionOverflow {
                resource: DimensionResource::BasisCount,
                maximum,
                required,
            },
            Err(error) => FormSumErrorKind::Bigint(error),
        },
        Err(error) => FormSumErrorKind::Bigint(error),
    }
}

#[cfg(test)]
mod validation_tests {
    use alloc::{vec, vec::Vec};

    use neco_bigint::{BigInt, BigUint, RawRational};
    use neco_monomial::{MonomialErrorKind, RawMonomial, RawPower};

    use super::{VALIDATION_FAILURE, VALIDATION_VISITS};
    use crate::{FormSumErrorKind, RawFormSum, RawTerm};

    fn rational(numerator: i32, denominator: u8) -> RawRational {
        RawRational::new(
            BigInt::try_from(numerator).unwrap(),
            BigUint::try_from(denominator).unwrap(),
        )
    }

    #[test]
    fn second_validation_scan_discards_collected_errors_on_operational_failure() {
        let raw = RawFormSum::new(vec![
            RawTerm::new(rational(1, 0), RawMonomial::positive(Vec::new())),
            RawTerm::new(
                rational(1, 1),
                RawMonomial::positive(vec![RawPower::new(BigUint::zero(), rational(0, 1))]),
            ),
        ]);
        VALIDATION_VISITS.with(|visits| visits.borrow_mut().clear());
        VALIDATION_FAILURE.with(|failure| failure.set(Some((2, 1))));
        let errors = raw.normalize().unwrap_err();
        VALIDATION_FAILURE.with(|failure| failure.set(None));

        assert!(errors.errors().eq([&FormSumErrorKind::Monomial(
            MonomialErrorKind::CapacityOverflow,
        )]));
        assert_eq!(
            VALIDATION_VISITS.with(|visits| visits.borrow().clone()),
            [(1, 0), (1, 1), (2, 0), (2, 1)]
        );
    }
}
