use neco_algnum::{Polynomial, RationalPolynomial};
use neco_bigint::{BigInt, BigUint, RawRational, ReducedRational, Sign};

fn integer(value: i32) -> BigInt {
    BigInt::try_from(value).unwrap()
}

fn polynomial(values: &[i32]) -> Polynomial {
    Polynomial::from_coefficients(values.iter().copied().map(integer).collect())
}

fn rational(numerator: i32, denominator: u32) -> ReducedRational {
    RawRational::new(integer(numerator), BigUint::try_from(denominator).unwrap())
        .reduce()
        .unwrap()
        .into_reduced()
}

fn integer_coefficients(value: &Polynomial) -> Vec<(Sign, u32)> {
    value
        .coefficients()
        .iter()
        .map(|coefficient| {
            (
                coefficient.sign(),
                coefficient.magnitude().to_u32().unwrap(),
            )
        })
        .collect()
}

#[test]
fn integer_operations_preserve_evaluation_identities() {
    let left = polynomial(&[3, -2, 1]);
    let right = polynomial(&[-1, 1]);
    for point in -3..=3 {
        let point = integer(point);
        let left_value = left.evaluate_bigint(&point).unwrap();
        let right_value = right.evaluate_bigint(&point).unwrap();
        assert_eq!(
            left.add(&right).unwrap().evaluate_bigint(&point).unwrap(),
            left_value.add(&right_value).unwrap()
        );
        assert_eq!(
            left.sub(&right).unwrap().evaluate_bigint(&point).unwrap(),
            left_value.sub(&right_value).unwrap()
        );
        assert_eq!(
            left.mul(&right).unwrap().evaluate_bigint(&point).unwrap(),
            left_value.mul(&right_value).unwrap()
        );
    }
}

#[test]
fn normalization_derivative_and_composition_have_exact_coefficients() {
    assert_eq!(polynomial(&[7, 0, 0]).degree(), Some(0));
    assert!(polynomial(&[0, 0]).is_zero());

    let value = polynomial(&[1, 2, 1]);
    assert_eq!(
        integer_coefficients(&value.derivative().unwrap()),
        [(Sign::Positive, 2), (Sign::Positive, 2)]
    );
    assert_eq!(
        integer_coefficients(&value.compose(&polynomial(&[1, 1])).unwrap()),
        [
            (Sign::Positive, 4),
            (Sign::Positive, 4),
            (Sign::Positive, 1),
        ]
    );
}

#[test]
fn rational_division_reconstructs_the_dividend_and_gcd_is_monic() {
    let dividend = RationalPolynomial::from_coefficients(vec![
        rational(-1, 1),
        rational(0, 1),
        rational(1, 1),
    ]);
    let divisor = RationalPolynomial::from_coefficients(vec![rational(-1, 1), rational(1, 1)]);
    let (quotient, remainder) = dividend.div_rem(&divisor).unwrap();
    assert_eq!(
        divisor.mul(&quotient).unwrap().add(&remainder).unwrap(),
        dividend
    );
    assert!(remainder.is_zero());

    let repeated = RationalPolynomial::from_coefficients(vec![
        rational(1, 1),
        rational(-2, 1),
        rational(1, 1),
    ]);
    let gcd = dividend.gcd(&repeated).unwrap();
    assert_eq!(gcd.coefficients(), [rational(-1, 1), rational(1, 1)]);
}
