use neco_algnum::{AlgnumError, Polynomial, RealAlgebraic};
use neco_bigint::{BigInt, BigUint, BigintError, Dyadic, RawRational};
use neco_formsum::{DimensionResource, FormSumErrorKind, RawFormSum, RawTerm};
use neco_monomial::{RawMonomial, RawPower};

fn polynomial(values: &[i32]) -> Polynomial {
    Polynomial::from_coefficients(
        values
            .iter()
            .map(|v| BigInt::try_from(*v).unwrap())
            .collect(),
    )
}

fn dyadic(value: i32) -> Dyadic {
    Dyadic::new(BigInt::try_from(value).unwrap(), 0)
}

fn three_real_root_factor() -> neco_algnum::IrreduciblePolynomial {
    let mut factors = polynomial(&[1, -3, 0, 1])
        .candidate()
        .unwrap()
        .square_free()
        .unwrap()
        .factor()
        .unwrap();
    assert_eq!(factors.len(), 1);
    factors.pop().unwrap()
}

#[test]
fn candidate_invalid_inputs_return_the_declared_failure() {
    assert_eq!(
        Polynomial::zero().candidate(),
        Err(AlgnumError::ZeroPolynomial)
    );
    assert_eq!(
        polynomial(&[7]).candidate(),
        Err(AlgnumError::ZeroPolynomial)
    );
}

#[test]
fn public_root_certification_distinguishes_all_interval_failures() {
    let factor = three_real_root_factor();
    assert_eq!(
        factor.certify_root(dyadic(2), dyadic(-2)),
        Err(AlgnumError::InvalidIsolation)
    );
    assert_eq!(
        factor.certify_root(dyadic(3), dyadic(4)),
        Err(AlgnumError::NoTargetRoot)
    );
    assert_eq!(
        factor.certify_root(dyadic(-2), dyadic(2)),
        Err(AlgnumError::MultipleTargetRoots)
    );
}

#[test]
fn a_valid_constructed_interval_returns_the_same_root_on_refinement() {
    let factor = three_real_root_factor();
    let certified = factor.certify_root(dyadic(0), dyadic(1)).unwrap();
    assert_eq!(certified.value().root_index().get(), 1);
    let refined = certified.value().enclose(10).unwrap();
    assert_eq!(refined.value(), certified.value());
    assert!(refined.enclosure().width().unwrap() <= dyadic_width(10));
}

#[test]
fn reducible_square_free_endpoint_root_precedes_multiple_internal_roots() {
    let square_free = polynomial(&[0, 2, -3, 1])
        .candidate()
        .unwrap()
        .square_free()
        .unwrap();
    assert_eq!(
        square_free.certify_root(dyadic(0), dyadic(3)),
        Err(AlgnumError::InvalidIsolation)
    );
}

#[test]
fn public_bigint_error_conversion_preserves_dyadic_exponent_overflow() {
    let required = BigUint::try_from(u64::from(u32::MAX) + 1).unwrap();
    let lower_error = Dyadic::new(BigInt::one().unwrap(), u32::MAX)
        .mul(&Dyadic::new(BigInt::one().unwrap(), 1))
        .unwrap_err();
    assert_eq!(
        AlgnumError::from(lower_error),
        AlgnumError::Bigint(BigintError::ExponentOverflow {
            required,
            maximum: u32::MAX,
        })
    );
}

#[test]
fn form_sum_promotion_preserves_an_unrepresentable_extension_denominator() {
    let maximum = BigUint::try_from(usize::MAX).unwrap();
    let required = maximum.add(&BigUint::one().unwrap()).unwrap();
    let value = RawFormSum::new(vec![RawTerm::new(
        RawRational::new(BigInt::one().unwrap(), BigUint::one().unwrap()),
        RawMonomial::positive(vec![RawPower::new(
            BigUint::try_from(2_u8).unwrap(),
            RawRational::new(BigInt::one().unwrap(), required.try_clone().unwrap()),
        )]),
    )])
    .normalize()
    .unwrap();

    assert_eq!(
        RealAlgebraic::from_form_sum(&value),
        Err(AlgnumError::FormSum(FormSumErrorKind::DimensionOverflow {
            resource: DimensionResource::Denominator,
            required,
            maximum,
        }))
    );
}

fn dyadic_width(bits: u32) -> neco_bigint::Dyadic {
    neco_bigint::Dyadic::new(BigInt::try_from(1).unwrap(), bits)
}
