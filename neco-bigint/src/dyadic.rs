use core::cmp::Ordering;

use crate::{BigInt, BigUint, BigintError, Sign};

#[derive(Debug, Eq, PartialEq)]
pub struct Dyadic {
    integer: BigInt,
    exponent: u32,
}

impl Dyadic {
    pub fn new(integer: BigInt, exponent: u32) -> Self {
        if integer.is_zero() {
            return Self {
                integer,
                exponent: 0,
            };
        }

        let mut removed = 0_u32;
        while removed < exponent && !integer.magnitude().bit(removed as usize) {
            removed += 1;
        }
        Self {
            integer: integer.into_shr_bits(removed as usize),
            exponent: exponent - removed,
        }
    }

    pub fn integer(&self) -> &BigInt {
        &self.integer
    }

    pub fn exponent(&self) -> u32 {
        self.exponent
    }

    pub fn is_zero(&self) -> bool {
        self.integer.is_zero()
    }

    pub fn try_clone(&self) -> Result<Self, BigintError> {
        Ok(Self {
            integer: self.integer.try_clone()?,
            exponent: self.exponent,
        })
    }

    pub fn add(&self, rhs: &Self) -> Result<Self, BigintError> {
        self.add_with(rhs, false)
    }

    pub fn sub(&self, rhs: &Self) -> Result<Self, BigintError> {
        self.add_with(rhs, true)
    }

    pub fn mul(&self, rhs: &Self) -> Result<Self, BigintError> {
        let exponent = self
            .exponent
            .checked_add(rhs.exponent)
            .ok_or_else(|| exponent_overflow(u64::from(self.exponent) + u64::from(rhs.exponent)))?;
        Ok(Self::new(self.integer.mul(&rhs.integer)?, exponent))
    }

    pub fn midpoint(&self, rhs: &Self) -> Result<Self, BigintError> {
        let common = self.exponent.max(rhs.exponent);
        let exponent = common
            .checked_add(1)
            .ok_or_else(|| exponent_overflow(u64::from(common) + 1))?;
        let left = shifted_integer(&self.integer, common - self.exponent)?;
        let right = shifted_integer(&rhs.integer, common - rhs.exponent)?;
        Ok(Self::new(left.add(&right)?, exponent))
    }

    pub fn from_f64_exact(value: f64) -> Result<Self, BigintError> {
        let bits = value.to_bits();
        let exponent_bits = ((bits >> 52) & 0x7ff) as u16;
        let fraction = bits & ((1_u64 << 52) - 1);
        if exponent_bits == 0x7ff {
            return Err(BigintError::NonFiniteFloat);
        }
        if exponent_bits == 0 && fraction == 0 {
            return Ok(Self::new(BigInt::zero(), 0));
        }

        let sign = if bits >> 63 == 0 {
            Sign::Positive
        } else {
            Sign::Negative
        };
        let (significand, binary_exponent) = if exponent_bits == 0 {
            (fraction, -1074_i32)
        } else {
            (
                (1_u64 << 52) | fraction,
                i32::from(exponent_bits) - 1023 - 52,
            )
        };
        let mut magnitude = BigUint::try_from(significand)?;
        let exponent = if binary_exponent >= 0 {
            magnitude = magnitude.shl_bits(binary_exponent as usize)?;
            0
        } else {
            (-binary_exponent) as u32
        };
        Ok(Self::new(
            BigInt::from_sign_magnitude(sign, magnitude),
            exponent,
        ))
    }

    pub fn round_to_f64_ties_even(&self) -> Result<f64, BigintError> {
        if self.is_zero() {
            return Ok(f64::from_bits(0));
        }
        let maximum = maximum_finite()?;
        if compare_absolute(self, &maximum) == Ordering::Greater {
            return Err(BigintError::FloatOutOfRange);
        }

        let magnitude = self.integer.magnitude();
        let bit_len = magnitude.bit_len();
        let top = bit_len as i128 - 1 - i128::from(self.exponent);
        let payload = if top < -1022 {
            let units = if self.exponent <= 1074 {
                let shift = (1074 - self.exponent) as usize;
                magnitude_to_u64_shifted(magnitude, shift)
            } else {
                round_right_to_u64(magnitude, (self.exponent - 1074) as usize)
            };
            if units == (1_u64 << 52) {
                1_u64 << 52
            } else {
                units
            }
        } else {
            let drop = bit_len.saturating_sub(53);
            let mut significand = round_right_to_u64(magnitude, drop);
            let mut rounded_top = top;
            if significand == (1_u64 << 53) {
                significand >>= 1;
                rounded_top += 1;
            } else if bit_len < 53 {
                significand <<= 53 - bit_len;
            }
            let encoded_exponent = (rounded_top + 1023) as u64;
            (encoded_exponent << 52) | (significand & ((1_u64 << 52) - 1))
        };
        let sign = if payload != 0 && self.integer.sign() == Sign::Negative {
            1_u64 << 63
        } else {
            0
        };
        Ok(f64::from_bits(sign | payload))
    }

    fn add_with(&self, rhs: &Self, subtract: bool) -> Result<Self, BigintError> {
        let common = self.exponent.max(rhs.exponent);
        let left = shifted_integer(&self.integer, common - self.exponent)?;
        let right = shifted_integer(&rhs.integer, common - rhs.exponent)?;
        let integer = if subtract {
            left.sub(&right)?
        } else {
            left.add(&right)?
        };
        Ok(Self::new(integer, common))
    }
}

impl Ord for Dyadic {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.integer.sign().cmp(&other.integer.sign()) {
            Ordering::Equal => match self.integer.sign() {
                Sign::Negative => compare_absolute(other, self),
                Sign::Zero => Ordering::Equal,
                Sign::Positive => compare_absolute(self, other),
            },
            ordering => ordering,
        }
    }
}

impl PartialOrd for Dyadic {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct DyadicEnclosure {
    lower: Dyadic,
    upper: Dyadic,
}

impl DyadicEnclosure {
    pub fn new(lower: Dyadic, upper: Dyadic) -> Result<Self, BigintError> {
        if lower > upper {
            return Err(BigintError::InvalidInterval);
        }
        Ok(Self { lower, upper })
    }

    pub fn lower(&self) -> &Dyadic {
        &self.lower
    }

    pub fn upper(&self) -> &Dyadic {
        &self.upper
    }

    pub fn try_clone(&self) -> Result<Self, BigintError> {
        Ok(Self {
            lower: self.lower.try_clone()?,
            upper: self.upper.try_clone()?,
        })
    }

    pub fn width(&self) -> Result<Dyadic, BigintError> {
        self.upper.sub(&self.lower)
    }

    pub fn midpoint(&self) -> Result<Dyadic, BigintError> {
        self.lower.midpoint(&self.upper)
    }

    pub fn contains_dyadic(&self, value: &Dyadic) -> bool {
        self.lower <= *value && *value <= self.upper
    }
}

fn shifted_integer(value: &BigInt, bits: u32) -> Result<BigInt, BigintError> {
    if value.is_zero() || bits == 0 {
        return value.try_clone();
    }
    Ok(BigInt::from_sign_magnitude(
        value.sign(),
        value.magnitude().shl_bits(bits as usize)?,
    ))
}

fn exponent_overflow(required: u64) -> BigintError {
    match BigUint::try_from(required) {
        Ok(required) => BigintError::ExponentOverflow {
            required,
            maximum: u32::MAX,
        },
        Err(error) => error,
    }
}

fn compare_absolute(left: &Dyadic, right: &Dyadic) -> Ordering {
    let left_magnitude = left.integer.magnitude();
    let right_magnitude = right.integer.magnitude();
    let left_len = left_magnitude.bit_len();
    let right_len = right_magnitude.bit_len();
    let left_top = left_len as i128 - i128::from(left.exponent);
    let right_top = right_len as i128 - i128::from(right.exponent);
    match left_top.cmp(&right_top) {
        Ordering::Equal => {
            let count = left_len.max(right_len);
            for distance in 0..count {
                let left_bit = left_len
                    .checked_sub(distance + 1)
                    .is_some_and(|index| left_magnitude.bit(index));
                let right_bit = right_len
                    .checked_sub(distance + 1)
                    .is_some_and(|index| right_magnitude.bit(index));
                match left_bit.cmp(&right_bit) {
                    Ordering::Equal => {}
                    ordering => return ordering,
                }
            }
            Ordering::Equal
        }
        ordering => ordering,
    }
}

fn maximum_finite() -> Result<Dyadic, BigintError> {
    Dyadic::from_f64_exact(f64::MAX)
}

fn magnitude_to_u64_shifted(magnitude: &BigUint, shift: usize) -> u64 {
    let mut result = 0_u64;
    let bit_len = magnitude.bit_len();
    for index in 0..bit_len {
        if magnitude.bit(index) {
            result |= 1_u64 << (index + shift);
        }
    }
    result
}

fn round_right_to_u64(magnitude: &BigUint, drop: usize) -> u64 {
    let bit_len = magnitude.bit_len();
    let kept = bit_len.saturating_sub(drop).min(64);
    let mut result = 0_u64;
    for index in 0..kept {
        if magnitude.bit(index + drop) {
            result |= 1_u64 << index;
        }
    }
    if drop == 0 || !magnitude.bit(drop - 1) {
        return result;
    }
    let below_half = (0..drop - 1).any(|index| magnitude.bit(index));
    if below_half || result & 1 == 1 {
        result += 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integer(value: i64) -> BigInt {
        BigInt::try_from(value).unwrap()
    }

    #[test]
    fn normalizes_and_computes_arithmetic() {
        let half = Dyadic::new(integer(4), 3);
        assert_eq!(half.integer(), &integer(1));
        assert_eq!(half.exponent(), 1);

        let three_halves = Dyadic::new(integer(3), 1);
        assert_eq!(half.add(&three_halves).unwrap(), Dyadic::new(integer(2), 0));
        assert_eq!(three_halves.sub(&half).unwrap(), Dyadic::new(integer(1), 0));
        assert_eq!(half.mul(&three_halves).unwrap(), Dyadic::new(integer(3), 2));
        assert_eq!(
            half.midpoint(&three_halves).unwrap(),
            Dyadic::new(integer(1), 0)
        );
    }

    #[test]
    fn orders_without_expanding_exponents() {
        let negative = Dyadic::new(integer(-1), u32::MAX);
        let zero = Dyadic::new(integer(0), 12);
        let positive = Dyadic::new(integer(1), u32::MAX);
        assert!(negative < zero);
        assert!(zero < positive);
        assert!(Dyadic::new(integer(1), 1) < Dyadic::new(integer(3), 2));
        assert_eq!(Dyadic::new(integer(2), 2), Dyadic::new(integer(1), 1));
    }

    #[test]
    fn finite_float_roundtrip_samples() {
        let patterns = [
            0_u64,
            1,
            (1_u64 << 52) - 1,
            1_u64 << 52,
            0x3ff0_0000_0000_0000,
            0x3ff0_0000_0000_0001,
            0x7fef_ffff_ffff_ffff,
            0x8000_0000_0000_0000,
            0x8000_0000_0000_0001,
            0xffef_ffff_ffff_ffff,
        ];
        for pattern in patterns {
            let expected = if pattern == 0x8000_0000_0000_0000 {
                0
            } else {
                pattern
            };
            let dyadic = Dyadic::from_f64_exact(f64::from_bits(pattern)).unwrap();
            assert_eq!(dyadic.round_to_f64_ties_even().unwrap().to_bits(), expected);
        }
    }

    #[test]
    fn finite_float_roundtrip_deterministic_patterns() {
        let mut state = 0x6a09_e667_f3bc_c909_u64;
        let mut accepted = 0_usize;
        while accepted < 100_000 {
            state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut pattern = state;
            pattern = (pattern ^ (pattern >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            pattern = (pattern ^ (pattern >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            pattern ^= pattern >> 31;
            if pattern & 0x7ff0_0000_0000_0000 == 0x7ff0_0000_0000_0000 {
                continue;
            }
            let expected = if pattern == 0x8000_0000_0000_0000 {
                0
            } else {
                pattern
            };
            let dyadic = Dyadic::from_f64_exact(f64::from_bits(pattern)).unwrap();
            assert_eq!(dyadic.round_to_f64_ties_even().unwrap().to_bits(), expected);
            accepted += 1;
        }
    }

    #[test]
    fn rounds_halfway_to_even_and_zero_is_positive() {
        let one_and_half_ulp = Dyadic::new(integer((1_i64 << 53) + 1), 53);
        assert_eq!(
            one_and_half_ulp.round_to_f64_ties_even().unwrap().to_bits(),
            1.0_f64.to_bits()
        );

        let half_minimum_subnormal = Dyadic::new(integer(-1), 1075);
        assert_eq!(
            half_minimum_subnormal
                .round_to_f64_ties_even()
                .unwrap()
                .to_bits(),
            0
        );
        let above_half = Dyadic::new(integer(-3), 1076);
        assert_eq!(
            above_half.round_to_f64_ties_even().unwrap().to_bits(),
            (1_u64 << 63) | 1
        );
    }

    #[test]
    fn validates_enclosure_and_float_domain() {
        assert_eq!(
            DyadicEnclosure::new(Dyadic::new(integer(1), 0), Dyadic::new(integer(0), 0)),
            Err(BigintError::InvalidInterval)
        );
        assert_eq!(
            Dyadic::from_f64_exact(f64::INFINITY),
            Err(BigintError::NonFiniteFloat)
        );

        let maximum = Dyadic::from_f64_exact(f64::MAX).unwrap();
        let outside = maximum.add(&Dyadic::new(integer(1), 0)).unwrap();
        assert_eq!(
            outside.round_to_f64_ties_even(),
            Err(BigintError::FloatOutOfRange)
        );
    }
}
