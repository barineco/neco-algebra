use neco_bigint::{BigintError, Dyadic};

use crate::bounds::next_up_nonnegative;
use crate::types::{CertifiedFrequency, UndeterminableReason};

const TWO_PI: f64 = 2.0 * core::f64::consts::PI;

#[derive(Debug)]
pub(crate) enum FrequencyCertificationError {
    Undeterminable(UndeterminableReason),
    ExactArithmetic(BigintError),
}

impl From<BigintError> for FrequencyCertificationError {
    fn from(error: BigintError) -> Self {
        Self::ExactArithmetic(error)
    }
}

pub(crate) fn certified_frequency(
    mu: f64,
    delta_exact: &Dyadic,
) -> Result<CertifiedFrequency, FrequencyCertificationError> {
    if mu.is_nan() || mu < 0.0 {
        return Err(FrequencyCertificationError::Undeterminable(
            UndeterminableReason::NegativeEigenvalue,
        ));
    }
    let mu_dyadic = Dyadic::from_f64_exact(mu)?;
    let lower_arg_dyadic = mu_dyadic.sub(delta_exact)?;
    let lower_arg = round_toward_negative_infinity(&lower_arg_dyadic)?;
    if lower_arg < 0.0 {
        return Err(FrequencyCertificationError::Undeterminable(
            UndeterminableReason::EnclosureLowerBoundBelowZero {
                lower_bound: lower_arg,
            },
        ));
    }
    let upper_arg_dyadic = mu_dyadic.add(delta_exact)?;
    let upper_arg = round_toward_positive_infinity(&upper_arg_dyadic)?;

    let lower_sqrt = down(lower_arg.sqrt());
    let upper_sqrt = up(upper_arg.sqrt());
    let lower_divisor = next_up_nonnegative(TWO_PI);
    let lower_bound = require_finite(down(lower_sqrt / lower_divisor))?;
    let upper_bound = require_finite(up(upper_sqrt / TWO_PI))?;

    Ok(CertifiedFrequency {
        value: mu.sqrt() / TWO_PI,
        lower_bound,
        upper_bound,
    })
}

fn require_finite(bound: f64) -> Result<f64, FrequencyCertificationError> {
    if bound.is_finite() {
        Ok(bound)
    } else {
        Err(FrequencyCertificationError::Undeterminable(
            UndeterminableReason::NonFiniteFrequencyBound,
        ))
    }
}

fn round_toward_negative_infinity(exact: &Dyadic) -> Result<f64, BigintError> {
    let nearest = exact.round_to_f64_ties_even()?;
    if Dyadic::from_f64_exact(nearest)? > *exact {
        Ok(step_toward_negative_infinity(nearest))
    } else {
        Ok(nearest)
    }
}

fn round_toward_positive_infinity(exact: &Dyadic) -> Result<f64, BigintError> {
    let nearest = exact.round_to_f64_ties_even()?;
    if Dyadic::from_f64_exact(nearest)? < *exact {
        Ok(step_toward_positive_infinity(nearest))
    } else {
        Ok(nearest)
    }
}

fn step_toward_negative_infinity(value: f64) -> f64 {
    if value == 0.0 {
        -f64::from_bits(1)
    } else if value > 0.0 {
        f64::from_bits(value.to_bits() - 1)
    } else {
        f64::from_bits(value.to_bits() + 1)
    }
}

fn step_toward_positive_infinity(value: f64) -> f64 {
    if value == 0.0 {
        f64::from_bits(1)
    } else if value > 0.0 {
        f64::from_bits(value.to_bits() + 1)
    } else {
        f64::from_bits(value.to_bits() - 1)
    }
}

fn down(value: f64) -> f64 {
    if value <= 0.0 {
        0.0
    } else {
        f64::from_bits(value.to_bits() - 1)
    }
}

fn up(value: f64) -> f64 {
    f64::from_bits(value.to_bits() + 1)
}

#[cfg(test)]
mod tests {
    use super::{certified_frequency, TWO_PI};
    use crate::types::UndeterminableReason;
    use neco_bigint::Dyadic;

    fn dyadic(value: f64) -> Dyadic {
        Dyadic::from_f64_exact(value).expect("finite value")
    }

    #[test]
    fn certified_frequency_rounds_outward_to_preserve_containment() {
        let mu = 271505.70483191794;
        let delta_exact = dyadic(99.49179087581372);
        let frequency = certified_frequency(mu, &delta_exact).expect("frequency");
        let naive = (mu - 99.49179087581372).sqrt() / TWO_PI;
        assert!(naive > 82.91441182509057);
        assert!(frequency.lower_bound() <= 82.91441182509057);

        let mu = 66470.1934083648;
        let delta_exact = dyadic(52.48569990951799);
        let frequency = certified_frequency(mu, &delta_exact).expect("frequency");
        let naive = (mu + 52.48569990951799).sqrt() / TWO_PI;
        assert!(naive < 41.04922833108367);
        assert!(frequency.upper_bound() >= 41.04922833108367);
    }

    #[test]
    fn certified_frequency_rejects_negative_lower_bound_and_negative_eigenvalue() {
        let boundary = certified_frequency(0.5, &dyadic(0.5)).expect("boundary");
        assert_eq!(boundary.lower_bound(), 0.0);

        match certified_frequency(0.5, &dyadic(0.8)) {
            Err(super::FrequencyCertificationError::Undeterminable(
                UndeterminableReason::EnclosureLowerBoundBelowZero { lower_bound },
            )) => assert_eq!(lower_bound, 0.5 - 0.8),
            other => panic!("expected EnclosureLowerBoundBelowZero, got {other:?}"),
        }

        assert!(matches!(
            certified_frequency(-1.0, &dyadic(0.0)),
            Err(super::FrequencyCertificationError::Undeterminable(
                UndeterminableReason::NegativeEigenvalue
            ))
        ));
    }

    #[test]
    fn require_finite_turns_infinity_and_nan_into_a_typed_undeterminable_reason() {
        assert_eq!(super::require_finite(1.0).expect("finite"), 1.0);
        assert!(matches!(
            super::require_finite(f64::INFINITY),
            Err(super::FrequencyCertificationError::Undeterminable(
                UndeterminableReason::NonFiniteFrequencyBound
            ))
        ));
        assert!(matches!(
            super::require_finite(f64::NEG_INFINITY),
            Err(super::FrequencyCertificationError::Undeterminable(
                UndeterminableReason::NonFiniteFrequencyBound
            ))
        ));
        assert!(matches!(
            super::require_finite(f64::NAN),
            Err(super::FrequencyCertificationError::Undeterminable(
                UndeterminableReason::NonFiniteFrequencyBound
            ))
        ));
    }

    #[test]
    fn certified_frequency_stays_finite_for_an_eigenvalue_near_the_f64_maximum() {
        let frequency =
            certified_frequency(f64::MAX, &dyadic(0.0)).expect("frequency near the maximum");
        assert!(frequency.value().is_finite());
        assert!(frequency.lower_bound().is_finite());
        assert!(frequency.upper_bound().is_finite());
    }

    #[test]
    fn certified_frequency_rejects_an_exact_sum_beyond_f64_max_before_rounding() {
        assert!(matches!(
            certified_frequency(f64::MAX, &dyadic(1.0e-300)),
            Err(super::FrequencyCertificationError::ExactArithmetic(
                neco_bigint::BigintError::FloatOutOfRange
            ))
        ));
    }
}
