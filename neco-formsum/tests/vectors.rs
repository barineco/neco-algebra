use neco_bigint::{BigInt, BigUint, RawRational, Sign};
use neco_formsum::{FormSum, RawFormSum, RawTerm};
use neco_monomial::{RawMonomial, RawPower};

fn raw(numerator: i32, denominator: u32) -> RawRational {
    RawRational::new(
        BigInt::try_from(numerator).unwrap(),
        BigUint::try_from(denominator).unwrap(),
    )
}

fn radical(
    coefficient: i32,
    base: u32,
    exponent_numerator: i32,
    exponent_denominator: u32,
) -> RawTerm {
    RawTerm::new(
        raw(coefficient, 1),
        RawMonomial::positive(vec![RawPower::new(
            BigUint::try_from(base).unwrap(),
            raw(exponent_numerator, exponent_denominator),
        )]),
    )
}

#[test]
fn required_radical_vectors_have_exact_coefficients_and_bases() {
    let vectors = [(8, 1, 2, 2, 2, 1), (2, -1, 2, 2, 1, 2), (72, 1, 2, 2, 6, 1)];
    for (base, numerator, denominator, expected_base, expected_num, expected_den) in vectors {
        let value = RawFormSum::new(vec![radical(1, base, numerator, denominator)])
            .normalize()
            .unwrap();
        let (basis, coefficient) = &value.terms()[0];
        assert_eq!(basis.factors()[0].0.value().to_u32(), Some(expected_base));
        assert_eq!(coefficient.numerator().sign(), Sign::Positive);
        assert_eq!(
            coefficient.numerator().magnitude().to_u32(),
            Some(expected_num)
        );
        assert_eq!(coefficient.denominator().to_u32(), Some(expected_den));
    }
}

#[test]
fn identities_for_zero_and_one_are_observed_from_both_sides() {
    let value = RawFormSum::new(vec![radical(3, 5, 1, 2)])
        .normalize()
        .unwrap();
    assert_eq!(value.add(&FormSum::zero()).unwrap(), value);
    assert_eq!(FormSum::zero().add(&value).unwrap(), value);
    assert_eq!(value.mul(&FormSum::one().unwrap()).unwrap(), value);
    assert_eq!(FormSum::one().unwrap().mul(&value).unwrap(), value);
}
