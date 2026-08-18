use core::cmp::Ordering;

use crate::{BigInt, BigUint, BigintError, Dyadic, Sign};

#[derive(Debug, Eq, PartialEq)]
pub struct RawRational {
    numerator: BigInt,
    denominator: BigUint,
}

impl RawRational {
    pub fn new(numerator: BigInt, denominator: BigUint) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    pub fn numerator(&self) -> &BigInt {
        &self.numerator
    }

    pub fn denominator(&self) -> &BigUint {
        &self.denominator
    }

    pub fn reduce(&self) -> Result<RationalReduction, BigintError> {
        if self.denominator.is_zero() {
            return Err(BigintError::ZeroDenominator);
        }

        let gcd = self.numerator.magnitude().gcd(&self.denominator)?;
        let reduced = if self.numerator.is_zero() {
            ReducedRational {
                numerator: BigInt::zero(),
                denominator: BigUint::one()?,
            }
        } else {
            let magnitude = self.numerator.magnitude().exact_div(&gcd)?;
            ReducedRational {
                numerator: BigInt::from_sign_magnitude(self.numerator.sign(), magnitude),
                denominator: self.denominator.exact_div(&gcd)?,
            }
        };

        Ok(RationalReduction {
            input: self.try_clone()?,
            gcd,
            reduced,
        })
    }

    pub fn try_clone(&self) -> Result<Self, BigintError> {
        Ok(Self {
            numerator: self.numerator.try_clone()?,
            denominator: self.denominator.try_clone()?,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct RationalReduction {
    input: RawRational,
    gcd: BigUint,
    reduced: ReducedRational,
}

impl RationalReduction {
    pub fn input(&self) -> &RawRational {
        &self.input
    }

    pub fn gcd(&self) -> &BigUint {
        &self.gcd
    }

    pub fn reduced(&self) -> &ReducedRational {
        &self.reduced
    }

    pub fn into_reduced(self) -> ReducedRational {
        self.reduced
    }

    pub fn try_clone(&self) -> Result<Self, BigintError> {
        Ok(Self {
            input: self.input.try_clone()?,
            gcd: self.gcd.try_clone()?,
            reduced: self.reduced.try_clone()?,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ReducedRational {
    numerator: BigInt,
    denominator: BigUint,
}

impl ReducedRational {
    pub fn from_bigint(value: BigInt) -> Result<Self, BigintError> {
        Ok(Self {
            numerator: value,
            denominator: BigUint::one()?,
        })
    }

    pub fn numerator(&self) -> &BigInt {
        &self.numerator
    }

    pub fn denominator(&self) -> &BigUint {
        &self.denominator
    }

    pub fn is_zero(&self) -> bool {
        self.numerator.is_zero()
    }

    pub fn try_clone(&self) -> Result<Self, BigintError> {
        Ok(Self {
            numerator: self.numerator.try_clone()?,
            denominator: self.denominator.try_clone()?,
        })
    }

    pub fn add(&self, rhs: &Self) -> Result<Self, BigintError> {
        let left = mul_int_uint(&self.numerator, &rhs.denominator)?;
        let right = mul_int_uint(&rhs.numerator, &self.denominator)?;
        let numerator = left.add(&right)?;
        let denominator = self.denominator.mul(&rhs.denominator)?;
        reduce_parts(numerator, denominator)
    }

    pub fn sub(&self, rhs: &Self) -> Result<Self, BigintError> {
        let left = mul_int_uint(&self.numerator, &rhs.denominator)?;
        let right = mul_int_uint(&rhs.numerator, &self.denominator)?;
        let numerator = left.sub(&right)?;
        let denominator = self.denominator.mul(&rhs.denominator)?;
        reduce_parts(numerator, denominator)
    }

    pub fn mul(&self, rhs: &Self) -> Result<Self, BigintError> {
        let numerator = self.numerator.mul(&rhs.numerator)?;
        let denominator = self.denominator.mul(&rhs.denominator)?;
        reduce_parts(numerator, denominator)
    }

    pub fn div(&self, rhs: &Self) -> Result<Self, BigintError> {
        if rhs.is_zero() {
            return Err(BigintError::DivisionByZero);
        }

        let magnitude = self.numerator.magnitude().mul(&rhs.denominator)?;
        let sign = multiply_sign(self.numerator.sign(), rhs.numerator.sign());
        let numerator = BigInt::from_sign_magnitude(sign, magnitude);
        let denominator = self.denominator.mul(rhs.numerator.magnitude())?;
        reduce_parts(numerator, denominator)
    }

    pub fn pow_i32(&self, exponent: i32) -> Result<Self, BigintError> {
        if exponent == 0 {
            return Self::from_bigint(BigInt::one()?);
        }

        let power = exponent.unsigned_abs();
        if exponent > 0 {
            return Ok(Self {
                numerator: self.numerator.pow_u32(power)?,
                denominator: self.denominator.pow_u32(power)?,
            });
        }
        if self.is_zero() {
            return Err(BigintError::DivisionByZero);
        }

        let numerator_sign = if self.numerator.sign() == Sign::Negative && power % 2 == 1 {
            Sign::Negative
        } else {
            Sign::Positive
        };
        Ok(Self {
            numerator: BigInt::from_sign_magnitude(
                numerator_sign,
                self.denominator.pow_u32(power)?,
            ),
            denominator: self.numerator.magnitude().pow_u32(power)?,
        })
    }

    pub fn floor(&self) -> Result<BigInt, BigintError> {
        let (quotient, _) = self.euclidean_parts()?;
        Ok(quotient)
    }

    pub fn ceil(&self) -> Result<BigInt, BigintError> {
        let (quotient, remainder) = self.euclidean_parts()?;
        if remainder.is_zero() {
            Ok(quotient)
        } else {
            quotient.add(&BigInt::one()?)
        }
    }

    pub fn dyadic_floor(&self, bits: u32) -> Result<Dyadic, BigintError> {
        let (quotient, _) = self.scaled_euclidean_parts(bits)?;
        Ok(Dyadic::new(quotient, bits))
    }

    pub fn dyadic_ceil(&self, bits: u32) -> Result<Dyadic, BigintError> {
        let (quotient, remainder) = self.scaled_euclidean_parts(bits)?;
        let integer = if remainder.is_zero() {
            quotient
        } else {
            quotient.add(&BigInt::one()?)?
        };
        Ok(Dyadic::new(integer, bits))
    }

    fn euclidean_parts(&self) -> Result<(BigInt, BigUint), BigintError> {
        let divisor = BigInt::from_sign_magnitude(Sign::Positive, self.denominator.try_clone()?);
        self.numerator.div_rem_euclid(&divisor)
    }

    fn scaled_euclidean_parts(&self, bits: u32) -> Result<(BigInt, BigUint), BigintError> {
        let shift = usize::try_from(bits).map_err(|_| BigintError::CapacityOverflow)?;
        let magnitude = self.numerator.magnitude().shl_bits(shift)?;
        let scaled = BigInt::from_sign_magnitude(self.numerator.sign(), magnitude);
        let divisor = BigInt::from_sign_magnitude(Sign::Positive, self.denominator.try_clone()?);
        scaled.div_rem_euclid(&divisor)
    }
}

impl Ord for ReducedRational {
    fn cmp(&self, rhs: &Self) -> Ordering {
        match self.numerator.sign().cmp(&rhs.numerator.sign()) {
            Ordering::Equal => match self.numerator.sign() {
                Sign::Zero => Ordering::Equal,
                Sign::Positive => compare_products(
                    self.numerator.magnitude(),
                    &rhs.denominator,
                    rhs.numerator.magnitude(),
                    &self.denominator,
                ),
                Sign::Negative => compare_products(
                    rhs.numerator.magnitude(),
                    &self.denominator,
                    self.numerator.magnitude(),
                    &rhs.denominator,
                ),
            },
            ordering => ordering,
        }
    }
}

impl PartialOrd for ReducedRational {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

fn reduce_parts(numerator: BigInt, denominator: BigUint) -> Result<ReducedRational, BigintError> {
    RawRational::new(numerator, denominator)
        .reduce()
        .map(RationalReduction::into_reduced)
}

fn mul_int_uint(value: &BigInt, factor: &BigUint) -> Result<BigInt, BigintError> {
    Ok(BigInt::from_sign_magnitude(
        value.sign(),
        value.magnitude().mul(factor)?,
    ))
}

fn multiply_sign(left: Sign, right: Sign) -> Sign {
    match (left, right) {
        (Sign::Zero, _) | (_, Sign::Zero) => Sign::Zero,
        (Sign::Positive, Sign::Positive) | (Sign::Negative, Sign::Negative) => Sign::Positive,
        _ => Sign::Negative,
    }
}

fn compare_products(a: &BigUint, b: &BigUint, c: &BigUint, d: &BigUint) -> Ordering {
    let left_limbs = a.limbs_le().len() + b.limbs_le().len();
    let right_limbs = c.limbs_le().len() + d.limbs_le().len();
    let top = left_limbs.max(right_limbs);

    for index in (0..top).rev() {
        match product_limb(a, b, index).cmp(&product_limb(c, d, index)) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
    }
    Ordering::Equal
}

fn product_limb(left: &BigUint, right: &BigUint, target: usize) -> u32 {
    let left_limbs = left.limbs_le();
    let right_limbs = right.limbs_le();
    let mut carry = 0_u128;

    for index in 0..=target {
        let start = index.saturating_sub(right_limbs.len().saturating_sub(1));
        let end = index.min(left_limbs.len().saturating_sub(1));
        let mut total = carry;
        if !left_limbs.is_empty() && !right_limbs.is_empty() && start <= end {
            for (left_index, left) in left_limbs.iter().enumerate().take(end + 1).skip(start) {
                let right_index = index - left_index;
                total += u128::from(*left) * u128::from(right_limbs[right_index]);
            }
        }
        if index == target {
            return total as u32;
        }
        carry = total >> 32;
    }
    0
}

#[cfg(test)]
mod tests {
    use core::cmp::Ordering;

    use super::{RawRational, ReducedRational};
    use crate::{BigInt, BigUint, BigintError, Sign};

    fn integer(value: i32) -> BigInt {
        BigInt::try_from(value).unwrap()
    }

    fn natural(value: u32) -> BigUint {
        BigUint::try_from(value).unwrap()
    }

    fn rational(numerator: i32, denominator: u32) -> ReducedRational {
        RawRational::new(integer(numerator), natural(denominator))
            .reduce()
            .unwrap()
            .into_reduced()
    }

    #[test]
    fn reduction_preserves_input_and_gcd() {
        let reduction = RawRational::new(integer(-6), natural(8)).reduce().unwrap();
        assert_eq!(reduction.input().numerator(), &integer(-6));
        assert_eq!(reduction.input().denominator(), &natural(8));
        assert_eq!(reduction.gcd(), &natural(2));
        assert_eq!(reduction.reduced().numerator(), &integer(-3));
        assert_eq!(reduction.reduced().denominator(), &natural(4));
    }

    #[test]
    fn reduction_normalizes_zero_and_rejects_zero_denominator() {
        let zero = RawRational::new(integer(0), natural(7)).reduce().unwrap();
        assert_eq!(zero.gcd(), &natural(7));
        assert_eq!(zero.reduced().numerator(), &integer(0));
        assert_eq!(zero.reduced().denominator(), &natural(1));
        assert_eq!(
            RawRational::new(integer(1), BigUint::zero()).reduce(),
            Err(BigintError::ZeroDenominator)
        );
    }

    #[test]
    fn arithmetic_reduces_results() {
        let one_half = rational(1, 2);
        let one_third = rational(1, 3);
        assert_eq!(one_half.add(&one_third).unwrap(), rational(5, 6));
        assert_eq!(one_half.sub(&one_third).unwrap(), rational(1, 6));
        assert_eq!(one_half.mul(&one_third).unwrap(), rational(1, 6));
        assert_eq!(one_half.div(&one_third).unwrap(), rational(3, 2));
        assert_eq!(
            one_half.div(&rational(0, 1)),
            Err(BigintError::DivisionByZero)
        );
    }

    #[test]
    fn powers_include_negative_exponents() {
        let value = rational(-2, 3);
        assert_eq!(value.pow_i32(0).unwrap(), rational(1, 1));
        assert_eq!(value.pow_i32(3).unwrap(), rational(-8, 27));
        assert_eq!(value.pow_i32(-2).unwrap(), rational(9, 4));
        assert_eq!(value.pow_i32(-3).unwrap(), rational(-27, 8));
        assert_eq!(rational(0, 1).pow_i32(-1), Err(BigintError::DivisionByZero));
    }

    #[test]
    fn integer_and_dyadic_rounding_use_euclidean_quotients() {
        let value = rational(-7, 3);
        assert_eq!(value.floor().unwrap(), integer(-3));
        assert_eq!(value.ceil().unwrap(), integer(-2));

        let floor = value.dyadic_floor(2).unwrap();
        let ceil = value.dyadic_ceil(2).unwrap();
        assert_eq!(floor.integer(), &integer(-5));
        assert_eq!(floor.exponent(), 1);
        assert_eq!(ceil.integer(), &integer(-9));
        assert_eq!(ceil.exponent(), 2);
    }

    #[test]
    fn ordering_compares_cross_products_without_allocation() {
        assert_eq!(rational(2, 3).cmp(&rational(3, 4)), Ordering::Less);
        assert_eq!(rational(-2, 3).cmp(&rational(-3, 4)), Ordering::Greater);
        assert_eq!(rational(2, 4).cmp(&rational(1, 2)), Ordering::Equal);
        assert_eq!(rational(-1, 2).cmp(&rational(0, 1)), Ordering::Less);
        assert_eq!(integer(-1).sign(), Sign::Negative);
    }
}
