use alloc::vec::Vec;

use neco_bigint::{BigInt, BigUint, BigintError, RawRational, ReducedRational, Sign};

use crate::error::{
    reserve_elements_at, AlgnumError, AllocationContact, AllocationResource, RepresentationResource,
};

#[derive(Debug, Eq, PartialEq)]
pub struct Polynomial {
    coefficients: Vec<BigInt>,
}

impl Polynomial {
    pub fn from_coefficients(mut coefficients: Vec<BigInt>) -> Self {
        trim_integer_coefficients(&mut coefficients);
        Self { coefficients }
    }

    pub fn zero() -> Self {
        Self {
            coefficients: Vec::new(),
        }
    }

    pub fn one() -> Result<Self, AlgnumError> {
        let mut coefficients = Vec::new();
        reserve_elements_at(
            &mut coefficients,
            1,
            AllocationResource::PolynomialCoefficients,
            AllocationContact::PolynomialOne,
        )?;
        coefficients.push(BigInt::one()?);
        Ok(Self { coefficients })
    }

    pub fn coefficients(&self) -> &[BigInt] {
        &self.coefficients
    }

    pub fn degree(&self) -> Option<usize> {
        self.coefficients.len().checked_sub(1)
    }

    pub fn is_zero(&self) -> bool {
        self.coefficients.is_empty()
    }

    pub fn try_clone(&self) -> Result<Self, AlgnumError> {
        let mut coefficients = Vec::new();
        reserve_elements_at(
            &mut coefficients,
            self.coefficients.len(),
            AllocationResource::PolynomialCoefficients,
            AllocationContact::PolynomialClone,
        )?;
        for coefficient in &self.coefficients {
            coefficients.push(coefficient.try_clone()?);
        }
        Ok(Self { coefficients })
    }

    pub fn add(&self, rhs: &Self) -> Result<Self, AlgnumError> {
        self.add_or_sub(rhs, false)
    }

    pub fn sub(&self, rhs: &Self) -> Result<Self, AlgnumError> {
        self.add_or_sub(rhs, true)
    }

    pub fn mul(&self, rhs: &Self) -> Result<Self, AlgnumError> {
        if self.is_zero() || rhs.is_zero() {
            return Ok(Self::zero());
        }
        let result_len =
            checked_product_coefficient_count(self.coefficients.len(), rhs.coefficients.len())?;
        let mut coefficients = Vec::new();
        reserve_elements_at(
            &mut coefficients,
            result_len,
            AllocationResource::PolynomialCoefficients,
            AllocationContact::PolynomialMul,
        )?;
        for _ in 0..result_len {
            coefficients.push(BigInt::zero());
        }
        for (left_index, left) in self.coefficients.iter().enumerate() {
            for (right_index, right) in rhs.coefficients.iter().enumerate() {
                let index = left_index + right_index;
                let product = left.mul(right)?;
                coefficients[index] = coefficients[index].add(&product)?;
            }
        }
        Ok(Self::from_coefficients(coefficients))
    }

    pub fn derivative(&self) -> Result<Self, AlgnumError> {
        let Some(degree) = self.degree() else {
            return Ok(Self::zero());
        };
        if degree == 0 {
            return Ok(Self::zero());
        }
        let mut coefficients = Vec::new();
        reserve_elements_at(
            &mut coefficients,
            degree,
            AllocationResource::PolynomialCoefficients,
            AllocationContact::PolynomialDerivative,
        )?;
        for (index, coefficient) in self.coefficients.iter().enumerate().skip(1) {
            let multiplier = BigInt::try_from(index)?;
            coefficients.push(coefficient.mul(&multiplier)?);
        }
        Ok(Self::from_coefficients(coefficients))
    }

    pub fn evaluate_bigint(&self, value: &BigInt) -> Result<BigInt, AlgnumError> {
        let mut result = BigInt::zero();
        for coefficient in self.coefficients.iter().rev() {
            result = result.mul(value)?;
            result = result.add(coefficient)?;
        }
        Ok(result)
    }

    pub fn evaluate_rational(
        &self,
        value: &ReducedRational,
    ) -> Result<ReducedRational, AlgnumError> {
        let mut result = rational_zero()?;
        for coefficient in self.coefficients.iter().rev() {
            result = result.mul(value)?;
            result = result.add(&ReducedRational::from_bigint(coefficient.try_clone()?)?)?;
        }
        Ok(result)
    }

    pub fn compose(&self, inner: &Self) -> Result<Self, AlgnumError> {
        let mut result = Self::zero();
        for coefficient in self.coefficients.iter().rev() {
            result = result.mul(inner)?;
            result = result.add(&Self::from_single_coefficient(coefficient.try_clone()?)?)?;
        }
        Ok(result)
    }

    pub fn candidate(self) -> Result<CandidatePolynomial, AlgnumError> {
        if self.degree().is_none_or(|degree| degree == 0) {
            return Err(AlgnumError::ZeroPolynomial);
        }
        Ok(CandidatePolynomial { polynomial: self })
    }

    pub(crate) fn primitive_part(&self) -> Result<Self, AlgnumError> {
        if self.is_zero() {
            return Ok(Self::zero());
        }
        let mut content = BigUint::zero();
        for coefficient in &self.coefficients {
            if !coefficient.is_zero() {
                content = if content.is_zero() {
                    coefficient.magnitude().try_clone()?
                } else {
                    content.gcd(coefficient.magnitude())?
                };
            }
        }
        let divisor = BigInt::from_sign_magnitude(Sign::Positive, content);
        let negate = self
            .coefficients
            .last()
            .is_some_and(|coefficient| coefficient.sign() == Sign::Negative);
        let mut coefficients = Vec::new();
        reserve_elements_at(
            &mut coefficients,
            self.coefficients.len(),
            AllocationResource::PolynomialCoefficients,
            AllocationContact::PolynomialPrimitivePart,
        )?;
        for coefficient in &self.coefficients {
            let reduced = coefficient.exact_div(&divisor)?;
            coefficients.push(if negate { reduced.negated()? } else { reduced });
        }
        Ok(Self { coefficients })
    }

    pub(crate) fn exact_div(&self, rhs: &Self) -> Result<Self, AlgnumError> {
        let (quotient, remainder) = self.to_rational()?.div_rem(&rhs.to_rational()?)?;
        if !remainder.is_zero() {
            return Err(BigintError::NonExactDivision.into());
        }
        quotient.to_integer_exact()
    }

    pub(crate) fn is_exactly_divisible_by(&self, rhs: &Self) -> Result<bool, AlgnumError> {
        let (_, remainder) = self.to_rational()?.div_rem(&rhs.to_rational()?)?;
        Ok(remainder.is_zero())
    }

    pub(crate) fn to_rational(&self) -> Result<RationalPolynomial, AlgnumError> {
        let mut coefficients = Vec::new();
        reserve_elements_at(
            &mut coefficients,
            self.coefficients.len(),
            AllocationResource::RationalCoefficients,
            AllocationContact::PolynomialToRational,
        )?;
        for coefficient in &self.coefficients {
            coefficients.push(ReducedRational::from_bigint(coefficient.try_clone()?)?);
        }
        Ok(RationalPolynomial { coefficients })
    }

    fn add_or_sub(&self, rhs: &Self, subtract: bool) -> Result<Self, AlgnumError> {
        let upper = self.coefficients.len().max(rhs.coefficients.len());
        let result_len = integer_sum_result_len(self, rhs, subtract, upper)?;
        let mut coefficients = Vec::new();
        reserve_elements_at(
            &mut coefficients,
            result_len,
            AllocationResource::PolynomialCoefficients,
            AllocationContact::PolynomialAddSub,
        )?;
        for index in 0..result_len {
            coefficients.push(integer_sum_at(self, rhs, subtract, index)?);
        }
        Ok(Self { coefficients })
    }

    fn from_single_coefficient(coefficient: BigInt) -> Result<Self, AlgnumError> {
        if coefficient.is_zero() {
            return Ok(Self::zero());
        }
        let mut coefficients = Vec::new();
        reserve_elements_at(
            &mut coefficients,
            1,
            AllocationResource::PolynomialCoefficients,
            AllocationContact::PolynomialSingleCoefficient,
        )?;
        coefficients.push(coefficient);
        Ok(Self { coefficients })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct RationalPolynomial {
    coefficients: Vec<ReducedRational>,
}

impl RationalPolynomial {
    pub fn from_coefficients(mut coefficients: Vec<ReducedRational>) -> Self {
        trim_rational_coefficients(&mut coefficients);
        Self { coefficients }
    }

    pub(crate) fn from_integer(polynomial: &Polynomial) -> Result<Self, AlgnumError> {
        polynomial.to_rational()
    }

    pub fn coefficients(&self) -> &[ReducedRational] {
        &self.coefficients
    }

    pub fn to_real_algebraic_coefficients(
        &self,
    ) -> Result<crate::RationalCoefficientConversion, AlgnumError> {
        let mut coefficients = Vec::new();
        for coefficient in &self.coefficients {
            coefficients.push(crate::RealAlgebraic::from_reduced_rational(coefficient)?);
        }
        Ok(crate::RationalCoefficientConversion { coefficients })
    }

    pub fn degree(&self) -> Option<usize> {
        self.coefficients.len().checked_sub(1)
    }

    pub fn is_zero(&self) -> bool {
        self.coefficients.is_empty()
    }

    pub fn try_clone(&self) -> Result<Self, AlgnumError> {
        let mut coefficients = Vec::new();
        reserve_elements_at(
            &mut coefficients,
            self.coefficients.len(),
            AllocationResource::RationalCoefficients,
            AllocationContact::RationalClone,
        )?;
        for coefficient in &self.coefficients {
            coefficients.push(coefficient.try_clone()?);
        }
        Ok(Self { coefficients })
    }

    pub fn add(&self, rhs: &Self) -> Result<Self, AlgnumError> {
        self.add_or_sub(rhs, false)
    }

    pub fn sub(&self, rhs: &Self) -> Result<Self, AlgnumError> {
        self.add_or_sub(rhs, true)
    }

    pub fn mul(&self, rhs: &Self) -> Result<Self, AlgnumError> {
        if self.is_zero() || rhs.is_zero() {
            return Ok(Self::zero());
        }
        let result_len =
            checked_product_coefficient_count(self.coefficients.len(), rhs.coefficients.len())?;
        let mut coefficients = Vec::new();
        reserve_elements_at(
            &mut coefficients,
            result_len,
            AllocationResource::RationalCoefficients,
            AllocationContact::RationalMul,
        )?;
        for _ in 0..result_len {
            coefficients.push(rational_zero()?);
        }
        for (left_index, left) in self.coefficients.iter().enumerate() {
            for (right_index, right) in rhs.coefficients.iter().enumerate() {
                let index = left_index + right_index;
                let product = left.mul(right)?;
                coefficients[index] = coefficients[index].add(&product)?;
            }
        }
        Ok(Self::from_coefficients(coefficients))
    }

    pub fn div_rem(&self, rhs: &Self) -> Result<(Self, Self), AlgnumError> {
        let Some(rhs_degree) = rhs.degree() else {
            return Err(BigintError::DivisionByZero.into());
        };
        let Some(left_degree) = self.degree() else {
            return Ok((Self::zero(), Self::zero()));
        };
        if left_degree < rhs_degree {
            return Ok((Self::zero(), self.try_clone()?));
        }
        let quotient_len = left_degree - rhs_degree + 1;
        let mut quotient = Vec::new();
        reserve_elements_at(
            &mut quotient,
            quotient_len,
            AllocationResource::RationalCoefficients,
            AllocationContact::RationalDivRem,
        )?;
        for _ in 0..quotient_len {
            quotient.push(rational_zero()?);
        }
        let mut remainder = self.try_clone()?;
        while let Some(remainder_degree) = remainder.degree() {
            if remainder_degree < rhs_degree {
                break;
            }
            let offset = remainder_degree - rhs_degree;
            let factor =
                remainder.coefficients[remainder_degree].div(&rhs.coefficients[rhs_degree])?;
            quotient[offset] = factor.try_clone()?;
            for (index, rhs_coefficient) in rhs.coefficients.iter().enumerate() {
                let target = offset + index;
                let product = factor.mul(rhs_coefficient)?;
                remainder.coefficients[target] = remainder.coefficients[target].sub(&product)?;
            }
            trim_rational_coefficients(&mut remainder.coefficients);
        }
        Ok((Self::from_coefficients(quotient), remainder))
    }

    pub fn gcd(&self, rhs: &Self) -> Result<Self, AlgnumError> {
        let mut left = self.try_clone()?;
        let mut right = rhs.try_clone()?;
        while !right.is_zero() {
            let (_, remainder) = left.div_rem(&right)?;
            left = right;
            right = remainder;
        }
        if left.is_zero() {
            return Ok(left);
        }
        let leading = left
            .coefficients
            .last()
            .map(ReducedRational::try_clone)
            .transpose()?;
        let Some(leading) = leading else {
            return Ok(Self::zero());
        };
        let mut coefficients = Vec::new();
        reserve_elements_at(
            &mut coefficients,
            left.coefficients.len(),
            AllocationResource::RationalCoefficients,
            AllocationContact::RationalGcd,
        )?;
        for coefficient in &left.coefficients {
            coefficients.push(coefficient.div(&leading)?);
        }
        Ok(Self { coefficients })
    }

    pub(crate) fn derivative_internal(&self) -> Result<Self, AlgnumError> {
        let Some(degree) = self.degree() else {
            return Ok(Self::zero());
        };
        if degree == 0 {
            return Ok(Self::zero());
        }
        let mut coefficients = Vec::new();
        reserve_elements_at(
            &mut coefficients,
            degree,
            AllocationResource::RationalCoefficients,
            AllocationContact::RationalDerivative,
        )?;
        for (index, coefficient) in self.coefficients.iter().enumerate().skip(1) {
            let multiplier = ReducedRational::from_bigint(BigInt::try_from(index)?)?;
            coefficients.push(coefficient.mul(&multiplier)?);
        }
        Ok(Self::from_coefficients(coefficients))
    }

    pub(crate) fn to_primitive_integer(&self) -> Result<Polynomial, AlgnumError> {
        if self.is_zero() {
            return Ok(Polynomial::zero());
        }
        let mut common_denominator = BigUint::one()?;
        for coefficient in &self.coefficients {
            common_denominator = common_denominator.lcm(coefficient.denominator())?;
        }
        let mut coefficients = Vec::new();
        reserve_elements_at(
            &mut coefficients,
            self.coefficients.len(),
            AllocationResource::PolynomialCoefficients,
            AllocationContact::RationalToPrimitiveInteger,
        )?;
        for coefficient in &self.coefficients {
            let multiplier = common_denominator.exact_div(coefficient.denominator())?;
            coefficients.push(BigInt::from_sign_magnitude(
                coefficient.numerator().sign(),
                coefficient.numerator().magnitude().mul(&multiplier)?,
            ));
        }
        Polynomial::from_coefficients(coefficients).primitive_part()
    }

    fn to_integer_exact(&self) -> Result<Polynomial, AlgnumError> {
        let mut coefficients = Vec::new();
        reserve_elements_at(
            &mut coefficients,
            self.coefficients.len(),
            AllocationResource::PolynomialCoefficients,
            AllocationContact::RationalToIntegerExact,
        )?;
        for coefficient in &self.coefficients {
            if coefficient.denominator().to_u32() != Some(1) {
                return Err(BigintError::NonExactDivision.into());
            }
            coefficients.push(coefficient.numerator().try_clone()?);
        }
        Ok(Polynomial::from_coefficients(coefficients))
    }

    fn add_or_sub(&self, rhs: &Self, subtract: bool) -> Result<Self, AlgnumError> {
        let upper = self.coefficients.len().max(rhs.coefficients.len());
        let result_len = rational_sum_result_len(self, rhs, subtract, upper)?;
        let mut coefficients = Vec::new();
        reserve_elements_at(
            &mut coefficients,
            result_len,
            AllocationResource::RationalCoefficients,
            AllocationContact::RationalAddSub,
        )?;
        for index in 0..result_len {
            coefficients.push(rational_sum_at(self, rhs, subtract, index)?);
        }
        Ok(Self { coefficients })
    }

    fn zero() -> Self {
        Self {
            coefficients: Vec::new(),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct CandidatePolynomial {
    polynomial: Polynomial,
}

impl CandidatePolynomial {
    pub fn polynomial(&self) -> &Polynomial {
        &self.polynomial
    }

    pub fn try_clone(&self) -> Result<Self, AlgnumError> {
        Ok(Self {
            polynomial: self.polynomial.try_clone()?,
        })
    }

    pub fn square_free(&self) -> Result<SquareFreePolynomial, AlgnumError> {
        let primitive = self.polynomial.primitive_part()?;
        let rational = primitive.to_rational()?;
        let derivative = rational.derivative_internal()?;
        let gcd = rational.gcd(&derivative)?;
        let (square_free, remainder) = rational.div_rem(&gcd)?;
        if !remainder.is_zero() {
            return Err(BigintError::NonExactDivision.into());
        }
        Ok(SquareFreePolynomial {
            polynomial: square_free.to_primitive_integer()?,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct SquareFreePolynomial {
    polynomial: Polynomial,
}

impl SquareFreePolynomial {
    pub fn polynomial(&self) -> &Polynomial {
        &self.polynomial
    }

    pub fn try_clone(&self) -> Result<Self, AlgnumError> {
        Ok(Self {
            polynomial: self.polynomial.try_clone()?,
        })
    }

    #[cfg(test)]
    pub(crate) fn from_polynomial(polynomial: Polynomial) -> Self {
        Self { polynomial }
    }
}

fn integer_sum_result_len(
    left: &Polynomial,
    right: &Polynomial,
    subtract: bool,
    upper: usize,
) -> Result<usize, AlgnumError> {
    for index in (0..upper).rev() {
        if !integer_sum_at(left, right, subtract, index)?.is_zero() {
            return Ok(index + 1);
        }
    }
    Ok(0)
}

fn integer_sum_at(
    left: &Polynomial,
    right: &Polynomial,
    subtract: bool,
    index: usize,
) -> Result<BigInt, AlgnumError> {
    match (left.coefficients.get(index), right.coefficients.get(index)) {
        (Some(left), Some(right)) if subtract => Ok(left.sub(right)?),
        (Some(left), Some(right)) => Ok(left.add(right)?),
        (Some(value), None) => Ok(value.try_clone()?),
        (None, Some(value)) if subtract => Ok(value.negated()?),
        (None, Some(value)) => Ok(value.try_clone()?),
        (None, None) => Ok(BigInt::zero()),
    }
}

fn rational_sum_result_len(
    left: &RationalPolynomial,
    right: &RationalPolynomial,
    subtract: bool,
    upper: usize,
) -> Result<usize, AlgnumError> {
    for index in (0..upper).rev() {
        if !rational_sum_at(left, right, subtract, index)?.is_zero() {
            return Ok(index + 1);
        }
    }
    Ok(0)
}

fn rational_sum_at(
    left: &RationalPolynomial,
    right: &RationalPolynomial,
    subtract: bool,
    index: usize,
) -> Result<ReducedRational, AlgnumError> {
    match (left.coefficients.get(index), right.coefficients.get(index)) {
        (Some(left), Some(right)) if subtract => Ok(left.sub(right)?),
        (Some(left), Some(right)) => Ok(left.add(right)?),
        (Some(value), None) => Ok(value.try_clone()?),
        (None, Some(value)) if subtract => negate_rational(value),
        (None, Some(value)) => Ok(value.try_clone()?),
        (None, None) => rational_zero(),
    }
}

fn negate_rational(value: &ReducedRational) -> Result<ReducedRational, AlgnumError> {
    Ok(RawRational::new(
        value.numerator().negated()?,
        value.denominator().try_clone()?,
    )
    .reduce()?
    .into_reduced())
}

fn rational_zero() -> Result<ReducedRational, AlgnumError> {
    Ok(ReducedRational::from_bigint(BigInt::zero())?)
}

fn checked_product_coefficient_count(
    left_len: usize,
    right_len: usize,
) -> Result<usize, AlgnumError> {
    let left_degree_usize = left_len.saturating_sub(1);
    if let Some(count) = left_degree_usize.checked_add(right_len) {
        return Ok(count);
    }
    let left_degree = BigUint::try_from(left_degree_usize)?;
    let right_degree = BigUint::try_from(right_len.saturating_sub(1))?;
    let required = left_degree.add(&right_degree)?;
    let maximum = BigUint::try_from(usize::MAX - 1)?;
    Err(AlgnumError::RepresentationLimit {
        resource: RepresentationResource::PolynomialDegree,
        required,
        maximum,
    })
}

fn trim_integer_coefficients(coefficients: &mut Vec<BigInt>) {
    while coefficients.last().is_some_and(BigInt::is_zero) {
        coefficients.pop();
    }
}

fn trim_rational_coefficients(coefficients: &mut Vec<ReducedRational>) {
    while coefficients.last().is_some_and(ReducedRational::is_zero) {
        coefficients.pop();
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use neco_bigint::{BigInt, BigUint, RawRational, Sign};

    use super::{Polynomial, RationalPolynomial};
    use crate::AlgnumError;

    fn integer(value: i32) -> BigInt {
        BigInt::try_from(value).unwrap()
    }

    fn polynomial(values: &[i32]) -> Polynomial {
        Polynomial::from_coefficients(values.iter().copied().map(integer).collect())
    }

    fn rational(numerator: i32, denominator: u32) -> neco_bigint::ReducedRational {
        RawRational::new(integer(numerator), BigUint::try_from(denominator).unwrap())
            .reduce()
            .unwrap()
            .into_reduced()
    }

    fn assert_integer_coefficients(value: &Polynomial, expected: &[i32]) {
        assert_eq!(value.coefficients().len(), expected.len());
        for (coefficient, expected) in value.coefficients().iter().zip(expected) {
            let expected_sign = match expected.cmp(&0) {
                core::cmp::Ordering::Less => Sign::Negative,
                core::cmp::Ordering::Equal => Sign::Zero,
                core::cmp::Ordering::Greater => Sign::Positive,
            };
            assert_eq!(coefficient.sign(), expected_sign);
            assert_eq!(
                coefficient.magnitude().to_u32(),
                Some(expected.unsigned_abs())
            );
        }
    }

    #[test]
    fn integer_normalization_and_operations_preserve_coefficient_order() {
        let normalized = polynomial(&[2, 0, 0]);
        assert_integer_coefficients(&normalized, &[2]);
        assert_eq!(normalized.degree(), Some(0));

        let left = polynomial(&[1, 2, 1]);
        let right = polynomial(&[-1, 0, 1]);
        assert_integer_coefficients(&left.add(&right).unwrap(), &[0, 2, 2]);
        assert_integer_coefficients(&left.sub(&left).unwrap(), &[]);
        assert_integer_coefficients(&left.mul(&right).unwrap(), &[-1, -2, 0, 2, 1]);
        assert_integer_coefficients(&left.derivative().unwrap(), &[2, 2]);
        assert_eq!(left.evaluate_bigint(&integer(2)).unwrap(), integer(9));
        assert_integer_coefficients(&left.compose(&polynomial(&[1, 1])).unwrap(), &[4, 4, 1]);
    }

    #[test]
    fn rational_division_and_gcd_use_monic_normal_form() {
        let dividend = RationalPolynomial::from_coefficients(vec![
            rational(-1, 1),
            rational(0, 1),
            rational(1, 1),
        ]);
        let divisor = RationalPolynomial::from_coefficients(vec![rational(-1, 1), rational(1, 1)]);
        let (quotient, remainder) = dividend.div_rem(&divisor).unwrap();
        assert!(remainder.is_zero());
        assert_eq!(quotient.coefficients(), [rational(1, 1), rational(1, 1)]);

        let repeated = RationalPolynomial::from_coefficients(vec![
            rational(1, 1),
            rational(-2, 1),
            rational(1, 1),
        ]);
        assert_eq!(
            dividend.gcd(&repeated).unwrap().coefficients(),
            [rational(-1, 1), rational(1, 1)]
        );
    }

    #[test]
    fn square_free_removes_content_sign_and_repeated_factor() {
        let candidate = polynomial(&[0, -2, 4, -2]).candidate().unwrap();
        let square_free = candidate.square_free().unwrap();
        assert_integer_coefficients(square_free.polynomial(), &[0, -1, 1]);
    }

    #[test]
    fn candidate_rejects_zero_and_nonzero_constants() {
        assert_eq!(
            Polynomial::zero().candidate(),
            Err(AlgnumError::ZeroPolynomial)
        );
        assert_eq!(
            polynomial(&[3]).candidate(),
            Err(AlgnumError::ZeroPolynomial)
        );
    }
}
