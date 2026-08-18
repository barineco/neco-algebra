use alloc::vec::Vec;
use core::cmp::Ordering;

use crate::BigintError;

#[derive(Debug, Eq, PartialEq)]
pub struct BigUint {
    limbs: Vec<u32>,
}

fn checked_limb_count(count: usize) -> Result<usize, BigintError> {
    if count <= BigUint::MAX_LIMBS {
        Ok(count)
    } else {
        Err(BigintError::CapacityOverflow)
    }
}

fn checked_result_limb_count(
    common: usize,
    extra: usize,
    maximum: usize,
) -> Result<usize, BigintError> {
    let required = common
        .checked_add(extra)
        .ok_or(BigintError::CapacityOverflow)?;
    if required <= maximum {
        Ok(required)
    } else {
        Err(BigintError::CapacityOverflow)
    }
}

fn reserve_limbs_with<F, E>(
    limbs: &mut Vec<u32>,
    total_required: usize,
    mut reserve_fn: F,
) -> Result<(), BigintError>
where
    F: FnMut(&mut Vec<u32>, usize) -> Result<(), E>,
{
    checked_limb_count(total_required)?;
    if total_required > limbs.capacity() {
        let additional = total_required
            .checked_sub(limbs.len())
            .ok_or(BigintError::CapacityOverflow)?;
        reserve_fn(limbs, additional).map_err(|_| BigintError::AllocationFailure {
            requested_limbs: total_required,
        })?;
    }
    Ok(())
}

fn reserve_limbs(limbs: &mut Vec<u32>, total_required: usize) -> Result<(), BigintError> {
    reserve_limbs_with(limbs, total_required, |values, additional| {
        values.try_reserve(additional)
    })
}

fn addition_carry(left: &[u32], right: &[u32], common: usize) -> u64 {
    let mut carry = 0_u64;
    for index in 0..common {
        let sum = u64::from(limb_at(left, index)) + u64::from(limb_at(right, index)) + carry;
        carry = sum >> 32;
    }
    carry
}

fn subtraction_result_limb_count(left: &[u32], right: &[u32]) -> usize {
    let mut borrow = 0_u64;
    let mut required = 0;
    for (index, left) in left.iter().copied().enumerate() {
        let right = u64::from(limb_at(right, index)) + borrow;
        let (value, next_borrow) = if u64::from(left) >= right {
            (u64::from(left) - right, 0)
        } else {
            ((1_u64 << 32) + u64::from(left) - right, 1)
        };
        if value != 0 {
            required = index + 1;
        }
        borrow = next_borrow;
    }
    required
}

fn multiplication_result_limb_count(left: &[u32], right: &[u32]) -> Result<usize, BigintError> {
    let minimum = left
        .len()
        .checked_add(right.len())
        .and_then(|count| count.checked_sub(1))
        .ok_or(BigintError::CapacityOverflow)?;
    checked_limb_count(minimum)?;
    let mut carry = 0_u128;
    for diagonal in 0..minimum {
        let first_left = diagonal.saturating_sub(right.len() - 1);
        let last_left = diagonal.min(left.len() - 1);
        let mut sum = carry;
        for (left_index, left_limb) in left
            .iter()
            .copied()
            .enumerate()
            .take(last_left + 1)
            .skip(first_left)
        {
            let right_index = diagonal - left_index;
            let product = u128::from(left_limb) * u128::from(right[right_index]);
            sum = sum
                .checked_add(product)
                .ok_or(BigintError::CapacityOverflow)?;
        }
        carry = sum >> 32;
    }
    checked_result_limb_count(minimum, usize::from(carry != 0), BigUint::MAX_LIMBS)
}

impl BigUint {
    pub const MAX_LIMBS: usize = usize::MAX / 32;

    pub fn zero() -> Self {
        Self { limbs: Vec::new() }
    }

    pub fn one() -> Result<Self, BigintError> {
        Self::from_limbs_le(vec_with_one(1)?)
    }

    pub fn from_limbs_le(mut limbs: Vec<u32>) -> Result<Self, BigintError> {
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
        checked_limb_count(limbs.len())?;
        Ok(Self { limbs })
    }

    pub fn limbs_le(&self) -> &[u32] {
        &self.limbs
    }

    pub fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    pub fn bit_len(&self) -> usize {
        match self.limbs.last() {
            Some(high) => (self.limbs.len() - 1) * 32 + (32 - high.leading_zeros()) as usize,
            None => 0,
        }
    }

    pub fn bit(&self, index: usize) -> bool {
        let limb = index / 32;
        self.limbs
            .get(limb)
            .is_some_and(|value| value & (1_u32 << (index % 32)) != 0)
    }

    pub fn to_u32(&self) -> Option<u32> {
        match self.limbs.as_slice() {
            [] => Some(0),
            [value] => Some(*value),
            _ => None,
        }
    }

    pub fn try_clone(&self) -> Result<Self, BigintError> {
        let mut limbs = Vec::new();
        reserve_limbs(&mut limbs, self.limbs.len())?;
        limbs.extend_from_slice(&self.limbs);
        Ok(Self { limbs })
    }

    pub fn add(&self, rhs: &Self) -> Result<Self, BigintError> {
        if self.is_zero() {
            return rhs.try_clone();
        }
        if rhs.is_zero() {
            return self.try_clone();
        }
        let common = self.limbs.len().max(rhs.limbs.len());
        checked_limb_count(common)?;
        let final_carry = addition_carry(&self.limbs, &rhs.limbs, common);
        let result_required =
            checked_result_limb_count(common, usize::from(final_carry != 0), usize::MAX)?;
        let mut limbs = Vec::new();
        checked_limb_count(result_required)?;
        reserve_limbs(&mut limbs, result_required)?;
        let mut carry = 0_u64;
        for index in 0..common {
            let left = u64::from(limb_at(&self.limbs, index));
            let right = u64::from(limb_at(&rhs.limbs, index));
            let sum = left + right + carry;
            limbs.push(sum as u32);
            carry = sum >> 32;
        }
        if carry != 0 {
            limbs.push(carry as u32);
        }
        Self::from_limbs_le(limbs)
    }

    pub fn checked_sub(&self, rhs: &Self) -> Result<Self, BigintError> {
        if self < rhs {
            return Err(BigintError::UnsignedUnderflow);
        }
        let result_required = subtraction_result_limb_count(&self.limbs, &rhs.limbs);
        let mut limbs = Vec::new();
        reserve_limbs(&mut limbs, result_required)?;
        let mut borrow = 0_u64;
        for index in 0..self.limbs.len() {
            let left = u64::from(self.limbs[index]);
            let right = u64::from(limb_at(&rhs.limbs, index)) + borrow;
            let (value, next_borrow) = if left >= right {
                (left - right, 0)
            } else {
                ((1_u64 << 32) + left - right, 1)
            };
            limbs.push(value as u32);
            borrow = next_borrow;
        }
        Self::from_limbs_le(limbs)
    }

    pub fn mul(&self, rhs: &Self) -> Result<Self, BigintError> {
        if self.is_zero() || rhs.is_zero() {
            return Ok(Self::zero());
        }
        let _term_count = self
            .limbs
            .len()
            .checked_mul(rhs.limbs.len())
            .ok_or(BigintError::CapacityOverflow)?;
        let result_required = multiplication_result_limb_count(&self.limbs, &rhs.limbs)?;
        let mut limbs = Vec::new();
        reserve_limbs(&mut limbs, result_required)?;
        limbs.resize(result_required, 0);
        for (left_index, left) in self.limbs.iter().copied().enumerate() {
            let mut carry = 0_u64;
            for (right_index, right) in rhs.limbs.iter().copied().enumerate() {
                let index = left_index + right_index;
                let product = u64::from(left) * u64::from(right) + u64::from(limbs[index]) + carry;
                limbs[index] = product as u32;
                carry = product >> 32;
            }
            let carry_index = left_index + rhs.limbs.len();
            if carry_index < limbs.len() {
                limbs[carry_index] = carry as u32;
            } else if carry != 0 {
                return Err(BigintError::CapacityOverflow);
            }
        }
        Self::from_limbs_le(limbs)
    }

    pub fn shl_bits(&self, bits: usize) -> Result<Self, BigintError> {
        if self.is_zero() {
            return Ok(Self::zero());
        }
        let limb_shift = bits / 32;
        let bit_shift = bits % 32;
        let extra = usize::from(
            bit_shift != 0
                && self.limbs.last().expect("nonzero value has a limb") >> (32 - bit_shift) != 0,
        );
        let required = self
            .limbs
            .len()
            .checked_add(limb_shift)
            .and_then(|count| count.checked_add(extra))
            .ok_or(BigintError::CapacityOverflow)?;
        checked_limb_count(required)?;
        let mut limbs = Vec::new();
        reserve_limbs(&mut limbs, required)?;
        limbs.resize(limb_shift, 0);
        let mut carry = 0_u64;
        for limb in self.limbs.iter().copied() {
            let value = (u64::from(limb) << bit_shift) | carry;
            limbs.push(value as u32);
            carry = value >> 32;
        }
        if carry != 0 {
            limbs.push(carry as u32);
        }
        Self::from_limbs_le(limbs)
    }

    pub(crate) fn into_shr_bits(mut self, bits: usize) -> Self {
        let limb_shift = bits / 32;
        if limb_shift >= self.limbs.len() {
            self.limbs.clear();
            return self;
        }
        let bit_shift = bits % 32;
        if limb_shift != 0 {
            let remaining = self.limbs.len() - limb_shift;
            self.limbs.copy_within(limb_shift.., 0);
            self.limbs.truncate(remaining);
        }
        if bit_shift != 0 {
            let mut carry = 0_u32;
            for limb in self.limbs.iter_mut().rev() {
                let next_carry = *limb << (32 - bit_shift);
                *limb = (*limb >> bit_shift) | carry;
                carry = next_carry;
            }
        }
        while self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
        self
    }

    pub fn shr_bits(&self, bits: usize) -> Result<Self, BigintError> {
        let limb_shift = bits / 32;
        if limb_shift >= self.limbs.len() {
            return Ok(Self::zero());
        }
        let bit_shift = bits % 32;
        let required = self.limbs.len() - limb_shift;
        let mut limbs = Vec::new();
        reserve_limbs(&mut limbs, required)?;
        let mut carry = 0_u32;
        for index in (limb_shift..self.limbs.len()).rev() {
            let limb = self.limbs[index];
            let value = if bit_shift == 0 {
                limb
            } else {
                (limb >> bit_shift) | carry
            };
            limbs.push(value);
            carry = if bit_shift == 0 {
                0
            } else {
                limb << (32 - bit_shift)
            };
        }
        limbs.reverse();
        Self::from_limbs_le(limbs)
    }

    pub fn div_rem(&self, divisor: &Self) -> Result<(Self, Self), BigintError> {
        if divisor.is_zero() {
            return Err(BigintError::DivisionByZero);
        }
        if self < divisor {
            return Ok((Self::zero(), self.try_clone()?));
        }
        let shift = self
            .bit_len()
            .checked_sub(divisor.bit_len())
            .ok_or(BigintError::CapacityOverflow)?;
        let _iterations = shift.checked_add(1).ok_or(BigintError::CapacityOverflow)?;
        let mut quotient_limbs = Vec::new();
        let quotient_required = shift / 32 + 1;
        reserve_limbs(&mut quotient_limbs, quotient_required)?;
        quotient_limbs.resize(quotient_required, 0);
        let mut remainder = self.try_clone()?;
        let mut shifted = divisor.shl_bits(shift)?;
        for position in (0..=shift).rev() {
            if remainder >= shifted {
                remainder = remainder.checked_sub(&shifted)?;
                quotient_limbs[position / 32] |= 1_u32 << (position % 32);
            }
            if position != 0 {
                shifted = shifted.shr_bits(1)?;
            }
        }
        Ok((Self::from_limbs_le(quotient_limbs)?, remainder))
    }

    pub fn exact_div(&self, divisor: &Self) -> Result<Self, BigintError> {
        let (quotient, remainder) = self.div_rem(divisor)?;
        if remainder.is_zero() {
            Ok(quotient)
        } else {
            Err(BigintError::NonExactDivision)
        }
    }

    pub fn gcd(&self, rhs: &Self) -> Result<Self, BigintError> {
        let mut left = self.try_clone()?;
        let mut right = rhs.try_clone()?;
        while !right.is_zero() {
            let (_, remainder) = left.div_rem(&right)?;
            left = right;
            right = remainder;
        }
        Ok(left)
    }

    pub fn lcm(&self, rhs: &Self) -> Result<Self, BigintError> {
        if self.is_zero() || rhs.is_zero() {
            return Ok(Self::zero());
        }
        self.exact_div(&self.gcd(rhs)?)?.mul(rhs)
    }

    pub fn pow_u32(&self, mut exponent: u32) -> Result<Self, BigintError> {
        let mut result = Self::one()?;
        let mut base = self.try_clone()?;
        while exponent != 0 {
            if exponent & 1 != 0 {
                result = result.mul(&base)?;
            }
            exponent >>= 1;
            if exponent != 0 {
                base = base.mul(&base)?;
            }
        }
        Ok(result)
    }
}

fn vec_with_one(value: u32) -> Result<Vec<u32>, BigintError> {
    let mut limbs = Vec::new();
    reserve_limbs(&mut limbs, 1)?;
    limbs.push(value);
    Ok(limbs)
}

fn limb_at(limbs: &[u32], index: usize) -> u32 {
    match limbs.get(index) {
        Some(value) => *value,
        None => 0,
    }
}

impl Ord for BigUint {
    fn cmp(&self, rhs: &Self) -> Ordering {
        self.limbs
            .len()
            .cmp(&rhs.limbs.len())
            .then_with(|| self.limbs.iter().rev().cmp(rhs.limbs.iter().rev()))
    }
}

impl PartialOrd for BigUint {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

macro_rules! impl_try_from_unsigned {
    ($($type:ty),* $(,)?) => {$ (
        impl TryFrom<$type> for BigUint {
            type Error = BigintError;

            fn try_from(value: $type) -> Result<Self, Self::Error> {
                let mut value = value as u128;
                let mut limbs = Vec::new();
                let required = if value == 0 {
                    0
                } else {
                    ((u128::BITS - value.leading_zeros()) as usize).div_ceil(32)
                };
                reserve_limbs(&mut limbs, required)?;
                while value != 0 {
                    limbs.push(value as u32);
                    value >>= 32;
                }
                Ok(Self { limbs })
            }
        }
    )* };
}

impl_try_from_unsigned!(u8, u16, u32, u64, usize);

#[cfg(test)]
mod tests {
    use super::{
        checked_result_limb_count, multiplication_result_limb_count, reserve_limbs_with,
        subtraction_result_limb_count, BigUint,
    };
    use crate::BigintError;
    use alloc::vec::Vec;

    #[test]
    fn reserve_failure_reports_total_required() {
        let mut limbs = Vec::new();
        let result = reserve_limbs_with(&mut limbs, 7, |_, _| Err(()));
        assert_eq!(
            result,
            Err(BigintError::AllocationFailure { requested_limbs: 7 })
        );
    }

    #[test]
    fn required_length_accepts_a_result_at_the_virtual_limit() {
        assert_eq!(checked_result_limb_count(3, 0, 3), Ok(3));
        assert_eq!(
            checked_result_limb_count(3, 1, 3),
            Err(BigintError::CapacityOverflow)
        );
        assert_eq!(checked_result_limb_count(2, 1, 3), Ok(3));
    }

    #[test]
    fn arithmetic_prepass_reports_normalized_result_lengths() {
        assert_eq!(subtraction_result_limb_count(&[0, 0, 1], &[1]), 2);
        assert_eq!(subtraction_result_limb_count(&[7], &[7]), 0);
        assert_eq!(multiplication_result_limb_count(&[1], &[1]), Ok(1));
        assert_eq!(multiplication_result_limb_count(&[1, 1], &[1, 1]), Ok(3));
        assert_eq!(
            multiplication_result_limb_count(&[u32::MAX, u32::MAX], &[u32::MAX, u32::MAX]),
            Ok(4)
        );
    }

    #[test]
    fn division_vector() {
        let seven = BigUint::try_from(7_u32).unwrap();
        let three = BigUint::try_from(3_u32).unwrap();
        let (quotient, remainder) = seven.div_rem(&three).unwrap();
        assert_eq!(quotient.to_u32(), Some(2));
        assert_eq!(remainder.to_u32(), Some(1));
    }
}
