use neco_algnum::{AlgnumError, Polynomial, RealAlgebraic};
use neco_bigint::{BigInt, BigUint, Dyadic, RawRational, Sign};

fn polynomial(values: &[i32]) -> Polynomial {
    Polynomial::from_coefficients(
        values
            .iter()
            .map(|value| BigInt::try_from(*value).unwrap())
            .collect(),
    )
}

fn integer(value: i32) -> Dyadic {
    Dyadic::new(BigInt::try_from(value).unwrap(), 0)
}

fn root(values: &[i32], lower: i32, upper: i32) -> RealAlgebraic {
    let factors = polynomial(values)
        .candidate()
        .unwrap()
        .square_free()
        .unwrap()
        .factor()
        .unwrap();
    assert_eq!(factors.len(), 1);
    factors[0]
        .certify_root(integer(lower), integer(upper))
        .unwrap()
        .into_value()
}

fn coefficients(value: &RealAlgebraic) -> Vec<i32> {
    value
        .minimal_polynomial()
        .polynomial()
        .coefficients()
        .iter()
        .map(|coefficient| match coefficient.sign() {
            Sign::Zero => 0,
            Sign::Positive => coefficient.magnitude().limbs_le()[0] as i32,
            Sign::Negative => -(coefficient.magnitude().limbs_le()[0] as i32),
        })
        .collect()
}

#[test]
fn arithmetic_reselects_the_unique_minimal_polynomial_and_root() {
    let positive = root(&[-2, 0, 1], 1, 2);
    let negative = root(&[-2, 0, 1], -2, -1);

    let two = root(&[-2, 1], 1, 3);
    let three = root(&[-3, 1], 2, 4);
    assert_eq!(
        two.add(&three)
            .unwrap()
            .compare_dyadic(&integer(5))
            .unwrap(),
        core::cmp::Ordering::Equal
    );

    let product = positive.mul(&positive).unwrap();
    assert_eq!(coefficients(&product), [-2, 1]);
    assert_eq!(
        product.compare_dyadic(&integer(2)).unwrap(),
        core::cmp::Ordering::Equal
    );

    let sum = positive.add(&negative).unwrap();
    assert!(sum.is_zero());
    assert_eq!(coefficients(&sum), [0, 1]);

    let quotient = positive.div(&positive).unwrap();
    assert_eq!(coefficients(&quotient), [-1, 1]);
    assert_eq!(quotient.sign().unwrap(), Sign::Positive);
}

#[test]
fn integer_and_rational_powers_cover_roots_and_zero_failures() {
    let two = root(&[-2, 1], 1, 3);
    let square_root = two.nth_root(2).unwrap();
    assert_eq!(coefficients(&square_root), [-2, 0, 1]);
    assert_eq!(square_root.sign().unwrap(), Sign::Positive);

    let squared = square_root
        .pow_integer(&BigInt::try_from(2).unwrap())
        .unwrap();
    assert_eq!(
        squared.compare_dyadic(&integer(2)).unwrap(),
        core::cmp::Ordering::Equal
    );

    let reciprocal = two.pow_integer(&BigInt::try_from(-1).unwrap()).unwrap();
    assert_eq!(
        reciprocal
            .compare_dyadic(&Dyadic::new(BigInt::try_from(1).unwrap(), 1))
            .unwrap(),
        core::cmp::Ordering::Equal
    );

    let four = root(&[-4, 1], 3, 5);
    let negative_half = RawRational::new(
        BigInt::try_from(-1).unwrap(),
        BigUint::try_from(2_u32).unwrap(),
    )
    .reduce()
    .unwrap();
    let reciprocal_root = four.pow_rational(negative_half.reduced()).unwrap();
    assert_eq!(
        reciprocal_root
            .compare_dyadic(&Dyadic::new(BigInt::try_from(1).unwrap(), 1))
            .unwrap(),
        core::cmp::Ordering::Equal
    );

    let zero = root(&[0, 1], -1, 1);
    assert_eq!(zero.nth_root(0), Err(AlgnumError::ZeroRootDegree));
    assert_eq!(zero.div(&zero), Err(AlgnumError::DivisionByZero));
    assert_eq!(
        zero.pow_integer(&BigInt::zero()),
        Err(AlgnumError::UndefinedZeroPower)
    );
    assert_eq!(
        zero.pow_integer(&BigInt::try_from(-1).unwrap()),
        Err(AlgnumError::ZeroToNegativePower)
    );
    assert_eq!(
        positive_even_root_failure(),
        AlgnumError::EvenRootOfNegative
    );
}

#[test]
fn is_one_cases_cover_zero_one_negative_one_two_and_sqrt_two() {
    let zero = root(&[0, 1], -1, 1);
    let one = root(&[-1, 1], 0, 2);
    let negative_one = root(&[1, 1], -2, 0);
    let two = root(&[-2, 1], 1, 3);
    let sqrt_two = root(&[-2, 0, 1], 1, 2);

    assert!(!zero.is_one());
    assert!(one.is_one());
    assert!(!negative_one.is_one());
    assert!(!two.is_one());
    assert!(!sqrt_two.is_one());
}

#[test]
fn is_one_regression_rejects_root_index_and_coefficient_mutations() {
    let positive_sqrt_two = root(&[-2, 0, 1], 1, 2);
    let negative_sqrt_two = root(&[-2, 0, 1], -2, -1);

    assert!(!negative_sqrt_two.is_one());

    let wrong_sign = root(&[1, 1], -2, 0);
    let wrong_constant = root(&[-2, 1], 1, 3);
    let two = root(&[-2, 1], 1, 3);
    assert!(!wrong_sign.is_one());
    assert!(!wrong_constant.is_one());

    let one_like = positive_sqrt_two
        .mul(&positive_sqrt_two)
        .unwrap()
        .div(&two)
        .unwrap();
    assert!(one_like.is_one());
}

fn positive_even_root_failure() -> AlgnumError {
    root(&[1, 1], -2, 0).nth_root(2).unwrap_err()
}
