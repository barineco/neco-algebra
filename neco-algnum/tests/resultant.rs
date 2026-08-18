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

#[test]
fn repeated_resultant_candidate_is_square_free_before_root_selection() {
    let square_free = polynomial(&[0, 0, 1])
        .candidate()
        .unwrap()
        .square_free()
        .unwrap();
    assert_eq!(square_free.polynomial().degree(), Some(1));
    assert_eq!(
        square_free.polynomial().coefficients()[0].sign(),
        Sign::Zero
    );
    assert_eq!(
        square_free.polynomial().coefficients()[1]
            .magnitude()
            .to_u32(),
        Some(1)
    );
}

#[test]
fn square_free_conversion_removes_the_quotient_content() {
    let square_free = polynomial(&[1, 4, 4])
        .candidate()
        .unwrap()
        .square_free()
        .unwrap();
    assert_eq!(square_free.polynomial(), &polynomial(&[1, 2]));
}

#[test]
fn square_free_conversion_preserves_all_distinct_roots() {
    let source = polynomial(&[0, 2, -3, 0, 1]);
    let square_free = source.candidate().unwrap().square_free().unwrap();
    let roots = square_free.isolate_real_roots().unwrap();
    assert_eq!(roots.len(), 3);
    assert!(roots
        .windows(2)
        .all(|pair| pair[0].enclosure().upper() < pair[1].enclosure().lower()));
}
