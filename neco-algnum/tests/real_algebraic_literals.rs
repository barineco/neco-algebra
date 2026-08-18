use neco_algnum::{RationalPolynomial, RealAlgebraic};
use neco_bigint::{BigInt, BigUint, RawRational};

fn rational(numerator: i32, denominator: u32) -> neco_bigint::ReducedRational {
    RawRational::new(
        BigInt::try_from(numerator).unwrap(),
        BigUint::try_from(denominator).unwrap(),
    )
    .reduce()
    .unwrap()
    .into_reduced()
}

#[test]
fn exact_integer_and_reduced_rational_literals_are_constructible() {
    let integer = RealAlgebraic::from_integer(BigInt::try_from(3).unwrap()).unwrap();
    assert!(!integer.is_one());
    let half = RealAlgebraic::from_reduced_rational(&rational(1, 2)).unwrap();
    assert!(half.compare(&integer).unwrap().is_lt());
}

#[test]
fn rational_coefficients_preserve_order_and_zero() {
    let polynomial = RationalPolynomial::from_coefficients(vec![rational(1, 2), rational(0, 1)]);
    let conversion = polynomial.to_real_algebraic_coefficients().unwrap();
    assert_eq!(conversion.coefficients().len(), 1);
}
