use alloc::vec::Vec;

use neco_bigint::{BigInt, BigUint, Dyadic, DyadicEnclosure, RawRational, ReducedRational, Sign};

use crate::error::{reserve_elements_at, AlgnumError, AllocationContact, AllocationResource};
use crate::polynomial::{Polynomial, RationalPolynomial};

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SturmSequence {
    polynomials: Vec<Polynomial>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct RootIsolation {
    pub(crate) index: usize,
    pub(crate) enclosure: DyadicEnclosure,
}

impl SturmSequence {
    pub(crate) fn new(polynomial: &Polynomial) -> Result<Self, AlgnumError> {
        if polynomial.degree().is_none_or(|degree| degree == 0) {
            return Err(AlgnumError::ZeroPolynomial);
        }

        let count = build_sturm_polynomials(polynomial, None)?;
        let mut polynomials = Vec::new();
        reserve_elements_at(
            &mut polynomials,
            count,
            AllocationResource::SturmSequence,
            AllocationContact::SturmPolynomials,
        )?;
        build_sturm_polynomials(polynomial, Some(&mut polynomials))?;
        Ok(Self { polynomials })
    }

    pub(crate) fn root_count(&self, lower: &Dyadic, upper: &Dyadic) -> Result<usize, AlgnumError> {
        if lower >= upper || self.is_root(lower)? || self.is_root(upper)? {
            return Err(AlgnumError::InvalidIsolation);
        }
        let lower_variations = self.variations_at(lower)?;
        let upper_variations = self.variations_at(upper)?;
        lower_variations
            .checked_sub(upper_variations)
            .ok_or(AlgnumError::InvalidIsolation)
    }

    pub(crate) fn isolate_real_roots(&self) -> Result<Vec<RootIsolation>, AlgnumError> {
        let (lower, upper) = self.initial_interval()?;
        let total = self.root_count(&lower, &upper)?;
        let mut pending = Vec::new();
        let mut intervals = Vec::new();
        reserve_elements_at(
            &mut pending,
            total,
            AllocationResource::RootIntervals,
            AllocationContact::SturmPending,
        )?;
        reserve_elements_at(
            &mut intervals,
            total,
            AllocationResource::RootIntervals,
            AllocationContact::SturmResults,
        )?;
        if total != 0 {
            pending.push((lower, upper, total));
        }

        while !pending.is_empty() {
            let (lower, upper, count) = pending.remove(0);
            if is_isolated_root_count(count) {
                intervals.push(DyadicEnclosure::new(lower, upper)?);
                continue;
            }
            let children = self.split_interval(lower, upper, count)?;
            for child in children.into_iter().rev() {
                pending.insert(0, child);
            }
        }

        let mut roots = Vec::new();
        reserve_elements_at(
            &mut roots,
            total,
            AllocationResource::RootIntervals,
            AllocationContact::SturmObservations,
        )?;
        for (index, enclosure) in intervals.into_iter().enumerate() {
            roots.push(RootIsolation { index, enclosure });
        }
        Ok(roots)
    }

    pub(crate) fn certify_root(
        &self,
        lower: Dyadic,
        upper: Dyadic,
    ) -> Result<RootIsolation, AlgnumError> {
        if lower >= upper || self.is_root(&lower)? || self.is_root(&upper)? {
            return Err(AlgnumError::InvalidIsolation);
        }
        match self.root_count(&lower, &upper)? {
            0 => return Err(AlgnumError::NoTargetRoot),
            1 => {}
            _ => return Err(AlgnumError::MultipleTargetRoots),
        }

        for root in self.isolate_real_roots()? {
            let canonical_lower = root.enclosure.lower();
            let canonical_upper = root.enclosure.upper();
            let intersection_lower = if lower >= *canonical_lower {
                lower.try_clone()?
            } else {
                canonical_lower.try_clone()?
            };
            let intersection_upper = if upper <= *canonical_upper {
                upper.try_clone()?
            } else {
                canonical_upper.try_clone()?
            };
            if intersection_lower < intersection_upper {
                return Ok(RootIsolation {
                    index: root.index,
                    enclosure: DyadicEnclosure::new(intersection_lower, intersection_upper)?,
                });
            }
        }
        Err(AlgnumError::NoTargetRoot)
    }

    pub(crate) fn refine(
        &self,
        enclosure: &DyadicEnclosure,
        bits: u32,
    ) -> Result<DyadicEnclosure, AlgnumError> {
        let mut lower = enclosure.lower().try_clone()?;
        let mut upper = enclosure.upper().try_clone()?;
        if self.root_count(&lower, &upper)? != 1 {
            return Err(AlgnumError::InvalidIsolation);
        }
        let target = Dyadic::new(BigInt::one()?, bits);
        while {
            let current = DyadicEnclosure::new(lower.try_clone()?, upper.try_clone()?)?;
            current.width()? > target
        } {
            let children = self.split_interval(lower, upper, 1)?;
            let mut selected = None;
            for child in children {
                if child.2 == 1 {
                    selected = Some(child);
                    break;
                }
            }
            let Some((next_lower, next_upper, _)) = selected else {
                return Err(AlgnumError::NoTargetRoot);
            };
            lower = next_lower;
            upper = next_upper;
        }
        Ok(DyadicEnclosure::new(lower, upper)?)
    }

    fn split_interval(
        &self,
        lower: Dyadic,
        upper: Dyadic,
        expected_count: usize,
    ) -> Result<Vec<(Dyadic, Dyadic, usize)>, AlgnumError> {
        let midpoint = lower.midpoint(&upper)?;
        if !self.is_root(&midpoint)? {
            let left_count = self.root_count(&lower, &midpoint)?;
            let right_count = self.root_count(&midpoint, &upper)?;
            if left_count.checked_add(right_count) != Some(expected_count) {
                return Err(AlgnumError::NoTargetRoot);
            }
            return interval_children([
                (lower, midpoint.try_clone()?, left_count),
                (midpoint, upper, right_count),
            ]);
        }

        let mut exponent = initial_search_exponent();
        loop {
            let offset = Dyadic::new(BigInt::one()?, exponent);
            let left = midpoint.sub(&offset)?;
            let right = midpoint.add(&offset)?;
            if lower < left && right < upper && !self.is_root(&left)? && !self.is_root(&right)? {
                let left_count = self.root_count(&lower, &left)?;
                let middle_count = self.root_count(&left, &right)?;
                let right_count = self.root_count(&right, &upper)?;
                if middle_count == 1
                    && left_count
                        .checked_add(middle_count)
                        .and_then(|sum| sum.checked_add(right_count))
                        == Some(expected_count)
                {
                    return interval_children([
                        (lower, left.try_clone()?, left_count),
                        (left, right.try_clone()?, middle_count),
                        (right, upper, right_count),
                    ]);
                }
            }
            let Some(next_exponent) = next_search_exponent(exponent) else {
                return Err(neco_bigint::BigintError::ExponentOverflow {
                    required: BigUint::try_from(u64::from(u32::MAX) + 1)?,
                    maximum: u32::MAX,
                }
                .into());
            };
            exponent = next_exponent;
        }
    }

    fn initial_interval(&self) -> Result<(Dyadic, Dyadic), AlgnumError> {
        let polynomial = self
            .polynomials
            .first()
            .ok_or(AlgnumError::ZeroPolynomial)?;
        let coefficients = polynomial.coefficients();
        let leading = coefficients.last().ok_or(AlgnumError::ZeroPolynomial)?;
        let mut maximum = BigInt::zero();
        for coefficient in coefficients
            .iter()
            .take(coefficients.len().saturating_sub(1))
        {
            let ratio = RawRational::new(
                BigInt::from_sign_magnitude(Sign::Positive, coefficient.magnitude().try_clone()?),
                leading.magnitude().try_clone()?,
            )
            .reduce()?
            .into_reduced();
            let ceil = ratio.ceil()?;
            if ceil > maximum {
                maximum = ceil;
            }
        }
        let bound = maximum.add(&BigInt::one()?)?;
        self.nonroot_interval_from_bound(bound)
    }

    fn nonroot_interval_from_bound(
        &self,
        mut bound: BigInt,
    ) -> Result<(Dyadic, Dyadic), AlgnumError> {
        loop {
            let upper = Dyadic::new(bound.try_clone()?, 0);
            let lower = Dyadic::new(bound.negated()?, 0);
            if !self.is_root(&lower)? && !self.is_root(&upper)? {
                return Ok((lower, upper));
            }
            bound = bound.add(&BigInt::one()?)?;
        }
    }

    fn is_root(&self, point: &Dyadic) -> Result<bool, AlgnumError> {
        let polynomial = self
            .polynomials
            .first()
            .ok_or(AlgnumError::ZeroPolynomial)?;
        Ok(polynomial
            .evaluate_rational(&dyadic_rational(point)?)?
            .is_zero())
    }

    fn variations_at(&self, point: &Dyadic) -> Result<usize, AlgnumError> {
        let point = dyadic_rational(point)?;
        let mut previous = Sign::Zero;
        let mut variations = 0_usize;
        for polynomial in &self.polynomials {
            let sign = polynomial.evaluate_rational(&point)?.numerator().sign();
            if sign == Sign::Zero {
                continue;
            }
            if previous != Sign::Zero && previous != sign {
                variations += 1;
            }
            previous = sign;
        }
        Ok(variations)
    }
}

fn build_sturm_polynomials(
    polynomial: &Polynomial,
    mut output: Option<&mut Vec<Polynomial>>,
) -> Result<usize, AlgnumError> {
    let first = polynomial.primitive_part()?;
    let mut previous = RationalPolynomial::from_integer(&first)?;
    let mut current = previous.derivative_internal()?;
    let mut count = 1_usize;
    if let Some(values) = output.as_deref_mut() {
        values.push(first);
    }
    if !current.is_zero() {
        count += 1;
        if let Some(values) = output.as_deref_mut() {
            values.push(primitive_integer_preserving_sign(&current)?);
        }
    }
    while !current.is_zero() {
        let (_, remainder) = previous.div_rem(&current)?;
        if remainder.is_zero() {
            break;
        }
        let next = negate_rational_polynomial(&remainder)?;
        count += 1;
        if let Some(values) = output.as_deref_mut() {
            values.push(primitive_integer_preserving_sign(&next)?);
        }
        previous = current;
        current = next;
    }
    Ok(count)
}

fn next_search_exponent(exponent: u32) -> Option<u32> {
    exponent.checked_add(1)
}

fn initial_search_exponent() -> u32 {
    1
}

fn is_isolated_root_count(count: usize) -> bool {
    count == 1
}

fn interval_children<const N: usize>(
    candidates: [(Dyadic, Dyadic, usize); N],
) -> Result<Vec<(Dyadic, Dyadic, usize)>, AlgnumError> {
    let retained = candidates
        .iter()
        .filter(|candidate| candidate.2 != 0)
        .count();
    let mut children = Vec::new();
    reserve_elements_at(
        &mut children,
        retained,
        AllocationResource::RootIntervals,
        AllocationContact::SturmChildren,
    )?;
    for candidate in candidates {
        if candidate.2 != 0 {
            children.push(candidate);
        }
    }
    Ok(children)
}

fn dyadic_rational(value: &Dyadic) -> Result<ReducedRational, AlgnumError> {
    let denominator = BigUint::one()?.shl_bits(value.exponent() as usize)?;
    Ok(RawRational::new(value.integer().try_clone()?, denominator)
        .reduce()?
        .into_reduced())
}

fn negate_rational_polynomial(
    polynomial: &RationalPolynomial,
) -> Result<RationalPolynomial, AlgnumError> {
    let mut coefficients = Vec::new();
    reserve_elements_at(
        &mut coefficients,
        polynomial.coefficients().len(),
        AllocationResource::RationalCoefficients,
        AllocationContact::SturmNegatedRational,
    )?;
    for coefficient in polynomial.coefficients() {
        coefficients.push(
            RawRational::new(
                coefficient.numerator().negated()?,
                coefficient.denominator().try_clone()?,
            )
            .reduce()?
            .into_reduced(),
        );
    }
    Ok(RationalPolynomial::from_coefficients(coefficients))
}

fn primitive_integer_preserving_sign(
    polynomial: &RationalPolynomial,
) -> Result<Polynomial, AlgnumError> {
    if polynomial.is_zero() {
        return Ok(Polynomial::zero());
    }
    let mut denominator = BigUint::one()?;
    for coefficient in polynomial.coefficients() {
        denominator = denominator.lcm(coefficient.denominator())?;
    }
    let mut integers = Vec::new();
    reserve_elements_at(
        &mut integers,
        polynomial.coefficients().len(),
        AllocationResource::PolynomialCoefficients,
        AllocationContact::SturmPrimitiveInteger,
    )?;
    for coefficient in polynomial.coefficients() {
        let factor = denominator.exact_div(coefficient.denominator())?;
        integers.push(BigInt::from_sign_magnitude(
            coefficient.numerator().sign(),
            coefficient.numerator().magnitude().mul(&factor)?,
        ));
    }
    let integer = Polynomial::from_coefficients(integers);
    let primitive = integer.primitive_part()?;
    let original_sign = polynomial
        .coefficients()
        .last()
        .map(|value| value.numerator().sign())
        .ok_or(AlgnumError::ZeroPolynomial)?;
    if original_sign == Sign::Negative {
        negate_integer_polynomial(&primitive)
    } else {
        Ok(primitive)
    }
}

fn negate_integer_polynomial(polynomial: &Polynomial) -> Result<Polynomial, AlgnumError> {
    let mut coefficients = Vec::new();
    reserve_elements_at(
        &mut coefficients,
        polynomial.coefficients().len(),
        AllocationResource::PolynomialCoefficients,
        AllocationContact::SturmNegatedInteger,
    )?;
    for coefficient in polynomial.coefficients() {
        coefficients.push(coefficient.negated()?);
    }
    Ok(Polynomial::from_coefficients(coefficients))
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use neco_bigint::{BigInt, Dyadic};

    use super::{
        build_sturm_polynomials, initial_search_exponent, is_isolated_root_count,
        next_search_exponent, primitive_integer_preserving_sign, SturmSequence,
    };
    use crate::error::{with_injected_failure, AllocationContact};
    use crate::{AlgnumError, AllocationResource, Polynomial, RationalPolynomial};

    fn integer(value: i32) -> BigInt {
        BigInt::try_from(value).unwrap()
    }

    fn dyadic(value: i32) -> Dyadic {
        Dyadic::new(integer(value), 0)
    }

    #[test]
    fn isolates_three_roots_in_ascending_order() {
        let polynomial =
            Polynomial::from_coefficients(vec![integer(0), integer(-1), integer(0), integer(1)]);
        let sequence = SturmSequence::new(&polynomial).unwrap();
        let roots = sequence.isolate_real_roots().unwrap();
        assert_eq!(roots.len(), 3);
        assert_eq!(roots[0].index, 0);
        assert_eq!(roots[1].index, 1);
        assert_eq!(roots[2].index, 2);
        assert!(roots[0].enclosure.upper() <= roots[1].enclosure.lower());
        assert!(roots[1].enclosure.upper() <= roots[2].enclosure.lower());
    }

    #[test]
    fn construction_interval_distinguishes_all_invalid_cases() {
        let polynomial = Polynomial::from_coefficients(vec![integer(-1), integer(0), integer(1)]);
        let sequence = SturmSequence::new(&polynomial).unwrap();
        assert_eq!(
            sequence.certify_root(dyadic(1), dyadic(2)),
            Err(AlgnumError::InvalidIsolation)
        );
        assert_eq!(
            sequence.root_count(&dyadic(1), &dyadic(2)),
            Err(AlgnumError::InvalidIsolation)
        );
        assert_eq!(
            sequence.certify_root(dyadic(2), dyadic(3)),
            Err(AlgnumError::NoTargetRoot)
        );
        assert_eq!(
            sequence.certify_root(dyadic(-2), dyadic(2)),
            Err(AlgnumError::MultipleTargetRoots)
        );
    }

    #[test]
    fn refinement_preserves_the_selected_root() {
        let polynomial = Polynomial::from_coefficients(vec![integer(-2), integer(0), integer(1)]);
        let sequence = SturmSequence::new(&polynomial).unwrap();
        let root = sequence.certify_root(dyadic(1), dyadic(2)).unwrap();
        assert_eq!(sequence.refine(&root.enclosure, 0).unwrap(), root.enclosure);
        let refined = sequence.refine(&root.enclosure, 16).unwrap();
        assert_eq!(
            sequence
                .root_count(refined.lower(), refined.upper())
                .unwrap(),
            1
        );
        assert!(refined.width().unwrap() <= Dyadic::new(integer(1), 16));
    }

    #[test]
    fn sturm_storage_count_uses_the_actual_nonzero_euclid_terms() {
        let polynomial = Polynomial::from_coefficients(vec![integer(0), integer(0), integer(1)]);
        assert_eq!(build_sturm_polynomials(&polynomial, None).unwrap(), 2);
        assert_eq!(
            with_injected_failure(AllocationContact::SturmPolynomials, || {
                SturmSequence::new(&polynomial)
            }),
            Err(AlgnumError::AllocationFailure {
                resource: AllocationResource::SturmSequence,
                requested: 2,
            })
        );
        let sequence = SturmSequence::new(&polynomial).unwrap();
        assert_eq!(sequence.polynomials.len(), 2);
    }

    #[test]
    fn primitive_conversion_preserves_a_negative_leading_coefficient() {
        let polynomial = Polynomial::from_coefficients(vec![integer(1), integer(-2)]);
        let rational = RationalPolynomial::from_integer(&polynomial).unwrap();
        assert_eq!(
            primitive_integer_preserving_sign(&rational).unwrap(),
            polynomial
        );
    }

    #[test]
    fn ordinary_split_uses_the_exact_dyadic_midpoint() {
        let polynomial = Polynomial::from_coefficients(vec![integer(-2), integer(0), integer(1)]);
        let sequence = SturmSequence::new(&polynomial).unwrap();
        let children = sequence.split_interval(dyadic(-2), dyadic(2), 2).unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].1, dyadic(0));
        assert_eq!(children[1].0, dyadic(0));
    }

    #[test]
    fn root_bound_rounds_a_noninteger_coefficient_ratio_upward() {
        let polynomial = Polynomial::from_coefficients(vec![integer(-3), integer(2)]);
        let sequence = SturmSequence::new(&polynomial).unwrap();
        assert_eq!(
            sequence.initial_interval().unwrap(),
            (dyadic(-3), dyadic(3))
        );
    }

    #[test]
    fn root_bound_moves_past_an_endpoint_root() {
        let polynomial = Polynomial::from_coefficients(vec![integer(-1), integer(1)]);
        let sequence = SturmSequence::new(&polynomial).unwrap();
        assert_eq!(
            sequence.nonroot_interval_from_bound(integer(1)).unwrap(),
            (dyadic(-2), dyadic(2))
        );
    }

    #[test]
    fn midpoint_root_search_advances_one_exponent_at_a_time() {
        assert_eq!(initial_search_exponent(), 1);
        assert_eq!(next_search_exponent(1), Some(2));
        assert_eq!(next_search_exponent(u32::MAX), None);
    }

    #[test]
    fn only_one_root_marks_an_interval_as_isolated() {
        assert!(!is_isolated_root_count(0));
        assert!(is_isolated_root_count(1));
        assert!(!is_isolated_root_count(2));
    }
}
