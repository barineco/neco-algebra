use neco_algnum::Polynomial;
use neco_bigint::BigInt;

fn polynomial(values: &[i32]) -> Polynomial {
    Polynomial::from_coefficients(
        values
            .iter()
            .map(|v| BigInt::try_from(*v).unwrap())
            .collect(),
    )
}

#[test]
fn irreducible_quadratic_roots_are_isolated_in_order_and_refine_monotonically() {
    let factors = polynomial(&[-2, 0, 1])
        .candidate()
        .unwrap()
        .square_free()
        .unwrap()
        .factor()
        .unwrap();
    let roots = factors[0].isolate_real_roots().unwrap();
    assert_eq!(roots.len(), 2);
    assert_eq!(roots[0].value().root_index().get(), 0);
    assert_eq!(roots[1].value().root_index().get(), 1);
    assert!(roots[0].enclosure().upper() <= roots[1].enclosure().lower());
    for root in roots {
        let before = root.enclosure().width().unwrap();
        let refined = root.value().enclose(8).unwrap();
        let after = refined.enclosure().width().unwrap();
        assert!(after <= before);
        assert_eq!(refined.value(), root.value());
    }
}

#[test]
fn reducible_cubic_preserves_all_three_values_in_numeric_order() {
    let roots = polynomial(&[0, -1, 0, 1])
        .candidate()
        .unwrap()
        .square_free()
        .unwrap()
        .isolate_real_roots()
        .unwrap();
    assert_eq!(roots.len(), 3);
    assert!(roots[0].enclosure().upper() < roots[1].enclosure().lower());
    assert!(roots[1].enclosure().upper() < roots[2].enclosure().lower());
    for root in roots {
        assert_eq!(root.value().root_index().get(), 0);
    }
}

#[test]
fn irreducible_three_real_root_polynomial_assigns_indices_zero_one_two() {
    let factors = polynomial(&[1, -3, 0, 1])
        .candidate()
        .unwrap()
        .square_free()
        .unwrap()
        .factor()
        .unwrap();
    assert_eq!(factors.len(), 1);
    let roots = factors[0].isolate_real_roots().unwrap();
    assert_eq!(
        roots
            .iter()
            .map(|root| root.value().root_index().get())
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
}
