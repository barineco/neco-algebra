use core::cmp::{max, min};

use neco_bigint::{BigInt, BigUint, BigintError, Dyadic, DyadicEnclosure, Sign};
use neco_monomial::RadicalBasis;

use crate::error::FormSumErrorKind;
use crate::formsum::FormSum;

impl FormSum {
    pub fn enclose(&self, bits: u32) -> Result<DyadicEnclosure, FormSumErrorKind> {
        if self.is_zero() {
            let zero = Dyadic::new(BigInt::zero(), 0);
            return Ok(DyadicEnclosure::new(zero, Dyadic::new(BigInt::zero(), 0))?);
        }
        let target = Dyadic::new(BigInt::one()?, bits);
        let mut precision = 0_u32;
        loop {
            let enclosure = enclose_at(self, precision)?;
            if enclosure.width()? <= target {
                return Ok(enclosure);
            }
            precision = precision.checked_add(1).ok_or_else(exponent_overflow)?;
        }
    }

    pub fn sign(&self) -> Result<Sign, FormSumErrorKind> {
        if self.is_zero() {
            return Ok(Sign::Zero);
        }
        let zero = Dyadic::new(BigInt::zero(), 0);
        let mut precision = 0_u32;
        loop {
            let enclosure = enclose_at(self, precision)?;
            if enclosure.lower() > &zero {
                return Ok(Sign::Positive);
            }
            if enclosure.upper() < &zero {
                return Ok(Sign::Negative);
            }
            precision = precision.checked_add(1).ok_or_else(exponent_overflow)?;
        }
    }
}

fn enclose_at(value: &FormSum, precision: u32) -> Result<DyadicEnclosure, FormSumErrorKind> {
    let mut sum = interval_integer(0)?;
    for (basis, coefficient) in value.terms() {
        let coefficient_interval = DyadicEnclosure::new(
            coefficient.dyadic_floor(precision)?,
            coefficient.dyadic_ceil(precision)?,
        )?;
        let term = multiply_intervals(&coefficient_interval, &enclose_basis(basis, precision)?)?;
        sum = add_intervals(&sum, &term)?;
    }
    Ok(sum)
}

fn enclose_basis(
    basis: &RadicalBasis,
    precision: u32,
) -> Result<DyadicEnclosure, FormSumErrorKind> {
    let mut result = interval_integer(1)?;
    for (prime, exponent) in basis.factors() {
        let root = root_floor(
            prime.value(),
            exponent.numerator().magnitude(),
            exponent.denominator(),
            precision,
        )?;
        let upper = root.add(&BigUint::one()?)?;
        let factor = DyadicEnclosure::new(
            Dyadic::new(BigInt::from_sign_magnitude(Sign::Positive, root), precision),
            Dyadic::new(
                BigInt::from_sign_magnitude(Sign::Positive, upper),
                precision,
            ),
        )?;
        result = multiply_intervals(&result, &factor)?;
    }
    Ok(result)
}

fn root_floor(
    prime: &BigUint,
    numerator: &BigUint,
    denominator: &BigUint,
    precision: u32,
) -> Result<BigUint, FormSumErrorKind> {
    let target_power = pow_biguint(prime, numerator)?;
    let shift = denominator.mul(&BigUint::try_from(precision)?)?;
    let shift =
        uint_to_usize(&shift).ok_or(FormSumErrorKind::Bigint(BigintError::CapacityOverflow))?;
    let target = shift_left(&target_power, shift)?;
    let mut lower = BigUint::zero();
    let mut upper = prime.shl_bits(precision as usize)?;
    let one = BigUint::one()?;
    while upper.checked_sub(&lower)? > one {
        let middle = lower.add(&upper)?.shr_bits(1)?;
        if pow_biguint(&middle, denominator)? <= target {
            lower = middle;
        } else {
            upper = middle;
        }
    }
    Ok(lower)
}

fn shift_left(value: &BigUint, shift: usize) -> Result<BigUint, BigintError> {
    #[cfg(test)]
    if let Some(requested_limbs) = injected_shift_allocation() {
        return Err(BigintError::AllocationFailure { requested_limbs });
    }
    value.shl_bits(shift)
}

#[cfg(test)]
std::thread_local! {
    static SHIFT_ALLOCATION_FAILURE: core::cell::Cell<Option<usize>> = const { core::cell::Cell::new(None) };
}

#[cfg(test)]
fn injected_shift_allocation() -> Option<usize> {
    SHIFT_ALLOCATION_FAILURE.with(core::cell::Cell::get)
}

fn pow_biguint(base: &BigUint, exponent: &BigUint) -> Result<BigUint, FormSumErrorKind> {
    let mut result = BigUint::one()?;
    if exponent.is_zero() {
        return Ok(result);
    }
    let mut power = base.try_clone()?;
    for bit in 0..exponent.bit_len() {
        if exponent.bit(bit) {
            result = result.mul(&power)?;
        }
        if bit + 1 < exponent.bit_len() {
            power = power.mul(&power)?;
        }
    }
    Ok(result)
}

fn interval_integer(value: i32) -> Result<DyadicEnclosure, FormSumErrorKind> {
    let integer = BigInt::try_from(value)?;
    let lower = Dyadic::new(integer.try_clone()?, 0);
    Ok(DyadicEnclosure::new(lower, Dyadic::new(integer, 0))?)
}

fn add_intervals(
    left: &DyadicEnclosure,
    right: &DyadicEnclosure,
) -> Result<DyadicEnclosure, FormSumErrorKind> {
    Ok(DyadicEnclosure::new(
        left.lower().add(right.lower())?,
        left.upper().add(right.upper())?,
    )?)
}

fn multiply_intervals(
    left: &DyadicEnclosure,
    right: &DyadicEnclosure,
) -> Result<DyadicEnclosure, FormSumErrorKind> {
    let ll = left.lower().mul(right.lower())?;
    let lu = left.lower().mul(right.upper())?;
    let ul = left.upper().mul(right.lower())?;
    let uu = left.upper().mul(right.upper())?;
    let lower = min(min(ll, lu), min(ul, uu));

    let ll = left.lower().mul(right.lower())?;
    let lu = left.lower().mul(right.upper())?;
    let ul = left.upper().mul(right.lower())?;
    let uu = left.upper().mul(right.upper())?;
    let upper = max(max(ll, lu), max(ul, uu));
    Ok(DyadicEnclosure::new(lower, upper)?)
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

fn exponent_overflow() -> FormSumErrorKind {
    let maximum = match BigUint::try_from(u32::MAX) {
        Ok(value) => value,
        Err(error) => return FormSumErrorKind::Bigint(error),
    };
    let one = match BigUint::one() {
        Ok(value) => value,
        Err(error) => return FormSumErrorKind::Bigint(error),
    };
    match maximum.add(&one) {
        Ok(required) => FormSumErrorKind::Bigint(BigintError::ExponentOverflow {
            required,
            maximum: u32::MAX,
        }),
        Err(error) => FormSumErrorKind::Bigint(error),
    }
}

#[cfg(test)]
mod tests {
    use neco_bigint::{BigUint, BigintError};

    use super::{exponent_overflow, root_floor, SHIFT_ALLOCATION_FAILURE};
    use crate::FormSumErrorKind;

    #[test]
    fn precision_overflow_preserves_the_required_exponent() {
        let maximum = BigUint::try_from(u32::MAX).unwrap();
        let required = maximum.add(&BigUint::one().unwrap()).unwrap();
        assert_eq!(
            exponent_overflow(),
            FormSumErrorKind::Bigint(BigintError::ExponentOverflow {
                required,
                maximum: u32::MAX,
            })
        );
    }

    #[test]
    fn root_shift_preserves_an_allocation_failure() {
        SHIFT_ALLOCATION_FAILURE.with(|failure| failure.set(Some(7)));
        let result = root_floor(
            &BigUint::try_from(2_u8).unwrap(),
            &BigUint::one().unwrap(),
            &BigUint::try_from(2_u8).unwrap(),
            1,
        );
        SHIFT_ALLOCATION_FAILURE.with(|failure| failure.set(None));
        assert_eq!(
            result.unwrap_err(),
            FormSumErrorKind::Bigint(BigintError::AllocationFailure { requested_limbs: 7 })
        );
    }

    #[test]
    fn root_floor_keeps_an_exact_middle_as_the_lower_endpoint() {
        assert_eq!(
            root_floor(
                &BigUint::try_from(4_u8).unwrap(),
                &BigUint::one().unwrap(),
                &BigUint::try_from(2_u8).unwrap(),
                0,
            )
            .unwrap(),
            BigUint::try_from(2_u8).unwrap()
        );
    }
}
