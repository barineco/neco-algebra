use neco_bigint::{BigInt, BigUint, BigintError, RawRational};
use neco_monomial::{MonomialErrorKind, RawMonomial, RawPower};

fn power(base: u32, numerator: i32, denominator: u32) -> RawPower {
    RawPower::new(
        BigUint::try_from(base).unwrap(),
        RawRational::new(
            BigInt::try_from(numerator).unwrap(),
            BigUint::try_from(denominator).unwrap(),
        ),
    )
}

#[test]
fn required_radical_vectors_have_structural_results() {
    let sqrt_twelve = RawMonomial::positive(vec![power(12, 1, 2)])
        .normalize()
        .unwrap();
    let (coefficient, basis) = sqrt_twelve.split_radical().unwrap();
    assert_eq!(coefficient.numerator(), &BigInt::try_from(2).unwrap());
    assert_eq!(basis.factors().len(), 1);
    assert_eq!(basis.factors()[0].0.value().to_u32(), Some(3));

    let sqrt_two = RawMonomial::positive(vec![power(2, 1, 2)])
        .normalize()
        .unwrap();
    let sqrt_eight = RawMonomial::positive(vec![power(8, 1, 2)])
        .normalize()
        .unwrap();
    let product = sqrt_two.mul(&sqrt_eight).unwrap();
    let (coefficient, basis) = product.split_radical().unwrap();
    assert_eq!(coefficient.numerator(), &BigInt::try_from(4).unwrap());
    assert!(basis.factors().is_empty());
}

#[test]
fn semantic_invalids_are_sorted_deduplicated_and_permutation_invariant() {
    let entries = [power(0, -1, 1), power(0, 0, 0), power(0, 0, 1)];
    let expected = [
        MonomialErrorKind::ZeroToNegativePower,
        MonomialErrorKind::UndefinedZeroPower,
        MonomialErrorKind::Bigint(BigintError::ZeroDenominator),
    ];
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let raw = RawMonomial::positive(
            order
                .into_iter()
                .flat_map(|index| {
                    [
                        entries[index].try_clone().unwrap(),
                        entries[index].try_clone().unwrap(),
                    ]
                })
                .collect(),
        );
        let errors = raw.normalize().unwrap_err();
        assert!(errors.errors().eq(expected.iter()));
    }
}

#[test]
fn zero_power_failures_are_distinct() {
    let zero = RawMonomial::zero().normalize().unwrap();
    for (numerator, expected) in [
        (0, MonomialErrorKind::UndefinedZeroPower),
        (-1, MonomialErrorKind::ZeroToNegativePower),
    ] {
        let exponent = RawRational::new(
            BigInt::try_from(numerator).unwrap(),
            BigUint::try_from(1_u8).unwrap(),
        )
        .reduce()
        .unwrap()
        .into_reduced();
        assert_eq!(zero.pow(&exponent), Err(expected));
    }
}
