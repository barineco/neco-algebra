use neco_bigint::{BigInt, BigUint, BigintError, RawRational, Sign};
use neco_formsum::{FormSumErrorKind, RawFormSum, RawTerm};
use neco_monomial::{MonomialErrorKind, RawMonomial, RawPower};

fn rational(numerator: i32, denominator: u32) -> RawRational {
    RawRational::new(
        BigInt::try_from(numerator).unwrap(),
        BigUint::try_from(denominator).unwrap(),
    )
}

fn power(base: u32, numerator: i32, denominator: u32) -> RawPower {
    RawPower::new(
        BigUint::try_from(base).unwrap(),
        rational(numerator, denominator),
    )
}

fn term(coefficient: i32, base: u32, numerator: i32, denominator: u32) -> RawTerm {
    RawTerm::new(
        rational(coefficient, 1),
        RawMonomial::positive(vec![power(base, numerator, denominator)]),
    )
}

fn integer_term(coefficient: i32) -> RawTerm {
    RawTerm::new(rational(coefficient, 1), RawMonomial::positive(Vec::new()))
}

fn coefficient(value: &neco_formsum::FormSum, index: usize) -> (Sign, u32, u32) {
    let coefficient = &value.terms()[index].1;
    (
        coefficient.numerator().sign(),
        coefficient.numerator().magnitude().to_u32().unwrap(),
        coefficient.denominator().to_u32().unwrap(),
    )
}

#[test]
fn normalization_is_permutation_invariant_and_combines_radicals() {
    let left = RawFormSum::new(vec![term(1, 8, 1, 2), term(-2, 2, 1, 2)])
        .normalize()
        .unwrap();
    let right = RawFormSum::new(vec![term(-2, 2, 1, 2), term(1, 8, 1, 2)])
        .normalize()
        .unwrap();
    assert_eq!(left, right);
    assert!(left.is_zero());

    let inverse_root = RawFormSum::new(vec![term(1, 2, -1, 2)])
        .normalize()
        .unwrap();
    assert_eq!(coefficient(&inverse_root, 0), (Sign::Positive, 1, 2));
    assert_eq!(
        inverse_root.terms()[0].0.factors()[0].0.value().to_u32(),
        Some(2)
    );
}

#[test]
fn zero_inputs_and_coefficient_cancellation_have_one_zero_form() {
    let value = RawFormSum::new(vec![
        term(0, 2, 1, 2),
        RawTerm::new(rational(7, 1), RawMonomial::zero()),
        term(3, 3, 1, 2),
        term(-3, 3, 1, 2),
    ])
    .normalize()
    .unwrap();
    assert_eq!(value, neco_formsum::FormSum::zero());
}

#[test]
fn normalization_collects_semantic_failures_even_under_zero_coefficient() {
    let orders = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for order in orders {
        let mut terms = invalid_terms();
        let mut arranged = Vec::new();
        for index in order {
            arranged.push(terms[index].take().unwrap());
        }
        let errors = RawFormSum::new(arranged).normalize().unwrap_err();
        let expected = [
            FormSumErrorKind::Bigint(BigintError::ZeroDenominator),
            FormSumErrorKind::Monomial(MonomialErrorKind::ZeroToNegativePower),
            FormSumErrorKind::Monomial(MonomialErrorKind::UndefinedZeroPower),
            FormSumErrorKind::Monomial(MonomialErrorKind::Bigint(BigintError::ZeroDenominator)),
        ];
        assert!(errors.errors().eq(expected.iter()));
    }
}

fn invalid_terms() -> [Option<RawTerm>; 3] {
    [
        Some(RawTerm::new(
            rational(1, 0),
            RawMonomial::positive(vec![power(0, 0, 1)]),
        )),
        Some(RawTerm::new(
            rational(0, 1),
            RawMonomial::positive(vec![power(0, -1, 1)]),
        )),
        Some(RawTerm::new(
            rational(1, 1),
            RawMonomial::positive(vec![power(2, 1, 0)]),
        )),
    ]
}

#[test]
fn cloning_and_sparse_arithmetic_preserve_normal_form() {
    let left = RawFormSum::new(vec![integer_term(1), term(1, 2, 1, 2)])
        .normalize()
        .unwrap();
    let right = RawFormSum::new(vec![integer_term(-1), term(1, 2, 1, 2)])
        .normalize()
        .unwrap();
    assert_eq!(left.try_clone().unwrap(), left);
    assert_eq!(
        left.add(&right).unwrap(),
        RawFormSum::new(vec![term(2, 2, 1, 2)]).normalize().unwrap()
    );
    assert_eq!(left.sub(&left).unwrap(), neco_formsum::FormSum::zero());
    assert_eq!(
        left.mul(&right).unwrap(),
        neco_formsum::FormSum::one().unwrap()
    );
}
