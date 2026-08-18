use neco_bigint::{BigInt, BigUint, RawRational, ReducedRational, Sign};
use neco_monomial::{Monomial, MonomialErrorKind, RawMonomial, RawPower};

fn uint(value: u32) -> BigUint {
    BigUint::try_from(value).unwrap()
}

fn int(value: i32) -> BigInt {
    BigInt::try_from(value).unwrap()
}

fn raw_rational(numerator: i32, denominator: u32) -> RawRational {
    RawRational::new(int(numerator), uint(denominator))
}

fn rational(numerator: i32, denominator: u32) -> ReducedRational {
    raw_rational(numerator, denominator)
        .reduce()
        .unwrap()
        .into_reduced()
}

fn positive(powers: &[(u32, i32, u32)]) -> Monomial {
    RawMonomial::positive(
        powers
            .iter()
            .map(|(base, numerator, denominator)| {
                RawPower::new(uint(*base), raw_rational(*numerator, *denominator))
            })
            .collect(),
    )
    .normalize()
    .unwrap()
}

#[test]
fn normalization_preserves_prime_products_and_combines_exponents() {
    let value = positive(&[(12, 1, 2), (18, 1, 3), (5, 0, 7)]);
    assert_eq!(value.sign(), Sign::Positive);
    assert_eq!(value.factors().len(), 2);
    assert_eq!(value.factors()[0].0.value().to_u32(), Some(2));
    assert_eq!(value.factors()[0].1, rational(4, 3));
    assert_eq!(value.factors()[1].0.value().to_u32(), Some(3));
    assert_eq!(value.factors()[1].1, rational(7, 6));
}

#[test]
fn normalization_is_permutation_invariant() {
    let entries = [(12, 1, 2), (8, -1, 3), (45, 2, 5)];
    let expected = positive(&entries);
    for permutation in [
        [entries[0], entries[1], entries[2]],
        [entries[0], entries[2], entries[1]],
        [entries[1], entries[0], entries[2]],
        [entries[1], entries[2], entries[0]],
        [entries[2], entries[0], entries[1]],
        [entries[2], entries[1], entries[0]],
    ] {
        assert_eq!(positive(&permutation), expected);
    }
}

#[test]
fn normalization_orders_primes_across_distinct_composite_bases() {
    let separated = positive(&[(5, 1, 1), (6, 1, 1)]);
    let combined = positive(&[(30, 1, 1)]);
    assert_eq!(separated, combined);
    assert_eq!(
        separated
            .factors()
            .iter()
            .map(|(prime, _)| prime.value().to_u32())
            .collect::<Vec<_>>(),
        vec![Some(2), Some(3), Some(5)]
    );
}

#[test]
fn multiplication_division_and_power_preserve_exponent_arithmetic() {
    let left = positive(&[(2, 1, 2), (3, -2, 3)]);
    let right = positive(&[(2, 3, 2), (5, 1, 7)]);
    let product = left.mul(&right).unwrap();
    assert_eq!(product.factors()[0].1, rational(2, 1));
    assert_eq!(product.div(&right).unwrap(), left);
    assert_eq!(left.div(&left).unwrap(), Monomial::one());
    assert_eq!(left.pow(&rational(0, 1)).unwrap(), Monomial::one());
}

#[test]
fn division_by_zero_is_an_explicit_failure() {
    let numerator = positive(&[(2, 3, 5), (7, -2, 3)]);
    assert_eq!(
        numerator.div(&Monomial::zero()),
        Err(MonomialErrorKind::DivisionByZero)
    );
}

#[test]
fn normalization_removes_factors_after_cross_base_cancellation() {
    assert_eq!(positive(&[(2, 1, 1), (4, -1, 2)]), Monomial::one());
}

#[test]
fn zero_short_circuits_unrelated_factors_after_invalid_scan() {
    let value = RawMonomial::positive(vec![
        RawPower::new(uint(97), raw_rational(1, 1)),
        RawPower::new(BigUint::zero(), raw_rational(3, 7)),
        RawPower::new(uint(0xffff_ffff), raw_rational(7, 11)),
    ])
    .normalize()
    .unwrap();
    assert!(value.is_zero());
}

#[test]
fn negative_real_powers_follow_reduced_exponent_parity() {
    let negative_eight = RawMonomial::negative(vec![RawPower::new(uint(8), raw_rational(1, 1))])
        .normalize()
        .unwrap();
    let cube_root = negative_eight.pow(&rational(2, 6)).unwrap();
    assert_eq!(cube_root.sign(), Sign::Negative);
    let (coefficient, basis) = cube_root.split_radical().unwrap();
    assert_eq!(coefficient, rational(-2, 1));
    assert!(basis.factors().is_empty());

    let square = negative_eight.pow(&rational(2, 3)).unwrap();
    assert_eq!(square.split_radical().unwrap().0, rational(4, 1));
    assert_eq!(
        negative_eight.pow(&rational(1, 2)),
        Err(MonomialErrorKind::EvenRootOfNegative)
    );
}

#[test]
fn radical_split_preserves_the_source_monomial() {
    let source = positive(&[(8, -1, 2), (27, 4, 3)]);
    let (coefficient, basis) = source.split_radical().unwrap();
    assert_eq!(coefficient, rational(81, 4));

    let coefficient_monomial = positive(&[(2, -2, 1), (3, 4, 1)]);
    let basis_monomial = RawMonomial::positive(
        basis
            .factors()
            .iter()
            .map(|(prime, exponent)| {
                RawPower::new(
                    prime.value().try_clone().unwrap(),
                    RawRational::new(
                        exponent.numerator().try_clone().unwrap(),
                        exponent.denominator().try_clone().unwrap(),
                    ),
                )
            })
            .collect(),
    )
    .normalize()
    .unwrap();
    assert_eq!(coefficient_monomial.mul(&basis_monomial).unwrap(), source);
}

#[test]
fn negative_half_exponent_uses_euclidean_floor() {
    let source = positive(&[(2, -1, 2)]);
    let (coefficient, basis) = source.split_radical().unwrap();
    assert_eq!(coefficient, rational(1, 2));
    assert_eq!(basis.factors().len(), 1);
    assert_eq!(basis.factors()[0].0.value().to_u32(), Some(2));
    assert_eq!(basis.factors()[0].1, rational(1, 2));
}
