use core::cmp::Ordering;

use crate::{BigUint, BigintError};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Sign {
    Negative,
    Zero,
    Positive,
}

#[derive(Debug, Eq, PartialEq)]
pub struct BigInt {
    sign: Sign,
    magnitude: BigUint,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ExtendedGcd {
    gcd: BigUint,
    x: BigInt,
    y: BigInt,
}

impl BigInt {
    pub fn zero() -> Self {
        Self {
            sign: Sign::Zero,
            magnitude: BigUint::zero(),
        }
    }

    pub fn one() -> Result<Self, BigintError> {
        Ok(Self::from_sign_magnitude(Sign::Positive, BigUint::one()?))
    }

    pub fn from_sign_magnitude(sign: Sign, magnitude: BigUint) -> Self {
        let sign = if magnitude.is_zero() {
            Sign::Zero
        } else if sign == Sign::Zero {
            Sign::Positive
        } else {
            sign
        };
        Self { sign, magnitude }
    }

    pub fn sign(&self) -> Sign {
        self.sign
    }

    pub fn magnitude(&self) -> &BigUint {
        &self.magnitude
    }

    pub fn is_zero(&self) -> bool {
        self.sign == Sign::Zero
    }

    pub fn try_clone(&self) -> Result<Self, BigintError> {
        Ok(Self {
            sign: self.sign,
            magnitude: self.magnitude.try_clone()?,
        })
    }

    pub fn abs(&self) -> Result<BigUint, BigintError> {
        self.magnitude.try_clone()
    }

    pub fn negated(&self) -> Result<Self, BigintError> {
        let sign = match self.sign {
            Sign::Negative => Sign::Positive,
            Sign::Zero => Sign::Zero,
            Sign::Positive => Sign::Negative,
        };
        Ok(Self {
            sign,
            magnitude: self.magnitude.try_clone()?,
        })
    }

    pub(crate) fn into_shr_bits(self, bits: usize) -> Self {
        Self::from_sign_magnitude(self.sign, self.magnitude.into_shr_bits(bits))
    }

    pub fn add(&self, rhs: &Self) -> Result<Self, BigintError> {
        match (self.sign, rhs.sign) {
            (Sign::Zero, _) => rhs.try_clone(),
            (_, Sign::Zero) => self.try_clone(),
            (left, right) if left == right => Ok(Self::from_sign_magnitude(
                left,
                self.magnitude.add(&rhs.magnitude)?,
            )),
            _ => match self.magnitude.cmp(&rhs.magnitude) {
                Ordering::Greater => Ok(Self::from_sign_magnitude(
                    self.sign,
                    self.magnitude.checked_sub(&rhs.magnitude)?,
                )),
                Ordering::Less => Ok(Self::from_sign_magnitude(
                    rhs.sign,
                    rhs.magnitude.checked_sub(&self.magnitude)?,
                )),
                Ordering::Equal => Ok(Self::zero()),
            },
        }
    }

    pub fn sub(&self, rhs: &Self) -> Result<Self, BigintError> {
        self.add(&rhs.negated()?)
    }

    pub fn mul(&self, rhs: &Self) -> Result<Self, BigintError> {
        let sign = product_sign(self.sign, rhs.sign);
        Ok(Self::from_sign_magnitude(
            sign,
            self.magnitude.mul(&rhs.magnitude)?,
        ))
    }

    pub fn div_rem_euclid(&self, divisor: &Self) -> Result<(Self, BigUint), BigintError> {
        if divisor.is_zero() {
            return Err(BigintError::DivisionByZero);
        }
        let (magnitude_quotient, remainder) = self.magnitude.div_rem(&divisor.magnitude)?;
        let quotient_sign = product_sign(self.sign, divisor.sign);
        let quotient = Self::from_sign_magnitude(quotient_sign, magnitude_quotient);
        if self.sign != Sign::Negative || remainder.is_zero() {
            return Ok((quotient, remainder));
        }

        let divisor_sign = Self::from_sign_magnitude(divisor.sign, BigUint::one()?);
        let corrected_quotient = quotient.sub(&divisor_sign)?;
        let corrected_remainder = divisor.magnitude.checked_sub(&remainder)?;
        Ok((corrected_quotient, corrected_remainder))
    }

    pub fn exact_div(&self, divisor: &Self) -> Result<Self, BigintError> {
        if divisor.is_zero() {
            return Err(BigintError::DivisionByZero);
        }
        let quotient = self.magnitude.exact_div(&divisor.magnitude)?;
        Ok(Self::from_sign_magnitude(
            product_sign(self.sign, divisor.sign),
            quotient,
        ))
    }

    pub fn extended_gcd(&self, rhs: &Self) -> Result<ExtendedGcd, BigintError> {
        let mut old_remainder = self.magnitude.try_clone()?;
        let mut remainder = rhs.magnitude.try_clone()?;
        let mut old_x = Self::one()?;
        let mut x = Self::zero();
        let mut old_y = Self::zero();
        let mut y = Self::one()?;

        while !remainder.is_zero() {
            let (quotient, next_remainder) = old_remainder.div_rem(&remainder)?;
            let quotient = Self::from_sign_magnitude(Sign::Positive, quotient);
            let next_x = old_x.sub(&quotient.mul(&x)?)?;
            let next_y = old_y.sub(&quotient.mul(&y)?)?;
            old_remainder = remainder;
            remainder = next_remainder;
            old_x = x;
            x = next_x;
            old_y = y;
            y = next_y;
        }

        if self.sign == Sign::Negative {
            old_x = old_x.negated()?;
        }
        if rhs.sign == Sign::Negative {
            old_y = old_y.negated()?;
        }
        Ok(ExtendedGcd {
            gcd: old_remainder,
            x: old_x,
            y: old_y,
        })
    }

    pub fn pow_u32(&self, exponent: u32) -> Result<Self, BigintError> {
        let sign = if exponent == 0 {
            Sign::Positive
        } else if self.sign == Sign::Negative && exponent & 1 != 0 {
            Sign::Negative
        } else if self.sign == Sign::Zero {
            Sign::Zero
        } else {
            Sign::Positive
        };
        Ok(Self::from_sign_magnitude(
            sign,
            self.magnitude.pow_u32(exponent)?,
        ))
    }
}

impl ExtendedGcd {
    pub fn gcd(&self) -> &BigUint {
        &self.gcd
    }

    pub fn x(&self) -> &BigInt {
        &self.x
    }

    pub fn y(&self) -> &BigInt {
        &self.y
    }

    pub fn try_clone(&self) -> Result<Self, BigintError> {
        Ok(Self {
            gcd: self.gcd.try_clone()?,
            x: self.x.try_clone()?,
            y: self.y.try_clone()?,
        })
    }
}

fn product_sign(left: Sign, right: Sign) -> Sign {
    match (left, right) {
        (Sign::Zero, _) | (_, Sign::Zero) => Sign::Zero,
        (Sign::Negative, Sign::Negative) | (Sign::Positive, Sign::Positive) => Sign::Positive,
        _ => Sign::Negative,
    }
}

impl Ord for BigInt {
    fn cmp(&self, rhs: &Self) -> Ordering {
        match (self.sign, rhs.sign) {
            (Sign::Negative, Sign::Negative) => rhs.magnitude.cmp(&self.magnitude),
            (left, right) if left == right => self.magnitude.cmp(&rhs.magnitude),
            _ => self.sign.cmp(&rhs.sign),
        }
    }
}

impl PartialOrd for BigInt {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

macro_rules! impl_try_from_unsigned {
    ($($type:ty),* $(,)?) => {$ (
        impl TryFrom<$type> for BigInt {
            type Error = BigintError;

            fn try_from(value: $type) -> Result<Self, Self::Error> {
                Ok(Self::from_sign_magnitude(Sign::Positive, BigUint::try_from(value)?))
            }
        }
    )* };
}

macro_rules! impl_try_from_signed {
    ($($type:ty),* $(,)?) => {$ (
        impl TryFrom<$type> for BigInt {
            type Error = BigintError;

            fn try_from(value: $type) -> Result<Self, Self::Error> {
                let sign = if value < 0 { Sign::Negative } else if value == 0 { Sign::Zero } else { Sign::Positive };
                let magnitude = BigUint::try_from(value.unsigned_abs())?;
                Ok(Self::from_sign_magnitude(sign, magnitude))
            }
        }
    )* };
}

impl_try_from_unsigned!(u8, u16, u32, u64, usize);
impl_try_from_signed!(i8, i16, i32, i64, isize);

#[cfg(test)]
mod tests {
    use super::{BigInt, Sign};

    #[test]
    fn negative_division_is_euclidean() {
        let dividend = BigInt::try_from(-7_i32).unwrap();
        let divisor = BigInt::try_from(3_i32).unwrap();
        let (quotient, remainder) = dividend.div_rem_euclid(&divisor).unwrap();
        assert_eq!(quotient.sign(), Sign::Negative);
        assert_eq!(quotient.magnitude().to_u32(), Some(3));
        assert_eq!(remainder.to_u32(), Some(2));
    }

    #[test]
    fn bezout_vector() {
        let left = BigInt::try_from(30_i32).unwrap();
        let right = BigInt::try_from(21_i32).unwrap();
        let result = left.extended_gcd(&right).unwrap();
        assert_eq!(result.gcd().to_u32(), Some(3));
        let value = left
            .mul(result.x())
            .unwrap()
            .add(&right.mul(result.y()).unwrap())
            .unwrap();
        assert_eq!(value, BigInt::try_from(3_i32).unwrap());
    }
}
