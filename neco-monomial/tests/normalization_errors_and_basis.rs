use neco_bigint::{BigInt, BigUint, RawRational, ReducedRational};
use neco_monomial::{
    MonomialErrorKind, NormalizationErrors, ProvenPrime, RadicalBasis, RawMonomial, RawPower,
};

fn rational(numerator: i32, denominator: u32) -> ReducedRational {
    RawRational::new(
        BigInt::try_from(numerator).unwrap(),
        BigUint::try_from(denominator).unwrap(),
    )
    .reduce()
    .unwrap()
    .into_reduced()
}

fn prime(value: u32) -> ProvenPrime {
    RawMonomial::positive(vec![RawPower::new(
        BigUint::try_from(value).unwrap(),
        RawRational::new(BigInt::one().unwrap(), BigUint::one().unwrap()),
    )])
    .normalize()
    .unwrap()
    .factors()[0]
        .0
        .try_clone()
        .unwrap()
}

fn factor(value: u32, numerator: i32, denominator: u32) -> (ProvenPrime, ReducedRational) {
    (prime(value), rational(numerator, denominator))
}

#[test]
fn normalization_errors_construct_and_transfer_all_valid_states() {
    assert!(NormalizationErrors::<i32>::from_errors(vec![]).is_none());

    let singleton = NormalizationErrors::from_one(7);
    assert!(singleton.errors().eq([&7]));
    assert_eq!(singleton.into_parts(), (7, vec![]));

    let singleton = NormalizationErrors::from_errors(vec![4, 4]).unwrap();
    assert_eq!(singleton.into_parts(), (4, vec![]));

    let values = vec![3, 1, 2, 3, 1];
    let allocation = values.as_ptr();
    let errors = NormalizationErrors::from_errors(values).unwrap();
    assert!(errors.errors().eq([&1, &2, &3]));
    let (first, additional) = errors.into_parts();
    assert_eq!(first, 1);
    assert_eq!(additional, [2, 3]);
    assert_eq!(additional.as_ptr(), allocation);
}

#[test]
fn radical_basis_accepts_only_sorted_distinct_proper_exponents() {
    assert!(RadicalBasis::try_from_sorted_factors(vec![]).is_ok());
    let valid =
        RadicalBasis::try_from_sorted_factors(vec![factor(2, 1, 2), factor(3, 2, 3)]).unwrap();
    assert_eq!(valid.factors().len(), 2);

    for factors in [
        vec![factor(3, 1, 2), factor(2, 1, 2)],
        vec![factor(2, 1, 2), factor(2, 2, 3)],
        vec![factor(2, 0, 1)],
        vec![factor(2, 1, 1)],
        vec![factor(2, -1, 2)],
        vec![factor(2, 3, 2)],
    ] {
        assert_eq!(
            RadicalBasis::try_from_sorted_factors(factors),
            Err(MonomialErrorKind::InvalidRadicalBasis)
        );
    }
}

#[test]
fn invalid_radical_basis_is_ordered_cloned_and_displayed() {
    let error = MonomialErrorKind::InvalidRadicalBasis;
    assert_eq!(error.try_clone().unwrap(), error);
    assert_eq!(error.to_string(), "invalid radical basis");
    assert!(MonomialErrorKind::EvenRootOfNegative < error);
    assert!(error < MonomialErrorKind::CapacityOverflow);
}
