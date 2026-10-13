use neco_bigint::{BigInt, BigUint, Dyadic, RawRational};
use neco_expr::{
    project_exact_value_f64, AbsoluteBits, ExactValue, ProjectionPolicy, ScalarProjectionError,
};
use neco_formsum::{RawFormSum, RawTerm};
use neco_monomial::RawMonomial;

#[test]
fn exact_form_sum_above_f64_range_is_rejected() {
    let maximum = Dyadic::from_f64_exact(f64::MAX).expect("finite f64");
    let denominator = BigUint::one()
        .expect("one")
        .shl_bits(maximum.exponent() as usize)
        .expect("dyadic denominator");
    let form_sum = RawFormSum::new(vec![
        RawTerm::new(
            RawRational::new(maximum.integer().try_clone().expect("integer"), denominator),
            RawMonomial::positive(Vec::new()),
        ),
        RawTerm::new(
            RawRational::new(
                BigInt::try_from(1_i64).expect("one"),
                BigUint::one().expect("one"),
            ),
            RawMonomial::positive(Vec::new()),
        ),
    ])
    .normalize()
    .expect("constant form sum");
    assert_eq!(
        project_exact_value_f64(
            &ExactValue::FormSum(form_sum),
            ProjectionPolicy::new(AbsoluteBits::new(2)),
        ),
        Err(ScalarProjectionError::FloatOutOfRange)
    );
}
