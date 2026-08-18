use alloc::vec::Vec;

use neco_bigint::{BigInt, BigUint};

use crate::error::{
    reserve_elements_at, AlgnumError, AllocationContact, AllocationResource, RepresentationResource,
};
use crate::polynomial::Polynomial;

pub(crate) fn resultant(
    left: &[Polynomial],
    right: &[Polynomial],
) -> Result<Polynomial, AlgnumError> {
    let left_len = normalized_len(left);
    let right_len = normalized_len(right);
    if left_len == 0 || right_len == 0 {
        return Ok(Polynomial::zero());
    }
    let left_degree = left_len - 1;
    let right_degree = right_len - 1;
    if left_degree == 0 && right_degree == 0 {
        return resultant_one();
    }
    if left_degree == 0 {
        return pow_resultant(&left[0], right_degree);
    }
    if right_degree == 0 {
        return pow_resultant(&right[0], left_degree);
    }

    let dimension = left_degree
        .checked_add(right_degree)
        .ok_or_else(|| sylvester_dimension_overflow(left_degree, right_degree))?;
    let element_count = dimension
        .checked_mul(dimension)
        .ok_or_else(|| sylvester_element_overflow(dimension))?;
    let mut matrix = Vec::new();
    reserve_elements_at(
        &mut matrix,
        element_count,
        AllocationResource::SylvesterElements,
        AllocationContact::SylvesterMatrix,
    )?;
    for row in 0..right_degree {
        append_sylvester_row(&mut matrix, dimension, row, &left[..left_len])?;
    }
    for row in 0..left_degree {
        append_sylvester_row(&mut matrix, dimension, row, &right[..right_len])?;
    }
    determinant(&matrix, dimension)
}

fn append_sylvester_row(
    matrix: &mut Vec<Polynomial>,
    dimension: usize,
    shift: usize,
    coefficients: &[Polynomial],
) -> Result<(), AlgnumError> {
    for column in 0..dimension {
        let value = if column >= shift && column - shift < coefficients.len() {
            let source = coefficients.len() - 1 - (column - shift);
            clone_resultant(&coefficients[source])?
        } else {
            Polynomial::zero()
        };
        matrix.push(value);
    }
    Ok(())
}

fn determinant(matrix: &[Polynomial], dimension: usize) -> Result<Polynomial, AlgnumError> {
    if dimension == 0 {
        return resultant_one();
    }
    let mut permutation = Vec::new();
    reserve_elements_at(
        &mut permutation,
        dimension,
        AllocationResource::Permutation,
        AllocationContact::DeterminantPermutation,
    )?;
    for index in 0..dimension {
        permutation.push(index);
    }
    let sum_count = determinant_sum_coefficient_count(matrix, dimension, &mut permutation)?;
    let mut sum = Vec::new();
    reserve_elements_at(
        &mut sum,
        sum_count,
        AllocationResource::ResultantCoefficients,
        AllocationContact::DeterminantSum,
    )?;
    for _ in 0..sum_count {
        sum.push(BigInt::zero());
    }
    loop {
        let product = determinant_term(matrix, dimension, &permutation)?;
        for (index, coefficient) in product.coefficients().iter().enumerate() {
            sum[index] = if inversion_is_odd(&permutation) {
                sum[index].sub(coefficient)?
            } else {
                sum[index].add(coefficient)?
            };
        }
        if !next_permutation(&mut permutation) {
            return Ok(Polynomial::from_coefficients(sum));
        }
    }
}

fn determinant_sum_coefficient_count(
    matrix: &[Polynomial],
    dimension: usize,
    permutation: &mut [usize],
) -> Result<usize, AlgnumError> {
    let mut maximum_degree = BigUint::zero();
    let mut found_nonzero_term = false;
    loop {
        let mut degree = BigUint::zero();
        let mut nonzero = true;
        for (row, column) in permutation.iter().enumerate() {
            let slot = row
                .checked_mul(dimension)
                .and_then(|base| base.checked_add(*column))
                .ok_or_else(|| sylvester_element_overflow(dimension))?;
            let Some(element_degree) = matrix[slot].degree() else {
                nonzero = false;
                break;
            };
            degree = degree.add(&BigUint::try_from(element_degree)?)?;
        }
        if nonzero {
            found_nonzero_term = true;
            if degree > maximum_degree {
                maximum_degree = degree;
            }
        }
        if !next_permutation(permutation) {
            break;
        }
    }
    for (index, value) in permutation.iter_mut().enumerate() {
        *value = index;
    }
    if found_nonzero_term {
        coefficient_count_from_degree(&maximum_degree)
    } else {
        Ok(0)
    }
}

fn determinant_term(
    matrix: &[Polynomial],
    dimension: usize,
    permutation: &[usize],
) -> Result<Polynomial, AlgnumError> {
    let mut degree = BigUint::zero();
    for (row, column) in permutation.iter().enumerate() {
        let slot = row
            .checked_mul(dimension)
            .and_then(|base| base.checked_add(*column))
            .ok_or_else(|| sylvester_element_overflow(dimension))?;
        if matrix[slot].is_zero() {
            return Ok(Polynomial::zero());
        }
        degree = degree.add(&BigUint::try_from(matrix[slot].degree().unwrap_or(0))?)?;
    }
    let count = coefficient_count_from_degree(&degree)?;
    let mut coefficients = Vec::new();
    reserve_elements_at(
        &mut coefficients,
        count,
        AllocationResource::ResultantCoefficients,
        AllocationContact::DeterminantProduct,
    )?;
    for target in 0..count {
        coefficients.push(determinant_term_coefficient(
            matrix,
            dimension,
            permutation,
            0,
            target,
        )?);
    }
    Ok(Polynomial::from_coefficients(coefficients))
}

fn determinant_term_coefficient(
    matrix: &[Polynomial],
    dimension: usize,
    permutation: &[usize],
    row: usize,
    target: usize,
) -> Result<BigInt, AlgnumError> {
    if row == dimension {
        return if target == 0 {
            Ok(BigInt::one()?)
        } else {
            Ok(BigInt::zero())
        };
    }
    let slot = row
        .checked_mul(dimension)
        .and_then(|base| base.checked_add(permutation[row]))
        .ok_or_else(|| sylvester_element_overflow(dimension))?;
    let polynomial = &matrix[slot];
    let mut sum = BigInt::zero();
    for (power, coefficient) in polynomial
        .coefficients()
        .iter()
        .enumerate()
        .take(target.saturating_add(1))
    {
        let suffix =
            determinant_term_coefficient(matrix, dimension, permutation, row + 1, target - power)?;
        sum = sum.add(&coefficient.mul(&suffix)?)?;
    }
    Ok(sum)
}

fn coefficient_count_from_degree(degree: &BigUint) -> Result<usize, AlgnumError> {
    let required = degree.add(&BigUint::one()?)?;
    let maximum = BigUint::try_from(usize::MAX)?;
    if required > maximum {
        return Err(AlgnumError::RepresentationLimit {
            resource: RepresentationResource::CoefficientCount,
            required,
            maximum,
        });
    }
    let mut count = 0_usize;
    for bit in 0..required.bit_len() {
        if required.bit(bit) {
            count |= 1_usize << bit;
        }
    }
    Ok(count)
}

fn next_permutation(values: &mut [usize]) -> bool {
    let Some(mut pivot) = values.len().checked_sub(2) else {
        return false;
    };
    loop {
        if values[pivot] < values[pivot + 1] {
            break;
        }
        if pivot == 0 {
            return false;
        }
        pivot -= 1;
    }
    let mut successor = values.len() - 1;
    while values[successor] <= values[pivot] {
        successor -= 1;
    }
    values.swap(pivot, successor);
    values[pivot + 1..].reverse();
    true
}

fn inversion_is_odd(permutation: &[usize]) -> bool {
    let mut odd = false;
    for left in 0..permutation.len() {
        for right in left + 1..permutation.len() {
            if permutation[left] > permutation[right] {
                odd = !odd;
            }
        }
    }
    odd
}

fn normalized_len(coefficients: &[Polynomial]) -> usize {
    coefficients
        .iter()
        .rposition(|coefficient| !coefficient.is_zero())
        .map_or(0, |index| index + 1)
}

fn pow_resultant(base: &Polynomial, exponent: usize) -> Result<Polynomial, AlgnumError> {
    let mut result = resultant_one()?;
    if exponent == 0 {
        return Ok(result);
    }
    let mut power = clone_resultant(base)?;
    let bits = usize::BITS as usize - exponent.leading_zeros() as usize;
    for bit in 0..bits {
        if exponent & (1_usize << bit) != 0 {
            result = mul_resultant(&result, &power)?;
        }
        if bit + 1 < bits {
            power = mul_resultant(&power, &power)?;
        }
    }
    Ok(result)
}

fn resultant_one() -> Result<Polynomial, AlgnumError> {
    let mut coefficients = Vec::new();
    reserve_elements_at(
        &mut coefficients,
        1,
        AllocationResource::ResultantCoefficients,
        AllocationContact::ResultantOne,
    )?;
    coefficients.push(BigInt::one()?);
    Ok(Polynomial::from_coefficients(coefficients))
}

fn clone_resultant(value: &Polynomial) -> Result<Polynomial, AlgnumError> {
    let mut coefficients = Vec::new();
    reserve_elements_at(
        &mut coefficients,
        value.coefficients().len(),
        AllocationResource::ResultantCoefficients,
        AllocationContact::ResultantClone,
    )?;
    for coefficient in value.coefficients() {
        coefficients.push(coefficient.try_clone()?);
    }
    Ok(Polynomial::from_coefficients(coefficients))
}

fn mul_resultant(left: &Polynomial, right: &Polynomial) -> Result<Polynomial, AlgnumError> {
    if left.is_zero() || right.is_zero() {
        return Ok(Polynomial::zero());
    }
    let count = left
        .coefficients()
        .len()
        .checked_add(right.coefficients().len())
        .and_then(|sum| sum.checked_sub(1))
        .ok_or_else(|| {
            coefficient_count_overflow(left.coefficients().len(), right.coefficients().len())
        })?;
    let mut coefficients = Vec::new();
    reserve_elements_at(
        &mut coefficients,
        count,
        AllocationResource::ResultantCoefficients,
        AllocationContact::ResultantMul,
    )?;
    for _ in 0..count {
        coefficients.push(BigInt::zero());
    }
    for (left_index, left_value) in left.coefficients().iter().enumerate() {
        for (right_index, right_value) in right.coefficients().iter().enumerate() {
            let slot = left_index + right_index;
            let product = left_value.mul(right_value)?;
            coefficients[slot] = coefficients[slot].add(&product)?;
        }
    }
    Ok(Polynomial::from_coefficients(coefficients))
}

fn sylvester_dimension_overflow(left: usize, right: usize) -> AlgnumError {
    binary_overflow(
        RepresentationResource::SylvesterDimension,
        left,
        right,
        usize::MAX,
        false,
    )
}

fn sylvester_element_overflow(dimension: usize) -> AlgnumError {
    binary_overflow(
        RepresentationResource::SylvesterElementCount,
        dimension,
        dimension,
        usize::MAX,
        true,
    )
}

fn coefficient_count_overflow(left_count: usize, right_count: usize) -> AlgnumError {
    let left = left_count.saturating_sub(1);
    binary_overflow(
        RepresentationResource::CoefficientCount,
        left,
        right_count,
        usize::MAX,
        false,
    )
}

fn binary_overflow(
    resource: RepresentationResource,
    left: usize,
    right: usize,
    maximum: usize,
    multiply: bool,
) -> AlgnumError {
    let left = match BigUint::try_from(left) {
        Ok(value) => value,
        Err(error) => return error.into(),
    };
    let right = match BigUint::try_from(right) {
        Ok(value) => value,
        Err(error) => return error.into(),
    };
    let required = match if multiply {
        left.mul(&right)
    } else {
        left.add(&right)
    } {
        Ok(value) => value,
        Err(error) => return error.into(),
    };
    let maximum = match BigUint::try_from(maximum) {
        Ok(value) => value,
        Err(error) => return error.into(),
    };
    AlgnumError::RepresentationLimit {
        resource,
        required,
        maximum,
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use neco_bigint::BigInt;

    use super::{
        coefficient_count_overflow, determinant, determinant_sum_coefficient_count,
        determinant_term, next_permutation, resultant, sylvester_dimension_overflow,
        sylvester_element_overflow,
    };
    use crate::error::{with_injected_failure, AllocationContact};
    use crate::polynomial::Polynomial;
    use crate::{AlgnumError, AllocationResource, RepresentationResource};

    fn polynomial(coefficients: &[i32]) -> Polynomial {
        Polynomial::from_coefficients(
            coefficients
                .iter()
                .map(|value| BigInt::try_from(*value).expect("small coefficient"))
                .collect(),
        )
    }

    fn constant(value: i32) -> Polynomial {
        polynomial(&[value])
    }

    #[test]
    fn sylvester_vectors_have_the_required_values() {
        let first = resultant(
            &[constant(-2), Polynomial::zero(), constant(1)],
            &[constant(-1), constant(1)],
        )
        .expect("resultant");
        assert_eq!(first, constant(-1));

        let shifted = resultant(
            &[constant(1), Polynomial::zero(), constant(1)],
            &[constant(1), constant(1)],
        )
        .expect("resultant");
        assert_eq!(shifted, constant(2));

        let symbolic = resultant(
            &[polynomial(&[0, -1]), constant(1)],
            &[constant(-1), constant(1)],
        )
        .expect("symbolic resultant");
        assert_eq!(symbolic, polynomial(&[-1, 1]));
    }

    #[test]
    fn leibniz_formula_visits_all_six_terms() {
        let matrix = [1, 2, 3, 4, 5, 6, 7, 8, 10]
            .map(constant)
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(determinant(&matrix, 3).expect("determinant"), constant(-3));

        let mut permutation = vec![0, 1, 2];
        for expected in [
            vec![0, 2, 1],
            vec![1, 0, 2],
            vec![1, 2, 0],
            vec![2, 0, 1],
            vec![2, 1, 0],
        ] {
            assert!(next_permutation(&mut permutation));
            assert_eq!(permutation, expected);
        }
        assert!(!next_permutation(&mut permutation));
    }

    #[test]
    fn constant_and_zero_inputs_follow_the_defined_branches() {
        let positive_degree = [constant(1), constant(1)];
        assert_eq!(
            resultant(&[constant(3)], &positive_degree).expect("left constant"),
            constant(3)
        );
        assert_eq!(
            resultant(&positive_degree, &[constant(3)]).expect("right constant"),
            constant(3)
        );
        assert_eq!(
            resultant(&[constant(2)], &[constant(3)]).expect("two constants"),
            constant(1)
        );
        assert!(resultant(&[], &positive_degree)
            .expect("zero polynomial")
            .is_zero());
    }

    #[test]
    fn representation_overflow_payloads_preserve_the_exact_required_value() {
        let maximum = neco_bigint::BigUint::try_from(usize::MAX).unwrap();
        let twice = maximum.add(&maximum).unwrap();
        assert_eq!(
            sylvester_dimension_overflow(usize::MAX, usize::MAX),
            AlgnumError::RepresentationLimit {
                resource: RepresentationResource::SylvesterDimension,
                required: twice.try_clone().unwrap(),
                maximum: maximum.try_clone().unwrap(),
            }
        );
        assert_eq!(
            sylvester_element_overflow(usize::MAX),
            AlgnumError::RepresentationLimit {
                resource: RepresentationResource::SylvesterElementCount,
                required: maximum.mul(&maximum).unwrap(),
                maximum: maximum.try_clone().unwrap(),
            }
        );
        assert_eq!(
            coefficient_count_overflow(usize::MAX, usize::MAX),
            AlgnumError::RepresentationLimit {
                resource: RepresentationResource::CoefficientCount,
                required: twice
                    .checked_sub(&neco_bigint::BigUint::one().unwrap())
                    .unwrap(),
                maximum,
            }
        );
    }

    #[test]
    fn determinant_storage_counts_full_terms_and_sum_before_accumulation() {
        let matrix = vec![
            polynomial(&[1, 1]),
            polynomial(&[1, 1]),
            polynomial(&[1, 1]),
            polynomial(&[1, 1]),
        ];
        assert_eq!(
            determinant_sum_coefficient_count(&matrix, 2, &mut [0, 1]).unwrap(),
            3
        );
        assert_eq!(
            determinant_term(&matrix, 2, &[0, 1])
                .unwrap()
                .coefficients()
                .len(),
            3
        );

        let constrained = vec![
            polynomial(&[1, 1]),
            polynomial(&[1]),
            polynomial(&[1, 1]),
            polynomial(&[1]),
        ];
        assert_eq!(
            determinant_sum_coefficient_count(&constrained, 2, &mut [0, 1]).unwrap(),
            2
        );

        let later_maximum = vec![
            polynomial(&[1]),
            polynomial(&[0, 0, 1]),
            polynomial(&[0, 0, 0, 1]),
            polynomial(&[1]),
        ];
        let mut permutation = [0, 1];
        assert_eq!(
            determinant_sum_coefficient_count(&later_maximum, 2, &mut permutation).unwrap(),
            6
        );
        assert_eq!(permutation, [0, 1]);
        assert_eq!(
            determinant(&later_maximum, 2).unwrap(),
            polynomial(&[1, 0, 0, 0, 0, -1])
        );

        let zero_matrix = vec![
            Polynomial::zero(),
            Polynomial::zero(),
            Polynomial::zero(),
            Polynomial::zero(),
        ];
        assert!(determinant(&zero_matrix, 2).unwrap().is_zero());
        assert!(
            with_injected_failure(AllocationContact::DeterminantSum, || {
                determinant(&zero_matrix, 2)
            })
            .unwrap()
            .is_zero()
        );
        assert_eq!(
            with_injected_failure(AllocationContact::DeterminantSum, || {
                determinant(&matrix, 2)
            }),
            Err(AlgnumError::AllocationFailure {
                resource: AllocationResource::ResultantCoefficients,
                requested: 3,
            })
        );
        assert_eq!(
            with_injected_failure(AllocationContact::DeterminantProduct, || {
                determinant_term(&matrix, 2, &[0, 1])
            }),
            Err(AlgnumError::AllocationFailure {
                resource: AllocationResource::ResultantCoefficients,
                requested: 3,
            })
        );
    }
}
