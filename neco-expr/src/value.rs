use alloc::vec::Vec;

use neco_algnum::RealAlgebraic;
#[cfg(test)]
use neco_bigint::BigintError;
use neco_bigint::{ReducedRational, Sign};
use neco_formsum::FormSum;
#[cfg(test)]
use neco_monomial::MonomialErrorKind;
use neco_monomial::{Monomial, RawMonomial};

use crate::{AtomId, EvalError, ExprId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExactLayer {
    Monomial,
    FormSum,
    Algebraic,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExactValue {
    Monomial(Monomial),
    FormSum(FormSum),
    Algebraic(RealAlgebraic),
}

impl ExactValue {
    pub fn layer(&self) -> ExactLayer {
        match self {
            Self::Monomial(_) => ExactLayer::Monomial,
            Self::FormSum(_) => ExactLayer::FormSum,
            Self::Algebraic(_) => ExactLayer::Algebraic,
        }
    }

    pub fn is_zero(&self) -> bool {
        match self {
            Self::Monomial(value) => value.is_zero(),
            Self::FormSum(value) => value.is_zero(),
            Self::Algebraic(value) => algebraic_is_zero(value),
        }
    }

    pub fn try_clone(&self) -> Result<Self, EvalError> {
        match self {
            Self::Monomial(value) => Ok(Self::Monomial(clone_monomial(value)?)),
            Self::FormSum(value) => Ok(Self::FormSum(value.try_clone()?)),
            Self::Algebraic(value) => Ok(Self::Algebraic(value.try_clone()?)),
        }
    }
}

fn algebraic_is_zero(value: &RealAlgebraic) -> bool {
    value.is_zero()
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExprNode {
    Atom(AtomId),
    Neg(ExprId),
    Add(ExprId, ExprId),
    Sub(ExprId, ExprId),
    Mul(ExprId, ExprId),
    Div(ExprId, ExprId),
    Pow {
        base: ExprId,
        exponent: ReducedRational,
    },
}

impl ExprNode {
    pub fn try_clone(&self) -> Result<Self, EvalError> {
        match self {
            Self::Atom(atom) => Ok(Self::Atom(*atom)),
            Self::Neg(child) => Ok(Self::Neg(*child)),
            Self::Add(left, right) => Ok(Self::Add(*left, *right)),
            Self::Sub(left, right) => Ok(Self::Sub(*left, *right)),
            Self::Mul(left, right) => Ok(Self::Mul(*left, *right)),
            Self::Div(left, right) => Ok(Self::Div(*left, *right)),
            Self::Pow { base, exponent } => Ok(Self::Pow {
                base: *base,
                exponent: clone_exponent(exponent)?,
            }),
        }
    }
}

fn clone_exponent(value: &ReducedRational) -> Result<ReducedRational, EvalError> {
    #[cfg(test)]
    if clone_failure_is(CloneContact::Exponent) {
        return Err(EvalError::Bigint(BigintError::AllocationFailure {
            requested_limbs: 17,
        }));
    }
    value.try_clone().map_err(EvalError::Bigint)
}

fn clone_monomial(value: &Monomial) -> Result<Monomial, EvalError> {
    #[cfg(test)]
    if clone_failure_is(CloneContact::Monomial) {
        return Err(EvalError::Monomial(MonomialErrorKind::AllocationFailure {
            requested_elements: 19,
        }));
    }
    value.try_clone().map_err(EvalError::Monomial)
}

#[cfg(test)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum CloneContact {
    Exponent,
    Monomial,
}

#[cfg(test)]
std::thread_local! {
    static CLONE_FAILURE: core::cell::Cell<Option<CloneContact>> = const { core::cell::Cell::new(None) };
}

#[cfg(test)]
fn clone_failure_is(contact: CloneContact) -> bool {
    CLONE_FAILURE.with(|configured| configured.get() == Some(contact))
}

#[cfg(test)]
pub(crate) fn with_clone_failure<R>(contact: CloneContact, operation: impl FnOnce() -> R) -> R {
    CLONE_FAILURE.with(|configured| configured.set(Some(contact)));
    let result = operation();
    CLONE_FAILURE.with(|configured| configured.set(None));
    result
}

pub(crate) fn neg_exact(value: &ExactValue) -> Result<ExactValue, EvalError> {
    match value {
        ExactValue::Monomial(value) => {
            let negative_one = negative_one()?;
            Ok(ExactValue::Monomial(value.mul(&negative_one)?))
        }
        ExactValue::FormSum(value) => Ok(ExactValue::FormSum(FormSum::zero().sub(value)?)),
        ExactValue::Algebraic(value) => {
            let zero = RealAlgebraic::from_form_sum(&FormSum::zero())?;
            Ok(ExactValue::Algebraic(zero.sub(value)?))
        }
    }
}

pub(crate) fn add_exact(left: &ExactValue, right: &ExactValue) -> Result<ExactValue, EvalError> {
    binary_exact(BinaryOperation::Add, left, right)
}

pub(crate) fn sub_exact(left: &ExactValue, right: &ExactValue) -> Result<ExactValue, EvalError> {
    binary_exact(BinaryOperation::Sub, left, right)
}

pub(crate) fn mul_exact(left: &ExactValue, right: &ExactValue) -> Result<ExactValue, EvalError> {
    binary_exact(BinaryOperation::Mul, left, right)
}

pub(crate) fn div_exact(left: &ExactValue, right: &ExactValue) -> Result<ExactValue, EvalError> {
    if right.is_zero() {
        return Err(EvalError::DivisionByZero);
    }
    binary_exact(BinaryOperation::Div, left, right)
}

pub(crate) fn pow_exact(
    base: &ExactValue,
    exponent: &ReducedRational,
) -> Result<ExactValue, EvalError> {
    if base.is_zero() {
        if exponent.is_zero() {
            return Err(EvalError::UndefinedZeroPower);
        }
        if exponent.numerator().sign() == Sign::Negative {
            return Err(EvalError::ZeroToNegativePower);
        }
    }
    if exponent.is_zero() {
        return one_at_layer(base.layer());
    }
    let integer = exponent.denominator().to_u32() == Some(1);
    if !integer && !exponent.denominator().bit(0) && sign_of(base)? == Sign::Negative {
        return Err(EvalError::EvenRootOfNegative);
    }

    match base {
        ExactValue::Monomial(value) => Ok(ExactValue::Monomial(value.pow(exponent)?)),
        ExactValue::FormSum(value) if integer => Ok(ExactValue::FormSum(pow_form_sum_integer(
            value,
            exponent.numerator(),
        )?)),
        ExactValue::FormSum(value) => Ok(ExactValue::Algebraic(
            RealAlgebraic::from_form_sum(value)?.pow_rational(exponent)?,
        )),
        ExactValue::Algebraic(value) if integer => Ok(ExactValue::Algebraic(
            value.pow_integer(exponent.numerator())?,
        )),
        ExactValue::Algebraic(value) => Ok(ExactValue::Algebraic(value.pow_rational(exponent)?)),
    }
}

fn to_algebraic(value: &ExactValue) -> Result<RealAlgebraic, EvalError> {
    match value {
        ExactValue::Monomial(value) => Ok(RealAlgebraic::from_form_sum(&FormSum::from_monomial(
            value,
        )?)?),
        ExactValue::FormSum(value) => Ok(RealAlgebraic::from_form_sum(value)?),
        ExactValue::Algebraic(value) => Ok(value.try_clone()?),
    }
}

#[derive(Clone, Copy)]
enum BinaryOperation {
    Add,
    Sub,
    Mul,
    Div,
}

enum FormOperand<'a> {
    Monomial(&'a Monomial),
    FormSum(&'a FormSum),
}

impl FormOperand<'_> {
    fn into_value(self) -> Result<FormSum, EvalError> {
        match self {
            Self::Monomial(value) => Ok(FormSum::from_monomial(value)?),
            Self::FormSum(value) => Ok(value.try_clone()?),
        }
    }
}

enum BinaryRoute<'a> {
    Monomial(&'a Monomial, &'a Monomial),
    FormSum(FormOperand<'a>, FormOperand<'a>),
    Algebraic(&'a ExactValue, &'a ExactValue),
}

fn binary_route<'a>(
    operation: BinaryOperation,
    left: &'a ExactValue,
    right: &'a ExactValue,
) -> BinaryRoute<'a> {
    match (operation, left, right) {
        (_, ExactValue::Algebraic(_), _) | (_, _, ExactValue::Algebraic(_)) => {
            BinaryRoute::Algebraic(left, right)
        }
        (
            BinaryOperation::Mul | BinaryOperation::Div,
            ExactValue::Monomial(left),
            ExactValue::Monomial(right),
        ) => BinaryRoute::Monomial(left, right),
        (_, ExactValue::Monomial(left), ExactValue::Monomial(right)) => {
            BinaryRoute::FormSum(FormOperand::Monomial(left), FormOperand::Monomial(right))
        }
        (_, ExactValue::Monomial(left), ExactValue::FormSum(right)) => {
            BinaryRoute::FormSum(FormOperand::Monomial(left), FormOperand::FormSum(right))
        }
        (_, ExactValue::FormSum(left), ExactValue::Monomial(right)) => {
            BinaryRoute::FormSum(FormOperand::FormSum(left), FormOperand::Monomial(right))
        }
        (_, ExactValue::FormSum(left), ExactValue::FormSum(right)) => {
            BinaryRoute::FormSum(FormOperand::FormSum(left), FormOperand::FormSum(right))
        }
    }
}

fn binary_exact(
    operation: BinaryOperation,
    left: &ExactValue,
    right: &ExactValue,
) -> Result<ExactValue, EvalError> {
    match binary_route(operation, left, right) {
        BinaryRoute::Monomial(left, right) => match operation {
            BinaryOperation::Add => Ok(ExactValue::FormSum(
                FormSum::from_monomial(left)?.add(&FormSum::from_monomial(right)?)?,
            )),
            BinaryOperation::Sub => Ok(ExactValue::FormSum(
                FormSum::from_monomial(left)?.sub(&FormSum::from_monomial(right)?)?,
            )),
            BinaryOperation::Mul => Ok(ExactValue::Monomial(left.mul(right)?)),
            BinaryOperation::Div => Ok(ExactValue::Monomial(left.div(right)?)),
        },
        BinaryRoute::FormSum(left, right) => {
            let left = left.into_value()?;
            let right = right.into_value()?;
            match operation {
                BinaryOperation::Add => Ok(ExactValue::FormSum(left.add(&right)?)),
                BinaryOperation::Sub => Ok(ExactValue::FormSum(left.sub(&right)?)),
                BinaryOperation::Mul => Ok(ExactValue::FormSum(left.mul(&right)?)),
                BinaryOperation::Div => Ok(ExactValue::FormSum(left.div(&right)?)),
            }
        }
        BinaryRoute::Algebraic(left, right) => {
            let left = to_algebraic(left)?;
            let right = to_algebraic(right)?;
            match operation {
                BinaryOperation::Add => Ok(ExactValue::Algebraic(left.add(&right)?)),
                BinaryOperation::Sub => Ok(ExactValue::Algebraic(left.sub(&right)?)),
                BinaryOperation::Mul => Ok(ExactValue::Algebraic(left.mul(&right)?)),
                BinaryOperation::Div => Ok(ExactValue::Algebraic(left.div(&right)?)),
            }
        }
    }
}

fn sign_of(value: &ExactValue) -> Result<Sign, EvalError> {
    match value {
        ExactValue::Monomial(value) => Ok(value.sign()),
        ExactValue::FormSum(value) => Ok(value.sign()?),
        ExactValue::Algebraic(value) => Ok(value.sign()?),
    }
}

pub(crate) fn decide_zero(value: &ExactValue) -> bool {
    value.is_zero()
}

pub(crate) fn decide_equality(left: &ExactValue, right: &ExactValue) -> Result<bool, EvalError> {
    Ok(sub_exact(left, right)?.is_zero())
}

pub(crate) fn decide_sign(value: &ExactValue) -> Result<Sign, EvalError> {
    sign_of(value)
}

fn one_at_layer(layer: ExactLayer) -> Result<ExactValue, EvalError> {
    match layer {
        ExactLayer::Monomial => Ok(ExactValue::Monomial(Monomial::one())),
        ExactLayer::FormSum => Ok(ExactValue::FormSum(FormSum::one()?)),
        ExactLayer::Algebraic => Ok(ExactValue::Algebraic(RealAlgebraic::from_form_sum(
            &FormSum::one()?,
        )?)),
    }
}

fn negative_one() -> Result<Monomial, EvalError> {
    RawMonomial::negative(Vec::new())
        .normalize()
        .map_err(|errors| EvalError::Monomial(errors.into_parts().0))
}

fn pow_form_sum_integer(
    value: &FormSum,
    exponent: &neco_bigint::BigInt,
) -> Result<FormSum, EvalError> {
    let mut result = FormSum::one()?;
    let mut power = if exponent.sign() == Sign::Negative {
        FormSum::one()?.div(value)?
    } else {
        value.try_clone()?
    };
    let bit_len = exponent.magnitude().bit_len();
    for bit in 0..bit_len {
        if exponent.magnitude().bit(bit) {
            result = result.mul(&power)?;
        }
        if bit + 1 < bit_len {
            power = power.mul(&power)?;
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use neco_algnum::RealAlgebraic;
    use neco_bigint::{BigInt, BigUint, RawRational, ReducedRational};
    use neco_formsum::FormSum;
    use neco_monomial::{Monomial, RawMonomial, RawPower};

    use super::{add_exact, div_exact, mul_exact, neg_exact, pow_exact, sub_exact};
    use crate::{ExactLayer, ExactValue};

    fn layer_values(one: bool) -> [ExactValue; 3] {
        let monomial = if one {
            Monomial::one()
        } else {
            Monomial::zero()
        };
        let form = FormSum::from_monomial(&monomial).unwrap();
        let algebraic = RealAlgebraic::from_form_sum(&form).unwrap();
        [
            ExactValue::Monomial(monomial),
            ExactValue::FormSum(form),
            ExactValue::Algebraic(algebraic),
        ]
    }

    fn monomial(sign: bool, powers: &[(u32, i32)]) -> Monomial {
        let powers = powers
            .iter()
            .map(|&(base, power)| {
                RawPower::new(
                    BigUint::try_from(base).unwrap(),
                    RawRational::new(BigInt::try_from(power).unwrap(), BigUint::one().unwrap()),
                )
            })
            .collect();
        if sign {
            RawMonomial::positive(powers)
        } else {
            RawMonomial::negative(powers)
        }
        .normalize()
        .unwrap()
    }

    fn at_layer(value: Monomial, layer: ExactLayer) -> ExactValue {
        match layer {
            ExactLayer::Monomial => ExactValue::Monomial(value),
            ExactLayer::FormSum => ExactValue::FormSum(FormSum::from_monomial(&value).unwrap()),
            ExactLayer::Algebraic => {
                let form = FormSum::from_monomial(&value).unwrap();
                ExactValue::Algebraic(RealAlgebraic::from_form_sum(&form).unwrap())
            }
        }
    }

    fn integer_at_layer(value: u32, layer: ExactLayer) -> ExactValue {
        at_layer(monomial(true, &[(value, 1)]), layer)
    }

    fn exponent(numerator: i32, denominator: u32) -> ReducedRational {
        RawRational::new(
            BigInt::try_from(numerator).unwrap(),
            BigUint::try_from(denominator).unwrap(),
        )
        .reduce()
        .unwrap()
        .into_reduced()
    }

    #[test]
    fn production_binary_results_cover_every_operation_table_cell() {
        let layers = [
            ExactLayer::Monomial,
            ExactLayer::FormSum,
            ExactLayer::Algebraic,
        ];
        let operations = [
            (
                add_exact as fn(&ExactValue, &ExactValue) -> Result<ExactValue, crate::EvalError>,
                0_u8,
            ),
            (sub_exact, 1),
            (mul_exact, 2),
            (div_exact, 3),
        ];
        for left_layer in layers {
            for right_layer in layers {
                let add_sub = if left_layer == ExactLayer::Algebraic
                    || right_layer == ExactLayer::Algebraic
                {
                    ExactLayer::Algebraic
                } else {
                    ExactLayer::FormSum
                };
                let mul_div = if left_layer == ExactLayer::Algebraic
                    || right_layer == ExactLayer::Algebraic
                {
                    ExactLayer::Algebraic
                } else if left_layer == ExactLayer::Monomial && right_layer == ExactLayer::Monomial
                {
                    ExactLayer::Monomial
                } else {
                    ExactLayer::FormSum
                };
                for (operation, operation_id) in operations {
                    let left = integer_at_layer(2, left_layer);
                    let right = integer_at_layer(3, right_layer);
                    let result_layer = if operation_id >= 2 { mul_div } else { add_sub };
                    let expected_monomial = match operation_id {
                        0 => monomial(true, &[(5, 1)]),
                        1 => monomial(false, &[]),
                        2 => monomial(true, &[(2, 1), (3, 1)]),
                        3 => monomial(true, &[(2, 1), (3, -1)]),
                        _ => unreachable!(),
                    };
                    assert_eq!(
                        operation(&left, &right).unwrap(),
                        at_layer(expected_monomial, result_layer)
                    );
                }
            }
        }
    }

    #[test]
    fn production_unary_and_power_results_cover_every_layer() {
        let layers = [
            ExactLayer::Monomial,
            ExactLayer::FormSum,
            ExactLayer::Algebraic,
        ];
        let integer = exponent(2, 1);
        let rational = exponent(1, 2);
        for layer in layers {
            let value = integer_at_layer(2, layer);
            assert_eq!(
                neg_exact(&value).unwrap(),
                at_layer(monomial(false, &[(2, 1)]), layer)
            );
            assert_eq!(
                pow_exact(&value, &integer).unwrap(),
                integer_at_layer(4, layer)
            );
            let rational_layer = if layer == ExactLayer::Monomial {
                ExactLayer::Monomial
            } else {
                ExactLayer::Algebraic
            };
            assert_eq!(
                pow_exact(&value, &rational).unwrap(),
                at_layer(
                    monomial(true, &[(2, 1)]).pow(&rational).unwrap(),
                    rational_layer,
                )
            );
        }
    }

    #[test]
    fn structural_zero_is_decided_in_every_exact_layer() {
        for value in layer_values(false) {
            assert!(value.is_zero());
        }
        for value in layer_values(true) {
            assert!(!value.is_zero());
        }
    }
}
