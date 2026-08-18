use neco_bigint::{BigInt, BigUint, BigintError, Dyadic, DyadicEnclosure, RawRational, Sign};

fn uint(value: u32) -> BigUint {
    BigUint::try_from(value).expect("small test integer")
}

fn int(value: i32) -> BigInt {
    BigInt::try_from(value).expect("small test integer")
}

fn signed_value(value: &BigInt) -> i64 {
    let magnitude = i64::from(value.magnitude().to_u32().expect("small result"));
    match value.sign() {
        Sign::Negative => -magnitude,
        Sign::Zero => 0,
        Sign::Positive => magnitude,
    }
}

fn dyadic(integer: i64, exponent: u32) -> Dyadic {
    Dyadic::new(BigInt::try_from(integer).expect("test integer"), exponent)
}

fn negative(value: &Dyadic) -> Dyadic {
    value.mul(&dyadic(-1, 0)).expect("negated dyadic")
}

fn assert_signed_rounds(value: &Dyadic, magnitude_bits: u64) {
    assert_eq!(
        value.round_to_f64_ties_even().unwrap().to_bits(),
        magnitude_bits
    );
    let negative_bits = if magnitude_bits == 0 {
        0
    } else {
        magnitude_bits | (1_u64 << 63)
    };
    assert_eq!(
        negative(value).round_to_f64_ties_even().unwrap().to_bits(),
        negative_bits
    );
}

fn assert_tie_site(lower_bits: u64) -> usize {
    let upper_bits = lower_bits + 1;
    let lower = Dyadic::from_f64_exact(f64::from_bits(lower_bits)).unwrap();
    let upper = Dyadic::from_f64_exact(f64::from_bits(upper_bits)).unwrap();
    let midpoint = lower.midpoint(&upper).unwrap();
    let before = lower.midpoint(&midpoint).unwrap();
    let after = midpoint.midpoint(&upper).unwrap();
    let tie_bits = if lower_bits & 1 == 0 {
        lower_bits
    } else {
        upper_bits
    };
    for (value, expected) in [
        (&before, lower_bits),
        (&midpoint, tie_bits),
        (&after, upper_bits),
    ] {
        assert_signed_rounds(value, expected);
    }
    6
}

#[test]
fn required_integer_and_rational_vectors() {
    let (q, r) = uint(7).div_rem(&uint(3)).unwrap();
    assert_eq!((q.to_u32(), r.to_u32()), (Some(2), Some(1)));
    let (q, r) = int(-7).div_rem_euclid(&int(3)).unwrap();
    assert_eq!((signed_value(&q), r.to_u32()), (-3, Some(2)));

    let witness = int(30).extended_gcd(&int(21)).unwrap();
    let bezout = int(30)
        .mul(witness.x())
        .unwrap()
        .add(&int(21).mul(witness.y()).unwrap())
        .unwrap();
    assert_eq!(witness.gcd().to_u32(), Some(3));
    assert_eq!(signed_value(&bezout), 3);

    let half = RawRational::new(int(2), uint(4)).reduce().unwrap();
    assert_eq!(signed_value(half.reduced().numerator()), 1);
    assert_eq!(half.reduced().denominator().to_u32(), Some(2));
    let zero = RawRational::new(int(0), uint(7)).reduce().unwrap();
    assert!(zero.reduced().numerator().is_zero());
    assert_eq!(zero.reduced().denominator().to_u32(), Some(1));
}

#[test]
fn non_allocation_public_failures_have_vectors() {
    assert_eq!(
        uint(1).shl_bits(usize::MAX),
        Err(BigintError::CapacityOverflow)
    );
    assert_eq!(
        uint(1).checked_sub(&uint(2)),
        Err(BigintError::UnsignedUnderflow)
    );
    assert_eq!(
        uint(1).div_rem(&BigUint::zero()),
        Err(BigintError::DivisionByZero)
    );
    assert_eq!(
        uint(5).exact_div(&uint(2)),
        Err(BigintError::NonExactDivision)
    );
    assert_eq!(
        RawRational::new(int(1), BigUint::zero()).reduce(),
        Err(BigintError::ZeroDenominator)
    );
    assert_eq!(
        DyadicEnclosure::new(Dyadic::new(int(1), 0), Dyadic::new(int(0), 0)),
        Err(BigintError::InvalidInterval)
    );
    assert_eq!(
        Dyadic::new(int(1), u32::MAX).mul(&Dyadic::new(int(1), 1)),
        Err(BigintError::ExponentOverflow {
            required: BigUint::try_from(u64::from(u32::MAX) + 1).unwrap(),
            maximum: u32::MAX
        })
    );
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            Dyadic::from_f64_exact(value),
            Err(BigintError::NonFiniteFloat)
        );
    }
    let beyond_max = Dyadic::from_f64_exact(f64::MAX)
        .unwrap()
        .add(&Dyadic::new(int(1), 0))
        .unwrap();
    assert_eq!(
        beyond_max.round_to_f64_ties_even(),
        Err(BigintError::FloatOutOfRange)
    );
}

#[test]
fn normal_float_matrix_roundtrips_all_28_644_patterns() {
    let fractions = [
        0,
        1,
        (1_u64 << 51) - 1,
        1_u64 << 51,
        (1_u64 << 51) + 1,
        (1_u64 << 52) - 2,
        (1_u64 << 52) - 1,
    ];
    let mut count = 0;
    for exponent in 1_u64..=2046 {
        for fraction in fractions {
            for sign in [0_u64, 1_u64 << 63] {
                let bits = sign | (exponent << 52) | fraction;
                let value = f64::from_bits(bits);
                let rounded = Dyadic::from_f64_exact(value)
                    .unwrap()
                    .round_to_f64_ties_even()
                    .unwrap();
                assert_eq!(rounded.to_bits(), bits, "pattern {bits:#018x}");
                count += 1;
            }
        }
    }
    assert_eq!(count, 28_644);
}

#[test]
fn transition_set_has_32_cases_and_tie_set_has_24_cases() {
    let mut transitions = 0;

    for (numerator, expected) in [(0, 0), (1, 0), (2, 0), (3, 1), (4, 1)] {
        assert_signed_rounds(&dyadic(numerator, 1076), expected);
        transitions += 2;
    }

    let maximum_subnormal = 0x000f_ffff_ffff_ffff;
    let minimum_normal = 0x0010_0000_0000_0000;
    let midpoint = 2_i64 * ((1_i64 << 53) - 1);
    let subnormal_normal = [
        (4_i64 * ((1_i64 << 52) - 1), maximum_subnormal),
        (midpoint - 1, maximum_subnormal),
        (midpoint, minimum_normal),
        (midpoint + 1, minimum_normal),
        (4_i64 * (1_i64 << 52), minimum_normal),
    ];
    for (numerator, expected) in subnormal_normal {
        assert_signed_rounds(&dyadic(numerator, 1076), expected);
        transitions += 2;
    }

    let maximum = Dyadic::from_f64_exact(f64::MAX).unwrap();
    assert_signed_rounds(&maximum, f64::MAX.to_bits());
    let outside = maximum.add(&dyadic(1, 0)).unwrap();
    assert_eq!(
        outside.round_to_f64_ties_even(),
        Err(BigintError::FloatOutOfRange)
    );
    assert_eq!(
        negative(&outside).round_to_f64_ties_even(),
        Err(BigintError::FloatOutOfRange)
    );
    transitions += 4;

    for lower_bits in [1_u64, 0x3fff_ffff_ffff_ffff] {
        let lower = Dyadic::from_f64_exact(f64::from_bits(lower_bits)).unwrap();
        let upper = Dyadic::from_f64_exact(f64::from_bits(lower_bits + 1)).unwrap();
        let midpoint = lower.midpoint(&upper).unwrap();
        let before = lower.midpoint(&midpoint).unwrap();
        assert_signed_rounds(&before, lower_bits);
        assert_signed_rounds(&midpoint, lower_bits + 1);
        transitions += 4;
    }
    assert_eq!(transitions, 32);

    let ties = [2_u64, 3_u64, 0x3ff0_0000_0000_0000, 0x3ff0_0000_0000_0001]
        .into_iter()
        .map(assert_tie_site)
        .sum::<usize>();
    assert_eq!(ties, 24);
}

#[test]
fn splitmix64_roundtrips_one_million_accepted_finite_patterns() {
    let mut state = 0x4e45_434f_u64;
    let mut accepted = 0_u32;
    while accepted < 1_000_000 {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut bits = state;
        bits = (bits ^ (bits >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        bits = (bits ^ (bits >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        bits ^= bits >> 31;
        let value = f64::from_bits(bits);
        if !value.is_finite() {
            continue;
        }
        let rounded = Dyadic::from_f64_exact(value)
            .unwrap()
            .round_to_f64_ties_even()
            .unwrap();
        assert_eq!(rounded.to_bits(), bits, "accepted index {accepted}");
        accepted += 1;
    }
}
