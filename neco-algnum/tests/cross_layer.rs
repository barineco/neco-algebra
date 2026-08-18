use neco_algnum::{Polynomial, RationalPolynomial, RealAlgebraic};
use neco_bigint::{BigInt, BigUint, Dyadic, RawRational, ReducedRational, Sign};
use neco_formsum::{FormSum, RawFormSum, RawTerm};
use neco_monomial::{RawMonomial, RawPower};

fn integer(value: i32) -> BigInt {
    BigInt::try_from(value).unwrap()
}

fn dyadic(value: i32) -> Dyadic {
    Dyadic::new(integer(value), 0)
}

fn polynomial(coefficients: &[i32]) -> Polynomial {
    Polynomial::from_coefficients(coefficients.iter().copied().map(integer).collect())
}

fn rational(numerator: i32, denominator: u32) -> ReducedRational {
    RawRational::new(integer(numerator), BigUint::try_from(denominator).unwrap())
        .reduce()
        .unwrap()
        .into_reduced()
}

fn rational_polynomial(coefficients: &[i32]) -> RationalPolynomial {
    RationalPolynomial::from_coefficients(
        coefficients
            .iter()
            .copied()
            .map(|coefficient| rational(coefficient, 1))
            .collect(),
    )
}

fn raw_rational(numerator: i32, denominator: u32) -> RawRational {
    RawRational::new(integer(numerator), BigUint::try_from(denominator).unwrap())
}

fn radical(base: u32) -> FormSum {
    RawFormSum::new(vec![RawTerm::new(
        raw_rational(1, 1),
        RawMonomial::positive(vec![RawPower::new(
            BigUint::try_from(base).unwrap(),
            raw_rational(1, 2),
        )]),
    )])
    .normalize()
    .unwrap()
}

fn rational_form_sum(numerator: i32, denominator: u32) -> FormSum {
    RawFormSum::new(vec![RawTerm::new(
        raw_rational(numerator, denominator),
        RawMonomial::positive(Vec::new()),
    )])
    .normalize()
    .unwrap()
}

fn root(coefficients: &[i32], lower: i32, upper: i32) -> RealAlgebraic {
    let factors = polynomial(coefficients)
        .candidate()
        .unwrap()
        .square_free()
        .unwrap()
        .factor()
        .unwrap();
    assert_eq!(factors.len(), 1);
    factors[0]
        .certify_root(dyadic(lower), dyadic(upper))
        .unwrap()
        .into_value()
}

fn signed_coefficients(value: &Polynomial) -> Vec<(Sign, u32)> {
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

fn assert_minimal_polynomial(value: &RealAlgebraic, expected: &[i32], root_index: usize) {
    assert_eq!(
        value.minimal_polynomial().polynomial(),
        &polynomial(expected)
    );
    assert_eq!(value.root_index().get(), root_index);
}

fn assert_reduced(quotient_degree: usize, value: &RationalPolynomial) {
    assert!(value.degree().is_none_or(|degree| degree < quotient_degree));
}

#[test]
fn quotient_operations_and_generator_return_reduced_representatives() {
    let sqrt_two = root(&[-2, 0, 1], 1, 2);
    let quotient = sqrt_two.minimal_polynomial().quotient().unwrap();
    let lhs = rational_polynomial(&[1, 1]);
    let rhs = rational_polynomial(&[-1, 1]);

    let unreduced = rational_polynomial(&[1, 0, 1]);
    let reduced = quotient.reduce(&unreduced).unwrap();
    let sum = quotient.add(&lhs, &rhs).unwrap();
    let difference = quotient.sub(&lhs, &rhs).unwrap();
    let product = quotient.mul(&lhs, &rhs).unwrap();
    let generator = quotient.generator().unwrap().as_polynomial().unwrap();

    for value in [&reduced, &sum, &difference, &product, &generator] {
        assert_reduced(2, value);
    }
    assert_eq!(reduced, rational_polynomial(&[3]));
    assert_eq!(sum, rational_polynomial(&[0, 2]));
    assert_eq!(difference, rational_polynomial(&[2]));
    assert_eq!(product, rational_polynomial(&[1]));
    assert_eq!(generator, rational_polynomial(&[0, 1]));

    let one = root(&[-1, 1], 0, 2);
    let linear_quotient = one.minimal_polynomial().quotient().unwrap();
    let linear_generator = linear_quotient
        .generator()
        .unwrap()
        .as_polynomial()
        .unwrap();
    assert_reduced(1, &linear_generator);
    assert_eq!(linear_generator, rational_polynomial(&[1]));

    let zero = root(&[0, 1], -1, 1);
    let zero_generator = zero
        .minimal_polynomial()
        .quotient()
        .unwrap()
        .generator()
        .unwrap()
        .as_polynomial()
        .unwrap();
    assert!(zero_generator.is_zero());
}

#[test]
fn form_sum_promotions_select_canonical_minimal_polynomials_and_roots() {
    let zero = RealAlgebraic::from_form_sum(&FormSum::zero()).unwrap();
    assert_minimal_polynomial(&zero, &[0, 1], 0);

    let one_half = RealAlgebraic::from_form_sum(&rational_form_sum(1, 2)).unwrap();
    assert_minimal_polynomial(&one_half, &[-1, 2], 0);

    let sqrt_two = radical(2);
    let positive = RealAlgebraic::from_form_sum(&sqrt_two).unwrap();
    assert_minimal_polynomial(&positive, &[-2, 0, 1], 1);

    let negative_sqrt_two = FormSum::zero().sub(&sqrt_two).unwrap();
    let negative = RealAlgebraic::from_form_sum(&negative_sqrt_two).unwrap();
    assert_minimal_polynomial(&negative, &[-2, 0, 1], 0);

    let one_plus_sqrt_two = FormSum::one().unwrap().add(&sqrt_two).unwrap();
    let promoted = RealAlgebraic::from_form_sum(&one_plus_sqrt_two).unwrap();
    assert_minimal_polynomial(&promoted, &[-1, -2, 1], 1);
    assert!(!promoted.is_one());

    let sqrt_two_plus_sqrt_three = sqrt_two.add(&radical(3)).unwrap();
    let promoted_sum = RealAlgebraic::from_form_sum(&sqrt_two_plus_sqrt_three).unwrap();
    assert_minimal_polynomial(&promoted_sum, &[1, 0, -10, 0, 1], 3);
}

#[test]
fn four_dimensional_annihilator_loses_its_duplicate_factor_before_root_selection() {
    let sqrt_two = radical(2);
    let sqrt_three = radical(3);
    let extension = sqrt_two.extension_with(&sqrt_three).unwrap();
    assert_eq!(extension.basis_count(), 4);
    let annihilator = sqrt_two
        .coordinates_with(&extension)
        .unwrap()
        .annihilating_coefficients()
        .unwrap();
    let candidate = Polynomial::from_coefficients(
        annihilator
            .coefficients()
            .iter()
            .map(|coefficient| coefficient.try_clone().unwrap())
            .collect(),
    );
    assert_eq!(
        signed_coefficients(&candidate),
        signed_coefficients(&polynomial(&[4, 0, -4, 0, 1]))
    );

    let square_free = candidate.candidate().unwrap().square_free().unwrap();
    assert_eq!(square_free.polynomial(), &polynomial(&[-2, 0, 1]));
    let roots = square_free.isolate_real_roots().unwrap();
    assert_eq!(roots.len(), 2);
    assert_eq!(roots[0].value().root_index().get(), 0);
    assert_eq!(roots[1].value().root_index().get(), 1);
}

#[test]
fn cross_layer_equality_uses_substitution_and_the_selected_root() {
    let sqrt_two = radical(2);
    let positive = root(&[-2, 0, 1], 1, 2);
    let negative = root(&[-2, 0, 1], -2, -1);

    assert!(positive.equals_form_sum(&sqrt_two).unwrap());
    assert!(!negative.equals_form_sum(&sqrt_two).unwrap());
    assert!(!positive.equals_form_sum(&FormSum::one().unwrap()).unwrap());
    assert!(negative
        .equals_form_sum(&FormSum::zero().sub(&sqrt_two).unwrap())
        .unwrap());
}

#[test]
fn negative_odd_root_returns_the_unique_real_root_with_a_certified_enclosure() {
    let negative_eight = root(&[8, 1], -9, -7);
    let result = negative_eight.nth_root(3).unwrap();
    assert_minimal_polynomial(&result, &[2, 1], 0);
    assert_eq!(
        result.compare_dyadic(&dyadic(-2)).unwrap(),
        core::cmp::Ordering::Equal
    );
    let enclosure = result.enclose(12).unwrap();
    assert!(enclosure.enclosure().contains_dyadic(&dyadic(-2)));
    assert!(enclosure.enclosure().width().unwrap() <= Dyadic::new(integer(1), 12));
}
