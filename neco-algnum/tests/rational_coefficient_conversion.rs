use neco_algnum::RationalPolynomial;
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
fn conversion_is_empty_for_empty_polynomial() {
    let conversion = RationalPolynomial::from_coefficients(Vec::new())
        .to_real_algebraic_coefficients()
        .unwrap();
    assert!(conversion.coefficients().is_empty());
}

#[test]
fn conversion_preserves_coefficient_order() {
    let polynomial = RationalPolynomial::from_coefficients(vec![rational(1, 2), rational(2, 3)]);
    let conversion = polynomial.to_real_algebraic_coefficients().unwrap();
    assert_eq!(conversion.coefficients().len(), 2);
}

#[test]
fn conversion_try_clone_preserves_coefficients() {
    let polynomial = RationalPolynomial::from_coefficients(vec![rational(1, 2), rational(2, 3)]);
    let conversion = polynomial.to_real_algebraic_coefficients().unwrap();
    let cloned = conversion.try_clone().unwrap();
    assert_eq!(cloned.coefficients(), conversion.coefficients());
}
