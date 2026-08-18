use neco_bigint::{BigInt, BigUint, BigintError, Dyadic, RawRational, Sign};
use neco_formsum::{DimensionResource, FormSum, FormSumErrorKind, RawFormSum, RawTerm};
use neco_monomial::{RawMonomial, RawPower};

fn assert_contains_square_root(lower: &Dyadic, upper: &Dyadic, radicand: u8) {
    assert_eq!(lower.integer().sign(), Sign::Positive);
    assert_eq!(upper.integer().sign(), Sign::Positive);
    let lower_square = lower.integer().magnitude().pow_u32(2).unwrap();
    let lower_target = BigUint::try_from(radicand)
        .unwrap()
        .shl_bits(2 * lower.exponent() as usize)
        .unwrap();
    let upper_square = upper.integer().magnitude().pow_u32(2).unwrap();
    let upper_target = BigUint::try_from(radicand)
        .unwrap()
        .shl_bits(2 * upper.exponent() as usize)
        .unwrap();
    assert!(lower_square <= lower_target);
    assert!(upper_square >= upper_target);
}

fn rational(numerator: i32, denominator: u32) -> RawRational {
    RawRational::new(
        BigInt::try_from(numerator).unwrap(),
        BigUint::try_from(denominator).unwrap(),
    )
}

fn term(coefficient: i32, base: u32, numerator: i32, denominator: u32) -> RawTerm {
    RawTerm::new(
        rational(coefficient, 1),
        RawMonomial::positive(vec![RawPower::new(
            BigUint::try_from(base).unwrap(),
            rational(numerator, denominator),
        )]),
    )
}

fn integer_term(coefficient: i32) -> RawTerm {
    RawTerm::new(rational(coefficient, 1), RawMonomial::positive(Vec::new()))
}

fn form(terms: Vec<RawTerm>) -> FormSum {
    RawFormSum::new(terms).normalize().unwrap()
}

fn integer_coefficients(value: &neco_formsum::AnnihilatingCoefficients) -> Vec<(Sign, u32)> {
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
fn common_extension_coordinates_and_matrix_preserve_values() {
    let value = form(vec![integer_term(1), term(1, 2, 1, 2)]);
    let sqrt_three = form(vec![term(1, 3, 1, 2)]);
    let extension = value.extension_with(&sqrt_three).unwrap();
    assert_eq!(extension.basis_count(), 4);

    let coordinates = value.coordinates_with(&extension).unwrap();
    assert_eq!(coordinates.coefficients().len(), 4);
    assert_eq!(
        coordinates.try_clone().unwrap().into_form_sum().unwrap(),
        value
    );
    assert_eq!(coordinates.multiplication_matrix().unwrap().len(), 16);

    let basis_values = [
        form(vec![integer_term(1)]),
        form(vec![term(1, 3, 1, 2)]),
        form(vec![term(1, 2, 1, 2)]),
        form(vec![RawTerm::new(
            rational(1, 1),
            RawMonomial::positive(vec![
                RawPower::new(BigUint::try_from(2_u8).unwrap(), rational(1, 2)),
                RawPower::new(BigUint::try_from(3_u8).unwrap(), rational(1, 2)),
            ]),
        )]),
    ];
    let matrix = coordinates.multiplication_matrix().unwrap();
    for (column, basis) in basis_values.iter().enumerate() {
        let expected = value
            .mul(basis)
            .unwrap()
            .coordinates_with(&extension)
            .unwrap();
        for row in 0..extension.basis_count() {
            assert_eq!(
                matrix[row + extension.basis_count() * column],
                expected.coefficients()[row]
            );
        }
    }
}

#[test]
fn gaussian_elimination_handles_the_sqrt_two_row_exchange() {
    let sqrt_two = form(vec![term(1, 2, 1, 2)]);
    let inverse = sqrt_two.inverse().unwrap();
    let coefficient = &inverse.terms()[0].1;
    assert_eq!(coefficient.numerator().sign(), Sign::Positive);
    assert_eq!(coefficient.numerator().magnitude().to_u32(), Some(1));
    assert_eq!(coefficient.denominator().to_u32(), Some(2));
    assert_eq!(sqrt_two.mul(&inverse).unwrap(), FormSum::one().unwrap());

    let four_dimensional = sqrt_two.add(&form(vec![term(1, 3, 1, 2)])).unwrap();
    assert_eq!(
        four_dimensional
            .mul(&four_dimensional.inverse().unwrap())
            .unwrap(),
        FormSum::one().unwrap()
    );
    assert_eq!(
        FormSum::zero().inverse().unwrap_err(),
        neco_formsum::FormSumErrorKind::DivisionByZero
    );
}

#[test]
fn faddeev_leverrier_produces_both_required_polynomials() {
    let sqrt_two = form(vec![term(1, 2, 1, 2)]);
    let sqrt_three = form(vec![term(1, 3, 1, 2)]);
    let sum = sqrt_two.add(&sqrt_three).unwrap();
    assert_eq!(
        integer_coefficients(&sum.annihilating_coefficients().unwrap()),
        vec![
            (Sign::Positive, 1),
            (Sign::Zero, 0),
            (Sign::Negative, 10),
            (Sign::Zero, 0),
            (Sign::Positive, 1),
        ]
    );

    let square = sum.mul(&sum).unwrap();
    let fourth = square.mul(&square).unwrap();
    let ten_square = square.mul(&form(vec![integer_term(10)])).unwrap();
    assert!(fourth
        .sub(&ten_square)
        .unwrap()
        .add(&FormSum::one().unwrap())
        .unwrap()
        .is_zero());

    let extension = sqrt_two.extension_with(&sqrt_three).unwrap();
    let polynomial = sqrt_two
        .coordinates_with(&extension)
        .unwrap()
        .annihilating_coefficients()
        .unwrap();
    assert_eq!(
        integer_coefficients(&polynomial),
        vec![
            (Sign::Positive, 4),
            (Sign::Zero, 0),
            (Sign::Negative, 4),
            (Sign::Zero, 0),
            (Sign::Positive, 1),
        ]
    );

    let one_half = RawFormSum::new(vec![RawTerm::new(
        rational(1, 2),
        RawMonomial::positive(Vec::new()),
    )])
    .normalize()
    .unwrap();
    assert_eq!(
        integer_coefficients(&one_half.annihilating_coefficients().unwrap()),
        vec![(Sign::Negative, 1), (Sign::Positive, 2)]
    );
}

#[test]
fn enclosure_and_sign_cover_positive_negative_and_zero_values() {
    let sqrt_two = form(vec![term(1, 2, 1, 2)]);
    let enclosure = sqrt_two.enclose(20).unwrap();
    assert_eq!(
        enclosure.width().unwrap(),
        Dyadic::new(BigInt::one().unwrap(), 20)
    );
    assert_contains_square_root(enclosure.lower(), enclosure.upper(), 2);
    assert_eq!(sqrt_two.sign().unwrap(), Sign::Positive);

    let negative = form(vec![integer_term(1), term(-1, 2, 1, 2)]);
    assert_eq!(negative.sign().unwrap(), Sign::Negative);
    assert_eq!(FormSum::zero().sign().unwrap(), Sign::Zero);

    let product = form(vec![RawTerm::new(
        rational(1, 1),
        RawMonomial::positive(vec![
            RawPower::new(BigUint::try_from(2_u8).unwrap(), rational(1, 2)),
            RawPower::new(BigUint::try_from(3_u8).unwrap(), rational(1, 2)),
        ]),
    )]);
    let coarse = product.enclose(8).unwrap();
    let fine = product.enclose(24).unwrap();
    assert!(fine.width().unwrap() <= coarse.width().unwrap());
    assert!(fine.width().unwrap() <= Dyadic::new(BigInt::one().unwrap(), 24));
    assert_contains_square_root(fine.lower(), fine.upper(), 6);

    let one = Dyadic::new(BigInt::one().unwrap(), 0);
    let negative_enclosure = negative.enclose(24).unwrap();
    let transformed_lower = one.sub(negative_enclosure.upper()).unwrap();
    let transformed_upper = one.sub(negative_enclosure.lower()).unwrap();
    assert_contains_square_root(&transformed_lower, &transformed_upper, 2);

    let high_cancellation = form(vec![integer_term(-665_857), term(470_832, 2, 1, 2)]);
    assert_eq!(high_cancellation.sign().unwrap(), Sign::Negative);

    let one_third = RawFormSum::new(vec![RawTerm::new(
        rational(1, 3),
        RawMonomial::positive(Vec::new()),
    )])
    .normalize()
    .unwrap()
    .enclose(16)
    .unwrap();
    let lower_scaled = one_third
        .lower()
        .integer()
        .magnitude()
        .mul(&BigUint::try_from(3_u8).unwrap())
        .unwrap();
    let lower_unit = BigUint::one()
        .unwrap()
        .shl_bits(one_third.lower().exponent() as usize)
        .unwrap();
    let upper_scaled = one_third
        .upper()
        .integer()
        .magnitude()
        .mul(&BigUint::try_from(3_u8).unwrap())
        .unwrap();
    let upper_unit = BigUint::one()
        .unwrap()
        .shl_bits(one_third.upper().exponent() as usize)
        .unwrap();
    assert!(lower_scaled <= lower_unit);
    assert!(upper_scaled >= upper_unit);
}

#[test]
fn mixed_radix_coordinates_cover_every_basis_index() {
    let cube_root_two = form(vec![term(1, 2, 1, 3)]);
    let sqrt_three = form(vec![term(1, 3, 1, 2)]);
    let extension = cube_root_two.extension_with(&sqrt_three).unwrap();
    assert_eq!(extension.basis_count(), 6);
    assert_eq!(extension.primes()[0].value().to_u32(), Some(2));
    assert_eq!(extension.primes()[1].value().to_u32(), Some(3));
    assert_eq!(extension.denominators()[0].to_u32(), Some(3));
    assert_eq!(extension.denominators()[1].to_u32(), Some(2));

    for first_digit in 0..3_i32 {
        for second_digit in 0..2_i32 {
            let mut powers = Vec::new();
            if first_digit != 0 {
                powers.push(RawPower::new(
                    BigUint::try_from(2_u8).unwrap(),
                    rational(first_digit, 3),
                ));
            }
            if second_digit != 0 {
                powers.push(RawPower::new(
                    BigUint::try_from(3_u8).unwrap(),
                    rational(second_digit, 2),
                ));
            }
            let basis = form(vec![RawTerm::new(
                rational(1, 1),
                RawMonomial::positive(powers),
            )]);
            let coordinates = basis.coordinates_with(&extension).unwrap();
            let expected = (first_digit * 2 + second_digit) as usize;
            for (index, coefficient) in coordinates.coefficients().iter().enumerate() {
                assert_eq!(coefficient.is_zero(), index != expected);
            }
            assert_eq!(coordinates.into_form_sum().unwrap(), basis);
        }
    }

    let fourth_power = form(vec![term(1, 2, 4, 3)]);
    assert_eq!(
        cube_root_two
            .mul(&cube_root_two)
            .unwrap()
            .mul(&cube_root_two)
            .unwrap(),
        form(vec![integer_term(2)])
    );
    assert_eq!(
        form(vec![term(1, 2, 2, 3)])
            .mul(&form(vec![term(1, 2, 2, 3)]))
            .unwrap(),
        fourth_power
    );
}

#[test]
fn enclosure_reports_an_unrepresentable_shift() {
    let denominator = BigUint::try_from(usize::MAX)
        .unwrap()
        .add(&BigUint::one().unwrap())
        .unwrap();
    let value = RawFormSum::new(vec![RawTerm::new(
        rational(1, 1),
        RawMonomial::positive(vec![RawPower::new(
            BigUint::try_from(2_u8).unwrap(),
            RawRational::new(BigInt::one().unwrap(), denominator),
        )]),
    )])
    .normalize()
    .unwrap();
    assert_eq!(
        value.enclose(1).unwrap_err(),
        neco_formsum::FormSumErrorKind::Bigint(BigintError::CapacityOverflow)
    );
}

#[test]
fn extension_reports_an_unrepresentable_denominator() {
    let maximum = BigUint::try_from(usize::MAX).unwrap();
    let required = maximum.add(&BigUint::one().unwrap()).unwrap();
    let value = RawFormSum::new(vec![RawTerm::new(
        rational(1, 1),
        RawMonomial::positive(vec![RawPower::new(
            BigUint::try_from(2_u8).unwrap(),
            RawRational::new(BigInt::one().unwrap(), required.try_clone().unwrap()),
        )]),
    )])
    .normalize()
    .unwrap();
    assert_eq!(
        value.extension_with(&FormSum::zero()).unwrap_err(),
        neco_formsum::FormSumErrorKind::DimensionOverflow {
            resource: neco_formsum::DimensionResource::Denominator,
            required,
            maximum,
        }
    );
}

#[test]
fn extension_reports_the_exact_basis_dimension_product() {
    let maximum = BigUint::try_from(usize::MAX).unwrap();
    let value = RawFormSum::new(vec![RawTerm::new(
        rational(1, 1),
        RawMonomial::positive(vec![
            RawPower::new(
                BigUint::try_from(2_u8).unwrap(),
                RawRational::new(BigInt::one().unwrap(), maximum.try_clone().unwrap()),
            ),
            RawPower::new(BigUint::try_from(3_u8).unwrap(), rational(1, 2)),
        ]),
    )])
    .normalize()
    .unwrap();
    let required = maximum.mul(&BigUint::try_from(2_u8).unwrap()).unwrap();
    assert_eq!(
        value.extension_with(&FormSum::zero()).unwrap_err(),
        FormSumErrorKind::DimensionOverflow {
            resource: DimensionResource::BasisCount,
            required,
            maximum,
        }
    );
}
