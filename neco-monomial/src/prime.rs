use neco_bigint::{BigUint, BigintError};

use crate::MonomialErrorKind;

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProvenPrime {
    value: BigUint,
}

impl ProvenPrime {
    pub fn value(&self) -> &BigUint {
        &self.value
    }

    pub(crate) fn value_internal(&self) -> &BigUint {
        &self.value
    }

    pub fn try_clone(&self) -> Result<Self, MonomialErrorKind> {
        self.try_clone_internal()
    }

    pub(crate) fn try_clone_internal(&self) -> Result<Self, MonomialErrorKind> {
        Ok(Self {
            value: self.value.try_clone().map_err(MonomialErrorKind::Bigint)?,
        })
    }
}

pub(crate) fn factor_biguint<F>(value: &BigUint, mut emit: F) -> Result<(), MonomialErrorKind>
where
    F: FnMut(ProvenPrime, BigUint) -> Result<(), MonomialErrorKind>,
{
    let one = BigUint::one().map_err(MonomialErrorKind::Bigint)?;
    if value <= &one {
        return Ok(());
    }

    let mut remainder = value.try_clone().map_err(MonomialErrorKind::Bigint)?;
    let mut candidate = BigUint::try_from(2_u8).map_err(MonomialErrorKind::Bigint)?;
    let mut valuation = BigUint::zero();

    loop {
        if remainder == one {
            emit_valuation(&candidate, &mut valuation, &mut emit)?;
            return Ok(());
        }

        let (quotient, division_remainder) = remainder
            .div_rem(&candidate)
            .map_err(MonomialErrorKind::Bigint)?;
        if candidate > quotient {
            if remainder == candidate {
                valuation = valuation.add(&one).map_err(MonomialErrorKind::Bigint)?;
                emit_valuation(&candidate, &mut valuation, &mut emit)?;
                return Ok(());
            }
            emit_valuation(&candidate, &mut valuation, &mut emit)?;
            emit(ProvenPrime { value: remainder }, one)?;
            return Ok(());
        }

        if division_remainder.is_zero() {
            remainder = quotient;
            valuation = valuation.add(&one).map_err(MonomialErrorKind::Bigint)?;
            continue;
        }

        emit_valuation(&candidate, &mut valuation, &mut emit)?;
        candidate = if candidate.to_u32() == Some(2) {
            BigUint::try_from(3_u8).map_err(MonomialErrorKind::Bigint)?
        } else {
            candidate
                .add(&BigUint::try_from(2_u8).map_err(MonomialErrorKind::Bigint)?)
                .map_err(MonomialErrorKind::Bigint)?
        };
    }
}

fn emit_valuation<F>(
    candidate: &BigUint,
    valuation: &mut BigUint,
    emit: &mut F,
) -> Result<(), MonomialErrorKind>
where
    F: FnMut(ProvenPrime, BigUint) -> Result<(), MonomialErrorKind>,
{
    if valuation.is_zero() {
        return Ok(());
    }
    let prime = ProvenPrime {
        value: candidate.try_clone().map_err(MonomialErrorKind::Bigint)?,
    };
    let emitted_valuation = core::mem::replace(valuation, BigUint::zero());
    emit(prime, emitted_valuation)
}

pub(crate) fn pow_biguint_with<F>(
    base: &BigUint,
    exponent: &BigUint,
    mut multiply: F,
) -> Result<BigUint, BigintError>
where
    F: FnMut(&BigUint, &BigUint) -> Result<BigUint, BigintError>,
{
    let mut result = BigUint::one()?;
    if exponent.is_zero() {
        return Ok(result);
    }

    let mut current_power = base.try_clone()?;
    let bit_len = exponent.bit_len();
    for bit_index in 0..bit_len {
        if exponent.bit(bit_index) {
            result = multiply(&result, &current_power)?;
        }
        if bit_index + 1 < bit_len {
            current_power = multiply(&current_power, &current_power)?;
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{factor_biguint, pow_biguint_with};
    use crate::MonomialErrorKind;
    use alloc::vec::Vec;
    use neco_bigint::BigUint;

    fn factor_vector(value: u32) -> Vec<(u32, u32)> {
        let value = BigUint::try_from(value).unwrap();
        let mut factors = Vec::new();
        factor_biguint(&value, |prime, valuation| {
            factors.push((prime.value().to_u32().unwrap(), valuation.to_u32().unwrap()));
            Ok(())
        })
        .unwrap();
        factors
    }

    #[test]
    fn trial_division_vectors_cover_all_state_transitions() {
        assert_eq!(factor_vector(1), []);
        assert_eq!(factor_vector(2_u32.pow(12)), [(2, 12)]);
        assert_eq!(factor_vector(3_u32.pow(8)), [(3, 8)]);
        assert_eq!(factor_vector(49), [(7, 2)]);
        assert_eq!(factor_vector(77), [(7, 1), (11, 1)]);
        assert_eq!(factor_vector(97), [(97, 1)]);
        assert_eq!(factor_vector(5 * 101), [(5, 1), (101, 1)]);
    }

    #[test]
    fn callback_failure_stops_the_stream() {
        let value = BigUint::try_from(6_u8).unwrap();
        let mut calls = 0;
        let result = factor_biguint(&value, |_, _| {
            calls += 1;
            Err(MonomialErrorKind::DivisionByZero)
        });
        assert_eq!(result, Err(MonomialErrorKind::DivisionByZero));
        assert_eq!(calls, 1);
    }

    #[test]
    fn arbitrary_precision_exponent_visits_all_bits() {
        let base = BigUint::try_from(2_u8).unwrap();
        let two_to_31 = BigUint::one().unwrap().shl_bits(31).unwrap();
        let mut calls = Vec::new();
        let result = pow_biguint_with(&base, &two_to_31, |left, right| {
            calls.push((left.to_u32(), right.to_u32()));
            right.try_clone()
        })
        .unwrap();
        assert_eq!(result.to_u32(), Some(2));
        assert_eq!(calls.len(), 32);
        assert_eq!(calls[..31], [(Some(2), Some(2)); 31]);
        assert_eq!(calls[31], (Some(1), Some(2)));

        let two_to_31_plus_one = two_to_31.add(&BigUint::one().unwrap()).unwrap();
        calls.clear();
        let result = pow_biguint_with(&base, &two_to_31_plus_one, |left, right| {
            calls.push((left.to_u32(), right.to_u32()));
            right.try_clone()
        })
        .unwrap();
        assert_eq!(result.to_u32(), Some(2));
        assert_eq!(calls.len(), 33);
        assert_eq!(calls[0], (Some(1), Some(2)));
        assert_eq!(calls[1..32], [(Some(2), Some(2)); 31]);
        assert_eq!(calls[32], (Some(2), Some(2)));
    }

    #[test]
    fn zero_exponent_returns_one_without_multiplication() {
        let base = BigUint::try_from(19_u8).unwrap();
        let result = pow_biguint_with(&base, &BigUint::zero(), |_, _| {
            panic!("zero exponent must not multiply")
        })
        .unwrap();
        assert_eq!(result, BigUint::one().unwrap());
    }
}
