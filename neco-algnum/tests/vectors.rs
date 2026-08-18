use neco_algnum::Polynomial;
use neco_bigint::{BigInt, Sign};

fn polynomial(values: &[i32]) -> Polynomial {
    Polynomial::from_coefficients(
        values
            .iter()
            .map(|v| BigInt::try_from(*v).unwrap())
            .collect(),
    )
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

#[test]
fn f0_factorization_vectors_match_all_terminal_coefficients() {
    let quartic = polynomial(&[6, 0, -5, 0, 1]);
    let factors = quartic
        .candidate()
        .unwrap()
        .square_free()
        .unwrap()
        .factor()
        .unwrap();
    assert_eq!(
        signed_coefficients(factors[0].polynomial()),
        [(Sign::Negative, 3), (Sign::Zero, 0), (Sign::Positive, 1)]
    );
    assert_eq!(
        signed_coefficients(factors[1].polynomial()),
        [(Sign::Negative, 2), (Sign::Zero, 0), (Sign::Positive, 1)]
    );

    let irreducible = polynomial(&[-2, 0, 1]);
    let factors = irreducible
        .candidate()
        .unwrap()
        .square_free()
        .unwrap()
        .factor()
        .unwrap();
    assert_eq!(factors.len(), 1);
    assert_eq!(
        signed_coefficients(factors[0].polynomial()),
        [(Sign::Negative, 2), (Sign::Zero, 0), (Sign::Positive, 1)]
    );
}

#[test]
fn f0_root_vector_returns_three_sorted_exact_values() {
    let roots = polynomial(&[0, -1, 0, 1])
        .candidate()
        .unwrap()
        .square_free()
        .unwrap()
        .isolate_real_roots()
        .unwrap();
    assert_eq!(roots.len(), 3);
    assert!(roots
        .windows(2)
        .all(|pair| pair[0].enclosure().upper() < pair[1].enclosure().lower()));
    let minimal_polynomials: Vec<Vec<(Sign, u32)>> = roots
        .iter()
        .map(|root| signed_coefficients(root.value().minimal_polynomial().polynomial()))
        .collect();
    assert_eq!(
        minimal_polynomials,
        [
            vec![(Sign::Positive, 1), (Sign::Positive, 1)],
            vec![(Sign::Zero, 0), (Sign::Positive, 1)],
            vec![(Sign::Negative, 1), (Sign::Positive, 1)],
        ]
    );
}
