use core::cmp::Ordering;

use neco_bigint::{
    BigInt, BigUint, BigintError, Dyadic, DyadicEnclosure, ExtendedGcd, RationalReduction,
    RawRational, ReducedRational, Sign,
};

type PublicDeclarations = (
    Sign,
    BigintError,
    BigUint,
    BigInt,
    ExtendedGcd,
    RawRational,
    RationalReduction,
    ReducedRational,
    Dyadic,
    DyadicEnclosure,
);

fn accepts_public_types(_: PublicDeclarations) {}

#[test]
fn crate_root_exports_all_ten_public_declarations() {
    let a = BigInt::try_from(30_i32).unwrap();
    let b = BigInt::try_from(21_i32).unwrap();
    let extended = a.extended_gcd(&b).unwrap();
    let raw = RawRational::new(
        BigInt::try_from(2_i32).unwrap(),
        BigUint::try_from(4_u32).unwrap(),
    );
    let reduction = raw.reduce().unwrap();
    let reduced = reduction.reduced().try_clone().unwrap();
    let dyadic = Dyadic::new(BigInt::try_from(1_i32).unwrap(), 1);
    let enclosure = DyadicEnclosure::new(dyadic.try_clone().unwrap(), dyadic).unwrap();
    accepts_public_types((
        Sign::Zero,
        BigintError::DivisionByZero,
        BigUint::zero(),
        BigInt::zero(),
        extended,
        raw,
        reduction,
        reduced,
        enclosure.lower().try_clone().unwrap(),
        enclosure,
    ));
}

#[test]
fn public_observers_do_not_expose_mutable_fields() {
    let raw = RawRational::new(
        BigInt::try_from(-2_i32).unwrap(),
        BigUint::try_from(4_u32).unwrap(),
    );
    let reduction = raw.reduce().unwrap();
    let reduced: &ReducedRational = reduction.reduced();
    assert_eq!(reduced.numerator().sign(), Sign::Negative);
    assert_eq!(reduced.denominator().to_u32(), Some(2));
    assert_eq!(reduced.cmp(reduced), Ordering::Equal);
    let owned: ReducedRational = reduction.into_reduced();
    assert_eq!(owned.denominator().to_u32(), Some(2));
}

#[test]
fn integer_conversions_are_fallible_and_normalized() {
    for value in [0_u64, 1, u32::MAX as u64, u64::MAX] {
        let converted = BigUint::try_from(value).unwrap();
        assert_eq!(converted.is_zero(), value == 0);
        assert!(converted.limbs_le().last().is_none_or(|limb| *limb != 0));
    }
    assert_eq!(
        BigInt::from_sign_magnitude(Sign::Negative, BigUint::zero()).sign(),
        Sign::Zero
    );
}

#[test]
fn errors_have_stable_public_display_text() {
    let cases = [
        (
            BigintError::CapacityOverflow,
            "required limb capacity exceeds usize",
        ),
        (
            BigintError::AllocationFailure { requested_limbs: 9 },
            "limb allocation failed",
        ),
        (
            BigintError::UnsignedUnderflow,
            "unsigned subtraction underflow",
        ),
        (BigintError::DivisionByZero, "division by zero"),
        (
            BigintError::NonExactDivision,
            "division has a nonzero remainder",
        ),
        (BigintError::ZeroDenominator, "rational denominator is zero"),
        (
            BigintError::NonFiniteFloat,
            "floating-point value is not finite",
        ),
        (
            BigintError::FloatOutOfRange,
            "exact value is outside finite f64 range",
        ),
        (
            BigintError::InvalidInterval,
            "dyadic enclosure endpoints are reversed",
        ),
        (
            BigintError::ExponentOverflow {
                required: BigUint::try_from(8_u32).unwrap(),
                maximum: 7,
            },
            "dyadic exponent exceeds u32",
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(error.to_string(), expected);
    }
}
