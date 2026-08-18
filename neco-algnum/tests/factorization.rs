use neco_algnum::Polynomial;
use neco_bigint::{BigInt, Sign};

fn polynomial(values: &[i32]) -> Polynomial {
    Polynomial::from_coefficients(
        values
            .iter()
            .map(|value| BigInt::try_from(*value).unwrap())
            .collect(),
    )
}

fn coefficients(value: &Polynomial) -> Vec<(Sign, u32)> {
    value
        .coefficients()
        .iter()
        .map(|value| (value.sign(), value.magnitude().to_u32().unwrap()))
        .collect()
}

#[test]
fn required_quartic_factors_and_reconstructs_in_canonical_order() {
    let source = polynomial(&[6, 0, -5, 0, 1]);
    let factors = source
        .try_clone()
        .unwrap()
        .candidate()
        .unwrap()
        .square_free()
        .unwrap()
        .factor()
        .unwrap();
    assert_eq!(factors.len(), 2);
    assert_eq!(
        coefficients(factors[0].polynomial()),
        [(Sign::Negative, 3), (Sign::Zero, 0), (Sign::Positive, 1)]
    );
    assert_eq!(
        coefficients(factors[1].polynomial()),
        [(Sign::Negative, 2), (Sign::Zero, 0), (Sign::Positive, 1)]
    );
    assert_eq!(
        factors[0]
            .polynomial()
            .mul(factors[1].polynomial())
            .unwrap(),
        source
    );
}

#[test]
fn irreducible_and_recursive_factorizations_preserve_the_source() {
    let irreducible = polynomial(&[-2, 0, 1]);
    let factors = irreducible
        .try_clone()
        .unwrap()
        .candidate()
        .unwrap()
        .square_free()
        .unwrap()
        .factor()
        .unwrap();
    assert_eq!(factors.len(), 1);
    assert_eq!(factors[0].polynomial(), &irreducible);

    let source = polynomial(&[-6, 11, -6, 1]);
    let factors = source
        .try_clone()
        .unwrap()
        .candidate()
        .unwrap()
        .square_free()
        .unwrap()
        .factor()
        .unwrap();
    assert_eq!(factors.len(), 3);
    let mut product = Polynomial::one().unwrap();
    for factor in factors {
        product = product.mul(factor.polynomial()).unwrap();
    }
    assert_eq!(product, source);
}

#[test]
fn equal_degree_factors_use_highest_degree_coefficients_first() {
    let source = polynomial(&[2, 5, 2]);
    let factors = source
        .candidate()
        .unwrap()
        .square_free()
        .unwrap()
        .factor()
        .unwrap();

    assert_eq!(factors.len(), 2);
    assert_eq!(
        coefficients(factors[0].polynomial()),
        [(Sign::Positive, 2), (Sign::Positive, 1)]
    );
    assert_eq!(
        coefficients(factors[1].polynomial()),
        [(Sign::Positive, 1), (Sign::Positive, 2)]
    );
}
