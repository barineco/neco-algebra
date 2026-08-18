use alloc::vec::Vec;

use neco_bigint::{BigInt, BigUint, RawRational, ReducedRational, Sign};
use neco_monomial::{ProvenPrime, RadicalBasis};

use crate::error::{reserve_elements, AllocationTarget, DimensionResource, FormSumErrorKind};
use crate::formsum::FormSum;

#[derive(Debug, Eq, PartialEq)]
pub struct RadicalExtension {
    primes: Vec<ProvenPrime>,
    denominators: Vec<BigUint>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct RadicalCoordinates {
    extension: RadicalExtension,
    coefficients: Vec<ReducedRational>,
}

impl RadicalExtension {
    pub fn primes(&self) -> &[ProvenPrime] {
        &self.primes
    }

    pub fn denominators(&self) -> &[BigUint] {
        &self.denominators
    }

    pub fn basis_count(&self) -> usize {
        self.denominators.iter().fold(1_usize, |product, value| {
            product * validated_uint_to_usize(value)
        })
    }

    pub fn try_clone(&self) -> Result<Self, FormSumErrorKind> {
        let mut primes = Vec::new();
        let mut denominators = Vec::new();
        reserve_elements(
            &mut primes,
            self.primes.len(),
            DimensionResource::Denominator,
            AllocationTarget::ExtensionPrimes,
        )?;
        reserve_elements(
            &mut denominators,
            self.denominators.len(),
            DimensionResource::Denominator,
            AllocationTarget::ExtensionDenominators,
        )?;
        for prime in &self.primes {
            primes.push(prime.try_clone()?);
        }
        for denominator in &self.denominators {
            denominators.push(denominator.try_clone()?);
        }
        Ok(Self {
            primes,
            denominators,
        })
    }

    pub(crate) fn from_form_sums(values: &[&FormSum]) -> Result<Self, FormSumErrorKind> {
        let count = count_distinct_primes(values)?;
        let mut primes = Vec::new();
        let mut denominators = Vec::new();
        reserve_elements(
            &mut primes,
            count,
            DimensionResource::Denominator,
            AllocationTarget::ExtensionPrimes,
        )?;
        reserve_elements(
            &mut denominators,
            count,
            DimensionResource::Denominator,
            AllocationTarget::ExtensionDenominators,
        )?;

        let mut previous: Option<BigUint> = None;
        loop {
            let next = next_prime(values, previous.as_ref())?;
            let Some(next) = next else { break };
            let mut denominator = BigUint::one()?;
            let mut witness: Option<ProvenPrime> = None;
            for value in values {
                for (basis, _) in value.terms() {
                    for (prime, exponent) in basis.factors() {
                        if prime.value() == &next {
                            denominator = denominator.lcm(exponent.denominator())?;
                            if witness.is_none() {
                                witness = Some(prime.try_clone()?);
                            }
                        }
                    }
                }
            }
            let prime = witness.ok_or_else(basis_overflow)?;
            primes.push(prime);
            denominators.push(denominator);
            previous = Some(next);
        }
        Self::from_parts(primes, denominators)
    }

    fn union_with_form_sum(&self, value: &FormSum) -> Result<Self, FormSumErrorKind> {
        let own = self.as_unit_form_sum()?;
        Self::from_form_sums(&[&own, value])
    }

    fn as_unit_form_sum(&self) -> Result<FormSum, FormSumErrorKind> {
        let mut factors = Vec::new();
        reserve_elements(
            &mut factors,
            self.primes.len(),
            DimensionResource::Denominator,
            AllocationTarget::ExtensionFactors,
        )?;
        for (prime, denominator) in self.primes.iter().zip(&self.denominators) {
            factors.push((
                prime.try_clone()?,
                rational_parts(1, denominator.try_clone()?)?,
            ));
        }
        let basis = RadicalBasis::try_from_sorted_factors(factors)?;
        let mut terms = Vec::new();
        reserve_elements(
            &mut terms,
            1,
            DimensionResource::BasisCount,
            AllocationTarget::CoordinateTerms,
        )?;
        terms.push((basis, rational_integer(1)?));
        Ok(FormSum::from_sorted_terms(terms))
    }

    fn from_parts(
        primes: Vec<ProvenPrime>,
        denominators: Vec<BigUint>,
    ) -> Result<Self, FormSumErrorKind> {
        let mut exact = BigUint::one()?;
        let mut basis_count = 1_usize;
        for denominator in &denominators {
            exact = exact.mul(denominator)?;
            let factor = uint_to_usize(denominator).ok_or_else(|| {
                dimension_overflow(DimensionResource::Denominator, denominator, usize::MAX)
            })?;
            basis_count = basis_count.checked_mul(factor).ok_or_else(|| {
                dimension_overflow(DimensionResource::BasisCount, &exact, usize::MAX)
            })?;
        }
        Ok(Self {
            primes,
            denominators,
        })
    }

    fn digit_at(&self, index: usize, position: usize) -> Result<usize, FormSumErrorKind> {
        let radix = uint_to_usize(&self.denominators[position]).ok_or_else(|| {
            dimension_overflow(
                DimensionResource::Denominator,
                &self.denominators[position],
                usize::MAX,
            )
        })?;
        let mut stride = 1_usize;
        for denominator in &self.denominators[position + 1..] {
            stride = stride
                .checked_mul(uint_to_usize(denominator).ok_or_else(basis_overflow)?)
                .ok_or_else(basis_overflow)?;
        }
        Ok((index / stride) % radix)
    }
}

impl RadicalCoordinates {
    pub fn extension(&self) -> &RadicalExtension {
        &self.extension
    }

    pub fn coefficients(&self) -> &[ReducedRational] {
        &self.coefficients
    }

    pub fn try_clone(&self) -> Result<Self, FormSumErrorKind> {
        let mut coefficients = Vec::new();
        reserve_elements(
            &mut coefficients,
            self.coefficients.len(),
            DimensionResource::BasisCount,
            AllocationTarget::CoordinateValues,
        )?;
        for coefficient in &self.coefficients {
            coefficients.push(coefficient.try_clone()?);
        }
        Ok(Self {
            extension: self.extension.try_clone()?,
            coefficients,
        })
    }

    pub fn multiplication_matrix(&self) -> Result<Vec<ReducedRational>, FormSumErrorKind> {
        self.multiplication_matrix_for(AllocationTarget::MultiplicationMatrix)
    }

    pub(crate) fn multiplication_matrix_for(
        &self,
        target: AllocationTarget,
    ) -> Result<Vec<ReducedRational>, FormSumErrorKind> {
        let dimension = self.extension.basis_count();
        let element_count = crate::error::checked_square_dimension(dimension)?;
        let mut matrix = Vec::new();
        reserve_elements(
            &mut matrix,
            element_count,
            DimensionResource::MatrixElementCount,
            target,
        )?;
        for _ in 0..element_count {
            matrix.push(rational_integer(0)?);
        }
        for column in 0..dimension {
            for (value_index, value_coefficient) in self.coefficients.iter().enumerate() {
                if value_coefficient.is_zero() {
                    continue;
                }
                let mut coefficient = value_coefficient.try_clone()?;
                let mut row = 0_usize;
                for position in 0..self.extension.primes.len() {
                    let radix = uint_to_usize(&self.extension.denominators[position])
                        .ok_or_else(basis_overflow)?;
                    let sum = self
                        .extension
                        .digit_at(value_index, position)?
                        .checked_add(self.extension.digit_at(column, position)?)
                        .ok_or_else(basis_overflow)?;
                    row = row
                        .checked_mul(radix)
                        .and_then(|value| value.checked_add(sum % radix))
                        .ok_or_else(basis_overflow)?;
                    if sum >= radix {
                        coefficient = coefficient.mul(&rational_biguint(
                            self.extension.primes[position].value().try_clone()?,
                        )?)?;
                    }
                }
                let slot = row
                    .checked_add(dimension.checked_mul(column).ok_or_else(matrix_overflow)?)
                    .ok_or_else(matrix_overflow)?;
                matrix[slot] = matrix[slot].add(&coefficient)?;
            }
        }
        Ok(matrix)
    }

    pub fn into_form_sum(self) -> Result<FormSum, FormSumErrorKind> {
        let count = self
            .coefficients
            .iter()
            .filter(|coefficient| !coefficient.is_zero())
            .count();
        let mut terms = Vec::new();
        reserve_elements(
            &mut terms,
            count,
            DimensionResource::BasisCount,
            AllocationTarget::CoordinateTerms,
        )?;
        for (index, coefficient) in self.coefficients.into_iter().enumerate() {
            if coefficient.is_zero() {
                continue;
            }
            let mut factors = Vec::new();
            let mut factor_count = 0_usize;
            for position in 0..self.extension.primes.len() {
                if self.extension.digit_at(index, position)? != 0 {
                    factor_count = factor_count.checked_add(1).ok_or_else(basis_overflow)?;
                }
            }
            reserve_elements(
                &mut factors,
                factor_count,
                DimensionResource::BasisCount,
                AllocationTarget::CoordinateFactors,
            )?;
            for position in 0..self.extension.primes.len() {
                let digit = self.extension.digit_at(index, position)?;
                if digit == 0 {
                    continue;
                }
                factors.push((
                    self.extension.primes[position].try_clone()?,
                    rational_parts(digit, self.extension.denominators[position].try_clone()?)?,
                ));
            }
            terms.push((RadicalBasis::try_from_sorted_factors(factors)?, coefficient));
        }
        Ok(FormSum::from_sorted_terms(terms))
    }

    pub(crate) fn solve_unit(&self) -> Result<Self, FormSumErrorKind> {
        let dimension = self.extension.basis_count();
        let mut matrix = self.multiplication_matrix_for(AllocationTarget::GaussianMatrix)?;
        let mut rhs = Vec::new();
        reserve_elements(
            &mut rhs,
            dimension,
            DimensionResource::BasisCount,
            AllocationTarget::GaussianRightHandSide,
        )?;
        for row in 0..dimension {
            rhs.push(rational_integer(if row == 0 { 1 } else { 0 })?);
        }
        gaussian_solve(&mut matrix, &mut rhs, dimension)?;
        Ok(Self {
            extension: self.extension.try_clone()?,
            coefficients: rhs,
        })
    }
}

impl FormSum {
    pub fn extension_with(&self, rhs: &Self) -> Result<RadicalExtension, FormSumErrorKind> {
        RadicalExtension::from_form_sums(&[self, rhs])
    }

    pub fn coordinates_with(
        &self,
        extension: &RadicalExtension,
    ) -> Result<RadicalCoordinates, FormSumErrorKind> {
        let extension = extension.union_with_form_sum(self)?;
        let mut coefficients = Vec::new();
        reserve_elements(
            &mut coefficients,
            extension.basis_count(),
            DimensionResource::BasisCount,
            AllocationTarget::CoordinateValues,
        )?;
        for _ in 0..extension.basis_count() {
            coefficients.push(rational_integer(0)?);
        }
        for (basis, coefficient) in self.terms() {
            let mut index = 0_usize;
            let mut factor_index = 0_usize;
            for position in 0..extension.primes.len() {
                let radix =
                    uint_to_usize(&extension.denominators[position]).ok_or_else(basis_overflow)?;
                let mut digit = 0_usize;
                if let Some((prime, exponent)) = basis.factors().get(factor_index) {
                    if prime.value() == extension.primes[position].value() {
                        let scale =
                            extension.denominators[position].exact_div(exponent.denominator())?;
                        let exact_digit = exponent.numerator().magnitude().mul(&scale)?;
                        digit = uint_to_usize(&exact_digit).ok_or_else(basis_overflow)?;
                        factor_index += 1;
                    }
                }
                index = index
                    .checked_mul(radix)
                    .and_then(|value| value.checked_add(digit))
                    .ok_or_else(basis_overflow)?;
            }
            coefficients[index] = coefficients[index].add(coefficient)?;
        }
        Ok(RadicalCoordinates {
            extension,
            coefficients,
        })
    }

    pub fn inverse(&self) -> Result<Self, FormSumErrorKind> {
        if self.is_zero() {
            return Err(FormSumErrorKind::DivisionByZero);
        }
        let extension = RadicalExtension::from_form_sums(&[self])?;
        self.coordinates_with(&extension)?
            .solve_unit()?
            .into_form_sum()
    }

    pub fn div(&self, rhs: &Self) -> Result<Self, FormSumErrorKind> {
        self.mul(&rhs.inverse()?)
    }
}

fn gaussian_solve(
    matrix: &mut [ReducedRational],
    rhs: &mut [ReducedRational],
    dimension: usize,
) -> Result<(), FormSumErrorKind> {
    for pivot_column in 0..dimension {
        let pivot_row = (pivot_column..dimension)
            .find(|row| !matrix[*row + dimension * pivot_column].is_zero())
            .ok_or_else(basis_overflow)?;
        #[cfg(test)]
        PIVOT_SELECTIONS.with(|selections| {
            selections.borrow_mut().push((pivot_column, pivot_row));
        });
        if pivot_row != pivot_column {
            for column in 0..dimension {
                matrix.swap(
                    pivot_column + dimension * column,
                    pivot_row + dimension * column,
                );
            }
            rhs.swap(pivot_column, pivot_row);
        }
        let pivot = matrix[pivot_column + dimension * pivot_column].try_clone()?;
        for column in pivot_column..dimension {
            let slot = pivot_column + dimension * column;
            matrix[slot] = matrix[slot].div(&pivot)?;
        }
        rhs[pivot_column] = rhs[pivot_column].div(&pivot)?;
        for row in 0..dimension {
            if row == pivot_column {
                continue;
            }
            let factor = matrix[row + dimension * pivot_column].try_clone()?;
            if factor.is_zero() {
                continue;
            }
            for column in pivot_column..dimension {
                let slot = row + dimension * column;
                let product = factor.mul(&matrix[pivot_column + dimension * column])?;
                matrix[slot] = matrix[slot].sub(&product)?;
            }
            let product = factor.mul(&rhs[pivot_column])?;
            rhs[row] = rhs[row].sub(&product)?;
        }
    }
    Ok(())
}

fn count_distinct_primes(values: &[&FormSum]) -> Result<usize, FormSumErrorKind> {
    let mut count = 0_usize;
    let mut previous: Option<&BigUint> = None;
    loop {
        let mut next: Option<&BigUint> = None;
        for value in values {
            for (basis, _) in value.terms() {
                for (prime, _) in basis.factors() {
                    if previous.is_none_or(|old| prime.value() > old)
                        && next.is_none_or(|current| prime.value() < current)
                    {
                        next = Some(prime.value());
                    }
                }
            }
        }
        let Some(found) = next else { break };
        count = count.checked_add(1).ok_or_else(basis_overflow)?;
        previous = Some(found);
    }
    Ok(count)
}

fn next_prime(
    values: &[&FormSum],
    previous: Option<&BigUint>,
) -> Result<Option<BigUint>, FormSumErrorKind> {
    let mut next: Option<&BigUint> = None;
    for value in values {
        for (basis, _) in value.terms() {
            for (prime, _) in basis.factors() {
                if previous.is_none_or(|old| prime.value() > old)
                    && next.is_none_or(|current| prime.value() < current)
                {
                    next = Some(prime.value());
                }
            }
        }
    }
    next.map(BigUint::try_clone).transpose().map_err(Into::into)
}

fn uint_to_usize(value: &BigUint) -> Option<usize> {
    if value.bit_len() > usize::BITS as usize {
        return None;
    }
    let mut result = 0_usize;
    for (index, limb) in value.limbs_le().iter().enumerate() {
        result |= (*limb as usize) << (32 * index);
    }
    Some(result)
}

fn validated_uint_to_usize(value: &BigUint) -> usize {
    value
        .limbs_le()
        .iter()
        .enumerate()
        .fold(0_usize, |result, (index, limb)| {
            result | ((*limb as usize) << (32 * index))
        })
}

fn rational_integer(value: i32) -> Result<ReducedRational, FormSumErrorKind> {
    Ok(ReducedRational::from_bigint(BigInt::try_from(value)?)?)
}

fn rational_biguint(value: BigUint) -> Result<ReducedRational, FormSumErrorKind> {
    Ok(ReducedRational::from_bigint(BigInt::from_sign_magnitude(
        Sign::Positive,
        value,
    ))?)
}

fn rational_parts(
    numerator: usize,
    denominator: BigUint,
) -> Result<ReducedRational, FormSumErrorKind> {
    Ok(RawRational::new(BigInt::try_from(numerator)?, denominator)
        .reduce()?
        .into_reduced())
}

fn dimension_overflow(
    resource: DimensionResource,
    required: &BigUint,
    maximum: usize,
) -> FormSumErrorKind {
    match (required.try_clone(), BigUint::try_from(maximum)) {
        (Ok(required), Ok(maximum)) => FormSumErrorKind::DimensionOverflow {
            resource,
            required,
            maximum,
        },
        (Err(error), _) | (_, Err(error)) => FormSumErrorKind::Bigint(error),
    }
}

fn basis_overflow() -> FormSumErrorKind {
    let maximum = match BigUint::try_from(usize::MAX) {
        Ok(value) => value,
        Err(error) => return FormSumErrorKind::Bigint(error),
    };
    let one = match BigUint::one() {
        Ok(value) => value,
        Err(error) => return FormSumErrorKind::Bigint(error),
    };
    let required = match maximum.add(&one) {
        Ok(value) => value,
        Err(error) => return FormSumErrorKind::Bigint(error),
    };
    FormSumErrorKind::DimensionOverflow {
        resource: DimensionResource::BasisCount,
        required,
        maximum,
    }
}

fn matrix_overflow() -> FormSumErrorKind {
    let mut error = basis_overflow();
    if let FormSumErrorKind::DimensionOverflow { resource, .. } = &mut error {
        *resource = DimensionResource::MatrixElementCount;
    }
    error
}

#[cfg(test)]
std::thread_local! {
    static PIVOT_SELECTIONS: core::cell::RefCell<Vec<(usize, usize)>> = const { core::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use neco_bigint::BigUint;

    use super::{basis_overflow, gaussian_solve, rational_integer, PIVOT_SELECTIONS};
    use crate::{DimensionResource, FormSumErrorKind};

    #[test]
    fn basis_overflow_preserves_the_resource_and_payload() {
        let maximum = BigUint::try_from(usize::MAX).unwrap();
        let required = maximum.add(&BigUint::one().unwrap()).unwrap();
        assert_eq!(
            basis_overflow(),
            FormSumErrorKind::DimensionOverflow {
                resource: DimensionResource::BasisCount,
                required,
                maximum,
            }
        );
    }

    #[test]
    fn gaussian_elimination_selects_the_first_nonzero_pivot_row() {
        let mut matrix = [
            rational_integer(0).unwrap(),
            rational_integer(1).unwrap(),
            rational_integer(1).unwrap(),
            rational_integer(1).unwrap(),
            rational_integer(0).unwrap(),
            rational_integer(0).unwrap(),
            rational_integer(0).unwrap(),
            rational_integer(0).unwrap(),
            rational_integer(1).unwrap(),
        ];
        let mut rhs = [
            rational_integer(1).unwrap(),
            rational_integer(0).unwrap(),
            rational_integer(0).unwrap(),
        ];
        PIVOT_SELECTIONS.with(|selections| selections.borrow_mut().clear());
        gaussian_solve(&mut matrix, &mut rhs, 3).unwrap();
        assert_eq!(
            PIVOT_SELECTIONS.with(|selections| selections.borrow().clone()),
            vec![(0, 1), (1, 1), (2, 2)]
        );
    }
}
