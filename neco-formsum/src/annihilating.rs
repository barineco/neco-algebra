use alloc::vec::Vec;

use neco_bigint::{BigInt, BigUint, RawRational, ReducedRational, Sign};

use crate::error::{
    checked_square_dimension, reserve_elements, AllocationTarget, DimensionResource,
    FormSumErrorKind,
};
use crate::extension::{RadicalCoordinates, RadicalExtension};
use crate::formsum::FormSum;

#[derive(Debug, Eq, PartialEq)]
pub struct AnnihilatingCoefficients {
    coefficients: Vec<BigInt>,
}

impl AnnihilatingCoefficients {
    pub fn coefficients(&self) -> &[BigInt] {
        &self.coefficients
    }

    pub fn try_clone(&self) -> Result<Self, FormSumErrorKind> {
        let mut coefficients = Vec::new();
        reserve_elements(
            &mut coefficients,
            self.coefficients.len(),
            DimensionResource::BasisCount,
            AllocationTarget::AnnihilatingCoefficients,
        )?;
        for coefficient in &self.coefficients {
            coefficients.push(coefficient.try_clone()?);
        }
        Ok(Self { coefficients })
    }
}

impl RadicalCoordinates {
    pub fn annihilating_coefficients(&self) -> Result<AnnihilatingCoefficients, FormSumErrorKind> {
        let dimension = self.extension().basis_count();
        let matrix = self.multiplication_matrix_for(AllocationTarget::AnnihilatingInputMatrix)?;
        let elements = checked_square_dimension(dimension)?;
        let coefficient_count = dimension.checked_add(1).ok_or_else(basis_overflow)?;

        let mut recurrence = Vec::new();
        reserve_elements(
            &mut recurrence,
            coefficient_count,
            DimensionResource::BasisCount,
            AllocationTarget::RecurrenceCoefficients,
        )?;
        recurrence.push(rational_integer(1)?);

        let mut b = zero_matrix(elements, AllocationTarget::RecurrenceStateMatrix)?;
        for index in 0..dimension {
            b[index + dimension * index] = rational_integer(1)?;
        }
        for k in 1..=dimension {
            let mut product = multiply_matrices(&matrix, &b, dimension)?;
            let mut trace = rational_integer(0)?;
            for index in 0..dimension {
                trace = trace.add(&product[index + dimension * index])?;
            }
            let divisor = rational_biguint(BigUint::try_from(k)?)?;
            let coefficient = negate(&trace.div(&divisor)?)?;
            for index in 0..dimension {
                let slot = index + dimension * index;
                product[slot] = product[slot].add(&coefficient)?;
            }
            recurrence.push(coefficient);
            b = product;
        }
        primitive_coefficients(&recurrence)
    }
}

impl FormSum {
    pub fn annihilating_coefficients(&self) -> Result<AnnihilatingCoefficients, FormSumErrorKind> {
        let extension = RadicalExtension::from_form_sums(&[self])?;
        self.coordinates_with(&extension)?
            .annihilating_coefficients()
    }
}

fn multiply_matrices(
    left: &[ReducedRational],
    right: &[ReducedRational],
    dimension: usize,
) -> Result<Vec<ReducedRational>, FormSumErrorKind> {
    let elements = checked_square_dimension(dimension)?;
    let mut result = zero_matrix(elements, AllocationTarget::RecurrenceProductMatrix)?;
    for column in 0..dimension {
        for row in 0..dimension {
            let mut value = rational_integer(0)?;
            for inner in 0..dimension {
                let term = left[row + dimension * inner].mul(&right[inner + dimension * column])?;
                value = value.add(&term)?;
            }
            result[row + dimension * column] = value;
        }
    }
    Ok(result)
}

fn zero_matrix(
    elements: usize,
    target: AllocationTarget,
) -> Result<Vec<ReducedRational>, FormSumErrorKind> {
    let mut matrix = Vec::new();
    reserve_elements(
        &mut matrix,
        elements,
        DimensionResource::MatrixElementCount,
        target,
    )?;
    for _ in 0..elements {
        matrix.push(rational_integer(0)?);
    }
    Ok(matrix)
}

fn primitive_coefficients(
    recurrence: &[ReducedRational],
) -> Result<AnnihilatingCoefficients, FormSumErrorKind> {
    let mut denominator_lcm = BigUint::one()?;
    for coefficient in recurrence {
        denominator_lcm = denominator_lcm.lcm(coefficient.denominator())?;
    }

    let mut scaled = Vec::new();
    reserve_elements(
        &mut scaled,
        recurrence.len(),
        DimensionResource::BasisCount,
        AllocationTarget::IntegerCoefficients,
    )?;
    for coefficient in recurrence.iter().rev() {
        let factor = denominator_lcm.exact_div(coefficient.denominator())?;
        scaled.push(multiply_int_uint(coefficient.numerator(), &factor)?);
    }

    let mut content = BigUint::zero();
    for coefficient in &scaled {
        if !coefficient.is_zero() {
            content = if content.is_zero() {
                coefficient.magnitude().try_clone()?
            } else {
                content.gcd(coefficient.magnitude())?
            };
        }
    }
    let one = BigUint::one()?;
    if content > one {
        for coefficient in &mut scaled {
            if coefficient.is_zero() {
                continue;
            }
            let magnitude = coefficient.magnitude().exact_div(&content)?;
            *coefficient = BigInt::from_sign_magnitude(coefficient.sign(), magnitude);
        }
    }
    if scaled
        .last()
        .is_some_and(|coefficient| coefficient.sign() == Sign::Negative)
    {
        for coefficient in &mut scaled {
            *coefficient = coefficient.negated()?;
        }
    }
    Ok(AnnihilatingCoefficients {
        coefficients: scaled,
    })
}

fn multiply_int_uint(left: &BigInt, right: &BigUint) -> Result<BigInt, FormSumErrorKind> {
    Ok(BigInt::from_sign_magnitude(
        left.sign(),
        left.magnitude().mul(right)?,
    ))
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
        Sign::Positive,
        value,
    ))?)
}

fn basis_overflow() -> FormSumErrorKind {
    overflow(DimensionResource::BasisCount)
}

fn overflow(resource: DimensionResource) -> FormSumErrorKind {
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
        resource,
        required,
        maximum,
    }
}
