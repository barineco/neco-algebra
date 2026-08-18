use core::cmp::Ordering;

use neco_bigint::{BigInt, BigUint, BigintError, Dyadic, DyadicEnclosure, RawRational, Sign};

fn uint(value: u32) -> BigUint {
    BigUint::try_from(value).expect("small test integer")
}

fn int(value: i32) -> BigInt {
    BigInt::try_from(value).expect("small test integer")
}

fn signed_value(value: &BigInt) -> i64 {
    let magnitude = i64::from(value.magnitude().to_u32().expect("small test result"));
    match value.sign() {
        Sign::Negative => -magnitude,
        Sign::Zero => 0,
        Sign::Positive => magnitude,
    }
}

fn limbs(values: &[u32]) -> BigUint {
    BigUint::from_limbs_le(values.to_vec()).expect("valid test limbs")
}

#[derive(Clone, Copy)]
struct ReferenceRational {
    numerator: i64,
    denominator: i64,
}

impl ReferenceRational {
    fn new(mut numerator: i64, mut denominator: i64) -> Self {
        if numerator == 0 {
            return Self {
                numerator: 0,
                denominator: 1,
            };
        }
        let mut left = numerator.unsigned_abs();
        let mut right = denominator.unsigned_abs();
        while right != 0 {
            (left, right) = (right, left % right);
        }
        let gcd = left as i64;
        numerator /= gcd;
        denominator /= gcd;
        if denominator < 0 {
            numerator = -numerator;
            denominator = -denominator;
        }
        Self {
            numerator,
            denominator,
        }
    }

    fn add(self, rhs: Self) -> Self {
        Self::new(
            self.numerator * rhs.denominator + rhs.numerator * self.denominator,
            self.denominator * rhs.denominator,
        )
    }

    fn sub(self, rhs: Self) -> Self {
        Self::new(
            self.numerator * rhs.denominator - rhs.numerator * self.denominator,
            self.denominator * rhs.denominator,
        )
    }

    fn mul(self, rhs: Self) -> Self {
        Self::new(
            self.numerator * rhs.numerator,
            self.denominator * rhs.denominator,
        )
    }

    fn div(self, rhs: Self) -> Self {
        Self::new(
            self.numerator * rhs.denominator,
            self.denominator * rhs.numerator,
        )
    }

    fn pow(self, exponent: i32) -> Self {
        if exponent == 0 {
            return Self::new(1, 1);
        }
        let power = exponent.unsigned_abs();
        if exponent < 0 {
            Self::new(self.denominator.pow(power), self.numerator.pow(power))
        } else {
            Self::new(self.numerator.pow(power), self.denominator.pow(power))
        }
    }

    fn floor(self) -> i64 {
        self.numerator.div_euclid(self.denominator)
    }

    fn ceil(self) -> i64 {
        -(-self.numerator).div_euclid(self.denominator)
    }
}

fn rational(value: ReferenceRational) -> neco_bigint::ReducedRational {
    RawRational::new(int(value.numerator as i32), uint(value.denominator as u32))
        .reduce()
        .unwrap()
        .into_reduced()
}

fn assert_rational_eq(actual: &neco_bigint::ReducedRational, expected: ReferenceRational) {
    assert_eq!(signed_value(actual.numerator()), expected.numerator);
    assert_eq!(
        actual.denominator().to_u32(),
        Some(expected.denominator as u32)
    );
}

#[test]
fn natural_arithmetic_preserves_values_and_normal_form() {
    let values = [0, 1, 2, 31, 32, 0xffff_ffff];
    for left in values {
        for right in values {
            let a = uint(left);
            let b = uint(right);
            let sum = a.add(&b).expect("sum");
            assert_eq!(
                sum,
                BigUint::try_from(u64::from(left) + u64::from(right)).unwrap()
            );
            assert!(sum.limbs_le().last().is_none_or(|limb| *limb != 0));
            let product = a.mul(&b).expect("product");
            assert_eq!(
                product,
                BigUint::try_from(u64::from(left) * u64::from(right)).unwrap()
            );
        }
    }
    assert_eq!(
        BigUint::from_limbs_le(vec![7, 0, 0]).unwrap().limbs_le(),
        &[7]
    );
    assert_eq!(
        uint(1).checked_sub(&uint(2)),
        Err(BigintError::UnsignedUnderflow)
    );
}

#[test]
fn multi_limb_arithmetic_matches_independent_limb_vectors() {
    let carry = limbs(&[u32::MAX, u32::MAX, u32::MAX])
        .add(&limbs(&[1]))
        .unwrap();
    assert_eq!(carry.limbs_le(), &[0, 0, 0, 1]);

    let borrow = limbs(&[0, 0, 0, 1]).checked_sub(&limbs(&[1])).unwrap();
    assert_eq!(borrow.limbs_le(), &[u32::MAX, u32::MAX, u32::MAX]);

    let product = limbs(&[u32::MAX, u32::MAX]).mul(&limbs(&[2, 1])).unwrap();
    assert_eq!(product.limbs_le(), &[0xffff_fffe, 0xffff_fffe, 1, 1]);

    let shifted = limbs(&[0x89ab_cdef, 0x0123_4567]).shl_bits(36).unwrap();
    assert_eq!(shifted.limbs_le(), &[0, 0x9abc_def0, 0x1234_5678]);
}

#[test]
fn multi_limb_long_division_preserves_the_equation_and_expected_vectors() {
    let dividend = limbs(&[
        0x8f42_a49c,
        0x605f_08d5,
        0x2e0b_a572,
        0xb119_a451,
        0x0113_366a,
    ]);
    let divisor = limbs(&[0x2222_2223, 0x1111_1111]);
    let (quotient, remainder) = dividend.div_rem(&divisor).unwrap();

    assert_eq!(
        quotient.limbs_le(),
        &[0x90ab_cdef, 0x5060_7080, 0x1020_3040]
    );
    assert_eq!(remainder.limbs_le(), &[0xdead_beef]);
    assert_eq!(
        quotient.mul(&divisor).unwrap().add(&remainder).unwrap(),
        dividend
    );
    assert!(remainder < divisor);
}

#[test]
fn large_gcd_and_bezout_witness_match_independent_values() {
    let common = limbs(&[0x1111_1111, 0x9abc_def0, 0x1234_5678]);
    let left_magnitude = limbs(&[0xffff_fffd, 0x3333_3032, 0x3333_3333, 3]);
    let right_magnitude = limbs(&[0xdddd_dddc, 0xeca8_6241, 0xfdb9_7530, 1]);
    assert_eq!(left_magnitude.gcd(&right_magnitude).unwrap(), common);

    let left = BigInt::from_sign_magnitude(Sign::Positive, left_magnitude);
    let right = BigInt::from_sign_magnitude(Sign::Positive, right_magnitude);
    let witness = left.extended_gcd(&right).unwrap();
    assert_eq!(witness.gcd().limbs_le(), common.limbs_le());
    assert_eq!(witness.x(), &int(5));
    assert_eq!(witness.y(), &int(-8));
    assert_eq!(
        left.mul(witness.x())
            .unwrap()
            .add(&right.mul(witness.y()).unwrap())
            .unwrap(),
        BigInt::from_sign_magnitude(Sign::Positive, common)
    );
}

#[test]
fn large_reduced_rational_order_uses_exact_cross_products() {
    let greater = RawRational::new(
        BigInt::from_sign_magnitude(Sign::Positive, limbs(&[1, 0, 1])),
        limbs(&[u32::MAX, 1]),
    )
    .reduce()
    .unwrap()
    .into_reduced();
    let lesser = RawRational::new(
        BigInt::from_sign_magnitude(Sign::Positive, limbs(&[u32::MAX, u32::MAX])),
        limbs(&[5, 2]),
    )
    .reduce()
    .unwrap()
    .into_reduced();

    assert_eq!(greater.cmp(&lesser), Ordering::Greater);
    assert_eq!(lesser.cmp(&greater), Ordering::Less);
    assert_eq!(greater.cmp(&greater), Ordering::Equal);
}

#[test]
fn try_clone_preserves_an_independent_multi_limb_vector() {
    let original = limbs(&[0x0123_4567, 0x89ab_cdef, 0xfedc_ba98, 0x7654_3210]);
    let cloned = original.try_clone().unwrap();
    assert_eq!(
        cloned.limbs_le(),
        &[0x0123_4567, 0x89ab_cdef, 0xfedc_ba98, 0x7654_3210]
    );
    assert_eq!(original.limbs_le(), cloned.limbs_le());
}

#[test]
fn euclidean_division_covers_sign_size_and_exactness_product() {
    let dividends = [0, 2, 3, 7, 8, -2, -3, -7, -8];
    let divisors = [3, -3, 7, -7];
    for dividend in dividends {
        for divisor in divisors {
            let (quotient, remainder) = int(dividend).div_rem_euclid(&int(divisor)).unwrap();
            let q = signed_value(&quotient);
            let r = i64::from(remainder.to_u32().unwrap());
            assert_eq!(i64::from(dividend), q * i64::from(divisor) + r);
            assert!((0..i64::from(divisor.abs())).contains(&r));
        }
    }
    for dividend in [-7, 0, 7] {
        assert_eq!(
            int(dividend).div_rem_euclid(&BigInt::zero()),
            Err(BigintError::DivisionByZero)
        );
    }
}

#[test]
fn gcd_lcm_and_bezout_witnesses_preserve_their_equations() {
    for (a, b) in [(0, 0), (0, 9), (30, 21), (35, 15), (97, 89)] {
        let ua = uint(a);
        let ub = uint(b);
        let gcd = ua.gcd(&ub).unwrap();
        if !gcd.is_zero() {
            assert!(ua.div_rem(&gcd).unwrap().1.is_zero());
            assert!(ub.div_rem(&gcd).unwrap().1.is_zero());
        }
        let lcm = ua.lcm(&ub).unwrap();
        if a == 0 || b == 0 {
            assert!(lcm.is_zero());
        } else {
            assert_eq!(
                u64::from(gcd.to_u32().unwrap()) * u64::from(lcm.to_u32().unwrap()),
                u64::from(a) * u64::from(b)
            );
        }
        let witness = int(a as i32).extended_gcd(&int(b as i32)).unwrap();
        let lhs = int(a as i32)
            .mul(witness.x())
            .unwrap()
            .add(&int(b as i32).mul(witness.y()).unwrap())
            .unwrap();
        assert_eq!(lhs.magnitude(), witness.gcd());
        assert_ne!(lhs.sign(), Sign::Negative);
    }
}

#[test]
fn reduction_witness_reconstructs_input_and_proves_reduced_form() {
    for (numerator, denominator) in [(0, 7), (2, 4), (-18, 24), (35, 15)] {
        let reduction = RawRational::new(int(numerator), uint(denominator))
            .reduce()
            .unwrap();
        let rebuilt_num = reduction
            .reduced()
            .numerator()
            .mul(&BigInt::from_sign_magnitude(
                Sign::Positive,
                reduction.gcd().try_clone().unwrap(),
            ))
            .unwrap();
        let rebuilt_den = reduction
            .reduced()
            .denominator()
            .mul(reduction.gcd())
            .unwrap();
        assert_eq!(&rebuilt_num, reduction.input().numerator());
        assert_eq!(&rebuilt_den, reduction.input().denominator());
        assert_eq!(
            reduction
                .reduced()
                .numerator()
                .magnitude()
                .gcd(reduction.reduced().denominator())
                .unwrap(),
            uint(1)
        );
        assert!(!reduction.reduced().denominator().is_zero());
    }
}

#[test]
fn rational_operations_match_independent_integer_reference() {
    let numerators = [-17, -7, -2, -1, 0, 1, 2, 7, 17];
    let denominators = [1, 2, 3, 5, 8];
    for numerator in numerators {
        for denominator in denominators {
            let left_reference = ReferenceRational::new(numerator, denominator);
            let left = rational(left_reference);
            assert_eq!(signed_value(&left.floor().unwrap()), left_reference.floor());
            assert_eq!(signed_value(&left.ceil().unwrap()), left_reference.ceil());
            for bits in [0, 1, 2, 5] {
                let scale = 1_i64 << bits;
                let floor = left_reference
                    .numerator
                    .saturating_mul(scale)
                    .div_euclid(left_reference.denominator);
                let ceil = -(-left_reference.numerator.saturating_mul(scale))
                    .div_euclid(left_reference.denominator);
                assert_eq!(
                    left.dyadic_floor(bits).unwrap(),
                    Dyadic::new(int(floor as i32), bits)
                );
                assert_eq!(
                    left.dyadic_ceil(bits).unwrap(),
                    Dyadic::new(int(ceil as i32), bits)
                );
            }
            for exponent in [-5, -2, -1, 0, 1, 2, 5] {
                if left_reference.numerator == 0 && exponent < 0 {
                    assert_eq!(left.pow_i32(exponent), Err(BigintError::DivisionByZero));
                } else {
                    assert_rational_eq(
                        &left.pow_i32(exponent).unwrap(),
                        left_reference.pow(exponent),
                    );
                }
            }
            for right_numerator in numerators {
                for right_denominator in denominators {
                    let right_reference =
                        ReferenceRational::new(right_numerator, right_denominator);
                    let right = rational(right_reference);
                    assert_rational_eq(
                        &left.add(&right).unwrap(),
                        left_reference.add(right_reference),
                    );
                    assert_rational_eq(
                        &left.sub(&right).unwrap(),
                        left_reference.sub(right_reference),
                    );
                    assert_rational_eq(
                        &left.mul(&right).unwrap(),
                        left_reference.mul(right_reference),
                    );
                    if right_reference.numerator == 0 {
                        assert_eq!(left.div(&right), Err(BigintError::DivisionByZero));
                    } else {
                        assert_rational_eq(
                            &left.div(&right).unwrap(),
                            left_reference.div(right_reference),
                        );
                    }
                }
            }
        }
    }

    let negative_one = rational(ReferenceRational::new(-1, 1));
    assert_rational_eq(
        &negative_one.pow_i32(i32::MIN).unwrap(),
        ReferenceRational::new(1, 1),
    );
    let zero = rational(ReferenceRational::new(0, 1));
    assert_eq!(zero.pow_i32(i32::MIN), Err(BigintError::DivisionByZero));
}

#[test]
fn dyadic_enclosure_preserves_order_width_midpoint_and_membership() {
    let lower = Dyadic::new(int(-3), 2);
    let upper = Dyadic::new(int(5), 2);
    let enclosure = DyadicEnclosure::new(lower, upper).unwrap();
    assert_eq!(enclosure.width().unwrap(), Dyadic::new(int(2), 0));
    assert_eq!(enclosure.midpoint().unwrap(), Dyadic::new(int(1), 2));
    assert!(enclosure.contains_dyadic(&Dyadic::new(int(0), 0)));
    assert!(enclosure.contains_dyadic(enclosure.lower()));
    assert!(enclosure.contains_dyadic(enclosure.upper()));
    assert!(!enclosure.contains_dyadic(&Dyadic::new(int(3), 0)));
}
