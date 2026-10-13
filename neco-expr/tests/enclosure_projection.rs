use neco_bigint::{BigInt, Dyadic, DyadicEnclosure};
use neco_expr::{project_enclosure_f64, AbsoluteBits, EnclosureProjectionError};

/// AbsoluteBits(4) allows a maximum width of 2^-4. This enclosure's
/// width is one 2^-40 step above that limit, so the rejection must
/// come from an exact width comparison, not an epsilon-tolerant one.
#[test]
fn enclosure_projection_rejects_insufficient_width() {
    let upper_integer = (1_u64 << 36) + 1;
    let enclosure = DyadicEnclosure::new(
        Dyadic::new(BigInt::zero(), 0),
        Dyadic::new(BigInt::try_from(upper_integer).unwrap(), 40),
    )
    .unwrap();
    let bits = AbsoluteBits::new(4);

    let error = project_enclosure_f64(enclosure, bits).unwrap_err();

    assert_eq!(
        error,
        EnclosureProjectionError::InsufficientWidth {
            requested: bits,
            width: Dyadic::new(BigInt::try_from(upper_integer).unwrap(), 40),
        }
    );
}

/// The width of [0, 2^-4] is exactly the maximum AbsoluteBits(4) allows. Its midpoint 2^-5 is
/// exact in f64, so the expected value, error, and enclosure are literals derived from the
/// input rather than recomputed from the function's own output.
#[test]
fn enclosure_projection_accepts_sufficient_width() {
    let enclosure = DyadicEnclosure::new(
        Dyadic::new(BigInt::zero(), 0),
        Dyadic::new(BigInt::one().unwrap(), 4),
    )
    .unwrap();
    let bits = AbsoluteBits::new(4);

    let certified = project_enclosure_f64(enclosure, bits).expect("width exactly at the limit");

    assert_eq!(certified.value(), 0.03125_f64);
    assert_eq!(
        certified.absolute_error(),
        &Dyadic::new(BigInt::one().unwrap(), 5)
    );
    assert_eq!(
        certified.enclosure(),
        &DyadicEnclosure::new(
            Dyadic::new(BigInt::zero(), 0),
            Dyadic::new(BigInt::one().unwrap(), 4)
        )
        .unwrap()
    );
}

/// Enclosures far coarser than the f64 ULP at their magnitude: [1, 1 + 2^-20] at
/// AbsoluteBits(10), [3, 3 + 2^-24] at AbsoluteBits(8), and [0, 2^-30] at AbsoluteBits(16).
#[test]
fn rounded_value_never_leaves_a_width_sufficient_enclosure() {
    let cases: [(u64, u32, u64, u32, u32); 3] = [
        (1, 0, (1_u64 << 20) + 1, 20, 10),
        (3, 0, (3_u64 << 24) + 1, 24, 8),
        (0, 0, 1, 30, 16),
    ];
    for (lower_integer, lower_exponent, upper_integer, upper_exponent, bits) in cases {
        let lower = Dyadic::new(BigInt::try_from(lower_integer).unwrap(), lower_exponent);
        let upper = Dyadic::new(BigInt::try_from(upper_integer).unwrap(), upper_exponent);
        let enclosure = DyadicEnclosure::new(lower, upper).unwrap();
        let certified =
            project_enclosure_f64(enclosure, AbsoluteBits::new(bits)).expect("sufficient width");
        let selected = Dyadic::from_f64_exact(certified.value()).unwrap();
        assert!(certified.enclosure().lower() <= &selected);
        assert!(&selected <= certified.enclosure().upper());
    }
}
