use alloc::vec::Vec;
use core::cmp::Ordering;

use neco_bigint::{BigInt, BigUint, ReducedRational, Sign};

use crate::error::{
    allocation_total_to_usize, reserve_elements_at, AlgnumError, AllocationContact,
    AllocationResource, RepresentationResource,
};
use crate::polynomial::{Polynomial, SquareFreePolynomial};

#[derive(Debug, Eq, PartialEq)]
pub struct IrreduciblePolynomial {
    polynomial: Polynomial,
}

impl IrreduciblePolynomial {
    pub fn polynomial(&self) -> &Polynomial {
        &self.polynomial
    }

    pub fn try_clone(&self) -> Result<Self, AlgnumError> {
        Ok(Self {
            polynomial: self.polynomial.try_clone()?,
        })
    }

    pub(crate) fn from_polynomial(polynomial: Polynomial) -> Self {
        Self { polynomial }
    }
}

impl SquareFreePolynomial {
    pub fn factor(&self) -> Result<Vec<IrreduciblePolynomial>, AlgnumError> {
        let count = count_irreducible(self.polynomial())?;
        let count = allocation_total_to_usize(&count, AllocationResource::Factors)?;
        let mut factors = Vec::new();
        reserve_elements_at(
            &mut factors,
            count,
            AllocationResource::Factors,
            AllocationContact::FactorOutput,
        )?;
        factor_recursive(self.polynomial(), &mut factors)?;
        factors.sort_unstable_by(compare_irreducible);
        Ok(factors)
    }
}

fn count_irreducible(polynomial: &Polynomial) -> Result<BigUint, AlgnumError> {
    let Some(factor) = selected_factor(polynomial)? else {
        return Ok(BigUint::one()?);
    };
    let quotient = polynomial.exact_div(&factor)?;
    Ok(count_irreducible(&factor)?.add(&count_irreducible(&quotient)?)?)
}

fn factor_recursive(
    polynomial: &Polynomial,
    output: &mut Vec<IrreduciblePolynomial>,
) -> Result<(), AlgnumError> {
    let Some(factor) = selected_factor(polynomial)? else {
        output.push(IrreduciblePolynomial::from_polynomial(
            polynomial.try_clone()?,
        ));
        return Ok(());
    };
    let quotient = polynomial.exact_div(&factor)?;
    factor_recursive(&factor, output)?;
    factor_recursive(&quotient, output)
}

fn selected_factor(polynomial: &Polynomial) -> Result<Option<Polynomial>, AlgnumError> {
    let Some(degree) = polynomial.degree() else {
        return Ok(None);
    };
    for candidate_degree in 1..=degree / 2 {
        let count = count_distinct_candidates(polynomial, candidate_degree)?;
        if count.is_zero() {
            continue;
        }
        let count = allocation_total_to_usize(&count, AllocationResource::FactorCandidates)?;
        let mut candidates = Vec::new();
        reserve_elements_at(
            &mut candidates,
            count,
            AllocationResource::FactorCandidates,
            AllocationContact::FactorCandidates,
        )?;
        enumerate_accepted(polynomial, candidate_degree, |rank, candidate| {
            if !appeared_before(polynomial, candidate_degree, rank, &candidate)? {
                candidates.push(candidate);
            }
            Ok(())
        })?;
        let selected = select_minimum_candidate(candidates);
        if selected.is_some() {
            return Ok(selected);
        }
    }
    Ok(None)
}

fn select_minimum_candidate(candidates: Vec<Polynomial>) -> Option<Polynomial> {
    let mut selected: Option<Polynomial> = None;
    for candidate in candidates {
        if selected
            .as_ref()
            .is_none_or(|current| compare_polynomials(&candidate, current) == Ordering::Less)
        {
            selected = Some(candidate);
        }
    }
    selected
}

fn count_distinct_candidates(
    polynomial: &Polynomial,
    candidate_degree: usize,
) -> Result<BigUint, AlgnumError> {
    let mut count = BigUint::zero();
    let one = BigUint::one()?;
    enumerate_accepted(polynomial, candidate_degree, |rank, candidate| {
        if !appeared_before(polynomial, candidate_degree, rank, &candidate)? {
            count = count.add(&one)?;
        }
        Ok(())
    })?;
    Ok(count)
}

fn appeared_before(
    polynomial: &Polynomial,
    candidate_degree: usize,
    target_rank: &BigUint,
    target: &Polynomial,
) -> Result<bool, AlgnumError> {
    let mut found = false;
    enumerate_accepted(polynomial, candidate_degree, |rank, candidate| {
        if rank < target_rank && candidate == *target {
            found = true;
        }
        Ok(())
    })?;
    Ok(found)
}

fn enumerate_accepted<F>(
    polynomial: &Polynomial,
    candidate_degree: usize,
    mut visit: F,
) -> Result<(), AlgnumError>
where
    F: FnMut(&BigUint, Polynomial) -> Result<(), AlgnumError>,
{
    let (points, values) = kronecker_points(polynomial, candidate_degree)?;
    let columns = divisor_columns(&values)?;
    let mut rank = BigUint::zero();
    let one = BigUint::one()?;
    visit_cartesian(&columns, |digits| {
        let current_rank = rank.try_clone()?;
        rank = rank.add(&one)?;
        let Some(interpolated) = interpolate_integer_candidate(&points, &columns, digits)? else {
            return Ok(());
        };
        let candidate = interpolated.primitive_part()?;
        let Some(degree) = candidate.degree() else {
            return Ok(());
        };
        if !candidate_degree_allowed(degree, candidate_degree) {
            return Ok(());
        }
        if polynomial.is_exactly_divisible_by(&candidate)? {
            visit(&current_rank, candidate)
        } else {
            Ok(())
        }
    })
}

fn candidate_degree_allowed(degree: usize, maximum: usize) -> bool {
    degree != 0 && degree <= maximum
}

fn compare_irreducible(left: &IrreduciblePolynomial, right: &IrreduciblePolynomial) -> Ordering {
    left.polynomial()
        .degree()
        .cmp(&right.polynomial().degree())
        .then_with(|| compare_polynomials(left.polynomial(), right.polynomial()))
}

fn compare_polynomials(left: &Polynomial, right: &Polynomial) -> Ordering {
    left.coefficients()
        .iter()
        .rev()
        .cmp(right.coefficients().iter().rev())
}

pub(crate) fn kronecker_points(
    polynomial: &Polynomial,
    degree: usize,
) -> Result<(Vec<BigInt>, Vec<BigInt>), AlgnumError> {
    let count = degree
        .checked_add(1)
        .ok_or_else(coefficient_count_overflow)?;
    let mut points = Vec::new();
    let mut values = Vec::new();
    reserve_elements_at(
        &mut points,
        count,
        AllocationResource::EvaluationPoints,
        AllocationContact::KroneckerPoints,
    )?;
    reserve_elements_at(
        &mut values,
        count,
        AllocationResource::EvaluationPoints,
        AllocationContact::KroneckerValues,
    )?;
    let mut sequence_index = BigUint::zero();
    let one = BigUint::one()?;
    while points.len() < count {
        let point = integer_point(&sequence_index)?;
        let value = polynomial.evaluate_bigint(&point)?;
        if !value.is_zero() {
            points.push(point);
            values.push(value);
        }
        sequence_index = sequence_index.add(&one)?;
    }
    Ok((points, values))
}

pub(crate) fn signed_divisors(value: &BigInt) -> Result<Vec<BigInt>, AlgnumError> {
    let magnitude = value.magnitude();
    let count =
        visit_absolute_divisors(magnitude, |_, _| Ok(()))?.mul(&BigUint::try_from(2_u32)?)?;
    let count = allocation_total_to_usize(&count, AllocationResource::Divisors)?;
    let mut divisors = Vec::new();
    reserve_elements_at(
        &mut divisors,
        count,
        AllocationResource::Divisors,
        AllocationContact::SignedDivisors,
    )?;
    visit_absolute_divisors(magnitude, |small, large| {
        append_signed(&mut divisors, small)?;
        if large != small {
            append_signed(&mut divisors, large)?;
        }
        Ok(())
    })?;
    divisors.sort_unstable_by(compare_signed_divisors);
    Ok(divisors)
}

pub(crate) fn divisor_columns(values: &[BigInt]) -> Result<Vec<Vec<BigInt>>, AlgnumError> {
    let mut columns = Vec::new();
    reserve_elements_at(
        &mut columns,
        values.len(),
        AllocationResource::Divisors,
        AllocationContact::DivisorColumns,
    )?;
    for value in values {
        columns.push(signed_divisors(value)?);
    }
    Ok(columns)
}

pub(crate) fn visit_cartesian<F>(columns: &[Vec<BigInt>], mut visit: F) -> Result<(), AlgnumError>
where
    F: FnMut(&[usize]) -> Result<(), AlgnumError>,
{
    let mut digits = Vec::new();
    reserve_elements_at(
        &mut digits,
        columns.len(),
        AllocationResource::ProductDigits,
        AllocationContact::CartesianDigits,
    )?;
    for column in columns {
        if column.is_empty() {
            return Ok(());
        }
        digits.push(0_usize);
    }
    if columns.is_empty() {
        return visit(&digits);
    }
    loop {
        visit(&digits)?;
        let mut position = digits.len();
        loop {
            if position == 0 {
                return Ok(());
            }
            position -= 1;
            let next = digits[position] + 1;
            if next < columns[position].len() {
                digits[position] = next;
                break;
            }
            digits[position] = 0;
        }
    }
}

pub(crate) fn interpolate_integer_candidate(
    points: &[BigInt],
    columns: &[Vec<BigInt>],
    digits: &[usize],
) -> Result<Option<Polynomial>, AlgnumError> {
    if points.len() != columns.len() || points.len() != digits.len() {
        return Ok(None);
    }
    let coefficient_count = points.len();
    let mut result = zero_rationals(coefficient_count)?;
    for index in 0..points.len() {
        let Some(value) = columns[index].get(digits[index]) else {
            return Ok(None);
        };
        let mut basis = Vec::new();
        reserve_elements_at(
            &mut basis,
            coefficient_count,
            AllocationResource::RationalCoefficients,
            AllocationContact::InterpolationBasis,
        )?;
        basis.push(rational_integer(1)?);
        let mut denominator = BigInt::one()?;
        for other in 0..points.len() {
            if other == index {
                continue;
            }
            basis = multiply_by_linear(&basis, &points[other])?;
            denominator = denominator.mul(&points[index].sub(&points[other])?)?;
        }
        let scale = ReducedRational::from_bigint(value.try_clone()?)?
            .div(&ReducedRational::from_bigint(denominator)?)?;
        for (slot, coefficient) in basis.iter().enumerate() {
            let term = coefficient.mul(&scale)?;
            result[slot] = result[slot].add(&term)?;
        }
    }

    let mut coefficients = Vec::new();
    reserve_elements_at(
        &mut coefficients,
        result.len(),
        AllocationResource::RationalCoefficients,
        AllocationContact::InterpolationOutput,
    )?;
    let one = BigUint::one()?;
    for coefficient in result {
        if coefficient.denominator() != &one {
            return Ok(None);
        }
        coefficients.push(coefficient.numerator().try_clone()?);
    }
    Ok(Some(Polynomial::from_coefficients(coefficients)))
}

fn integer_point(sequence_index: &BigUint) -> Result<BigInt, AlgnumError> {
    if sequence_index.is_zero() {
        return Ok(BigInt::zero());
    }
    let magnitude = sequence_index.add(&BigUint::one()?)?.shr_bits(1)?;
    let sign = if sequence_index.bit(0) {
        Sign::Positive
    } else {
        Sign::Negative
    };
    Ok(BigInt::from_sign_magnitude(sign, magnitude))
}

fn visit_absolute_divisors<F>(value: &BigUint, mut visit: F) -> Result<BigUint, AlgnumError>
where
    F: FnMut(&BigUint, &BigUint) -> Result<(), AlgnumError>,
{
    if value.is_zero() {
        return Ok(BigUint::zero());
    }
    let one = BigUint::one()?;
    let mut candidate = one.try_clone()?;
    let mut count = BigUint::zero();
    loop {
        let (quotient, remainder) = value.div_rem(&candidate)?;
        if candidate > quotient {
            return Ok(count);
        }
        if remainder.is_zero() {
            let increment = if candidate == quotient { 1_u32 } else { 2_u32 };
            count = count.add(&BigUint::try_from(increment)?)?;
            visit(&candidate, &quotient)?;
        }
        candidate = candidate.add(&one)?;
    }
}

fn append_signed(values: &mut Vec<BigInt>, magnitude: &BigUint) -> Result<(), AlgnumError> {
    values.push(BigInt::from_sign_magnitude(
        Sign::Positive,
        magnitude.try_clone()?,
    ));
    values.push(BigInt::from_sign_magnitude(
        Sign::Negative,
        magnitude.try_clone()?,
    ));
    Ok(())
}

fn compare_signed_divisors(left: &BigInt, right: &BigInt) -> Ordering {
    left.magnitude()
        .cmp(right.magnitude())
        .then_with(|| match (left.sign(), right.sign()) {
            (Sign::Positive, Sign::Negative) => Ordering::Less,
            (Sign::Negative, Sign::Positive) => Ordering::Greater,
            _ => Ordering::Equal,
        })
}

fn zero_rationals(count: usize) -> Result<Vec<ReducedRational>, AlgnumError> {
    let mut values = Vec::new();
    reserve_elements_at(
        &mut values,
        count,
        AllocationResource::RationalCoefficients,
        AllocationContact::InterpolationZeroes,
    )?;
    for _ in 0..count {
        values.push(rational_integer(0)?);
    }
    Ok(values)
}

fn rational_integer(value: i32) -> Result<ReducedRational, AlgnumError> {
    Ok(ReducedRational::from_bigint(BigInt::try_from(value)?)?)
}

fn multiply_by_linear(
    value: &[ReducedRational],
    root: &BigInt,
) -> Result<Vec<ReducedRational>, AlgnumError> {
    let count = value
        .len()
        .checked_add(1)
        .ok_or_else(coefficient_count_overflow)?;
    let mut result = zero_rationals(count)?;
    let negative_root = ReducedRational::from_bigint(root.negated()?)?;
    for (index, coefficient) in value.iter().enumerate() {
        result[index] = result[index].add(&coefficient.mul(&negative_root)?)?;
        result[index + 1] = result[index + 1].add(coefficient)?;
    }
    Ok(result)
}

fn coefficient_count_overflow() -> AlgnumError {
    let maximum = match BigUint::try_from(usize::MAX) {
        Ok(value) => value,
        Err(error) => return error.into(),
    };
    let one = match BigUint::one() {
        Ok(value) => value,
        Err(error) => return error.into(),
    };
    let required = match maximum.add(&one) {
        Ok(value) => value,
        Err(error) => return error.into(),
    };
    AlgnumError::RepresentationLimit {
        resource: RepresentationResource::CoefficientCount,
        required,
        maximum,
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use neco_bigint::{BigInt, Sign};

    use super::{
        candidate_degree_allowed, kronecker_points, select_minimum_candidate, signed_divisors,
        visit_cartesian,
    };
    use crate::polynomial::Polynomial;

    fn polynomial(coefficients: &[i32]) -> Polynomial {
        Polynomial::from_coefficients(
            coefficients
                .iter()
                .map(|value| BigInt::try_from(*value).expect("small test coefficient"))
                .collect(),
        )
    }

    fn integer(value: &BigInt) -> i32 {
        let magnitude = value.magnitude().to_u32().expect("small test magnitude") as i32;
        if value.sign() == Sign::Negative {
            -magnitude
        } else {
            magnitude
        }
    }

    #[test]
    fn point_sequence_skips_exact_zeros() {
        let value = polynomial(&[0, -1, 1]);
        let (points, evaluations) = kronecker_points(&value, 1).expect("point search");
        assert_eq!(points.iter().map(integer).collect::<Vec<_>>(), [-1, 2]);
        assert_eq!(evaluations.iter().map(integer).collect::<Vec<_>>(), [2, 2]);
    }

    #[test]
    fn divisors_are_complete_and_have_the_required_order() {
        let value = BigInt::try_from(12_i32).expect("small integer");
        let divisors = signed_divisors(&value).expect("divisors");
        assert_eq!(
            divisors.iter().map(integer).collect::<Vec<_>>(),
            [1, -1, 2, -2, 3, -3, 4, -4, 6, -6, 12, -12]
        );
        let square = BigInt::try_from(9_i32).expect("small square");
        assert_eq!(
            signed_divisors(&square)
                .expect("square divisors")
                .iter()
                .map(integer)
                .collect::<Vec<_>>(),
            [1, -1, 3, -3, 9, -9]
        );
    }

    #[test]
    fn cartesian_product_changes_the_last_digit_first() {
        let columns = [
            vec![BigInt::zero(), BigInt::zero()],
            vec![BigInt::zero(), BigInt::zero(), BigInt::zero()],
        ];
        let mut tuples = Vec::new();
        visit_cartesian(&columns, |digits| {
            tuples.push(digits.to_vec());
            Ok(())
        })
        .expect("cartesian product");
        assert_eq!(
            tuples,
            [
                vec![0, 0],
                vec![0, 1],
                vec![0, 2],
                vec![1, 0],
                vec![1, 1],
                vec![1, 2],
            ]
        );
    }

    #[test]
    fn lagrange_interpolation_preserves_every_evaluation_point() {
        let points = [BigInt::zero(), BigInt::try_from(1_i32).expect("point")];
        let columns = [
            vec![BigInt::try_from(1_i32).expect("value")],
            vec![BigInt::try_from(3_i32).expect("value")],
        ];
        let value = super::interpolate_integer_candidate(&points, &columns, &[0, 0])
            .expect("interpolation")
            .expect("integer coefficients");
        assert_eq!(value, polynomial(&[1, 2]));
    }

    #[test]
    fn lagrange_interpolation_rejects_noninteger_coefficients() {
        let points = [BigInt::zero(), BigInt::try_from(2_i32).expect("point")];
        let columns = [
            vec![BigInt::zero()],
            vec![BigInt::try_from(1_i32).expect("value")],
        ];
        assert!(
            super::interpolate_integer_candidate(&points, &columns, &[0, 0])
                .expect("interpolation")
                .is_none()
        );
    }

    #[test]
    fn complete_enumeration_factors_the_required_quartic() {
        let square_free =
            crate::polynomial::SquareFreePolynomial::from_polynomial(polynomial(&[6, 0, -5, 0, 1]));
        let factors = square_free.factor().expect("complete factorization");
        assert_eq!(factors.len(), 2);
        assert_eq!(factors[0].polynomial(), &polynomial(&[-3, 0, 1]));
        assert_eq!(factors[1].polynomial(), &polynomial(&[-2, 0, 1]));

        let irreducible =
            crate::polynomial::SquareFreePolynomial::from_polynomial(polynomial(&[-2, 0, 1]));
        assert_eq!(irreducible.factor().expect("irreducible").len(), 1);
    }

    #[test]
    fn final_order_distinguishes_non_monic_linear_factors() {
        let square_free =
            crate::polynomial::SquareFreePolynomial::from_polynomial(polynomial(&[2, 5, 2]));
        let factors = square_free.factor().expect("complete factorization");
        assert_eq!(factors.len(), 2);
        assert_eq!(factors[0].polynomial(), &polynomial(&[2, 1]));
        assert_eq!(factors[1].polynomial(), &polynomial(&[1, 2]));
    }

    #[test]
    fn candidate_selection_considers_later_lexicographic_minimum() {
        let selected = select_minimum_candidate(vec![polynomial(&[2, 1]), polynomial(&[1, 1])])
            .expect("candidate");
        assert_eq!(selected, polynomial(&[1, 1]));
    }

    #[test]
    fn candidate_degree_rejects_zero_and_the_first_excess_degree() {
        assert!(!candidate_degree_allowed(0, 1));
        assert!(candidate_degree_allowed(1, 1));
        assert!(!candidate_degree_allowed(2, 1));
    }
}
