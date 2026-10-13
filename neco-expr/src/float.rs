use core::cmp::Ordering;

#[cfg(test)]
use neco_algnum::RepresentationResource;
use neco_algnum::{AlgnumError, IsolatingInterval};
use neco_bigint::{
    BigInt, BigUint, BigintError, Dyadic, DyadicEnclosure, RawRational, ReducedRational, Sign,
};
use neco_formsum::{FormSum, FormSumErrorKind, RawFormSum, RawTerm};
use neco_monomial::RawMonomial;

use crate::{AbsoluteBits, ExactValue, ExprId, IsolationCache, StorageError};

const SIGN_MASK: u64 = 1_u64 << 63;
const MAX_FINITE_BITS: u64 = 0x7fef_ffff_ffff_ffff;
const NEGATIVE_KEY_START: u64 = 0x0010_0000_0000_0000;
const NEGATIVE_COUNT: u64 = 0x7fef_ffff_ffff_ffff;
const LAST_ORDERED: u64 = NEGATIVE_COUNT + MAX_FINITE_BITS;

#[derive(Debug)]
pub struct CertifiedF64 {
    value: f64,
    enclosure: DyadicEnclosure,
    absolute_error: Dyadic,
}

impl CertifiedF64 {
    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn enclosure(&self) -> &DyadicEnclosure {
        &self.enclosure
    }

    pub fn absolute_error(&self) -> &Dyadic {
        &self.absolute_error
    }

    pub fn enclosure_width(&self) -> Result<Dyadic, BigintError> {
        self.enclosure.width()
    }

    pub fn try_clone(&self) -> Result<Self, BigintError> {
        Ok(Self {
            value: self.value,
            enclosure: self.enclosure.try_clone()?,
            absolute_error: self.absolute_error.try_clone()?,
        })
    }

    pub(crate) fn resolve(
        value: &ExactValue,
        expr: ExprId,
        bits: AbsoluteBits,
        isolation: &mut IsolationCache,
    ) -> Result<Self, FloatError> {
        let comparable = ComparableExact::new(value)?;
        let rounded = nearest_f64(&comparable)?;
        let enclosure = enclose_exact(value, expr, bits, isolation)?;
        let exact_float = Dyadic::from_f64_exact(rounded).map_err(FloatError::Bigint)?;
        let absolute_error =
            absolute_error(&exact_float, &enclosure).map_err(FloatError::Bigint)?;
        Ok(Self {
            value: rounded,
            enclosure,
            absolute_error,
        })
    }

    pub(crate) fn resolve_for_requirement(
        value: &ExactValue,
        expr: ExprId,
        bits: AbsoluteBits,
        isolation: &mut IsolationCache,
    ) -> Result<Self, PrecisionResolutionError> {
        let resolved =
            Self::resolve(value, expr, bits, isolation).map_err(PrecisionResolutionError::Float)?;
        let width =
            resolved
                .enclosure
                .width()
                .map_err(|error| PrecisionResolutionError::WidthFailure {
                    requested: bits,
                    error,
                })?;
        let maximum = Dyadic::new(
            BigInt::one()
                .map_err(|error| PrecisionResolutionError::Float(FloatError::Bigint(error)))?,
            bits.get(),
        );
        if width > maximum {
            return Err(PrecisionResolutionError::Width {
                requested: bits,
                width,
            });
        }
        Ok(resolved)
    }
}

impl PartialEq for CertifiedF64 {
    fn eq(&self, other: &Self) -> bool {
        self.value.to_bits() == other.value.to_bits()
            && self.enclosure == other.enclosure
            && self.absolute_error == other.absolute_error
    }
}

impl Eq for CertifiedF64 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionPolicy {
    absolute_bits: AbsoluteBits,
}

impl ProjectionPolicy {
    pub const fn new(absolute_bits: AbsoluteBits) -> Self {
        Self { absolute_bits }
    }

    pub const fn absolute_bits(self) -> AbsoluteBits {
        self.absolute_bits
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct CertifiedScalarProjection {
    policy: ProjectionPolicy,
    certified: CertifiedF64,
}

impl CertifiedScalarProjection {
    pub const fn policy(&self) -> ProjectionPolicy {
        self.policy
    }

    pub fn certified(&self) -> &CertifiedF64 {
        &self.certified
    }

    pub fn value(&self) -> f64 {
        self.certified.value()
    }

    pub fn enclosure(&self) -> &DyadicEnclosure {
        self.certified.enclosure()
    }

    pub fn absolute_error(&self) -> &Dyadic {
        self.certified.absolute_error()
    }

    pub fn try_clone(&self) -> Result<Self, ScalarProjectionError> {
        Ok(Self {
            policy: self.policy,
            certified: self
                .certified
                .try_clone()
                .map_err(ScalarProjectionError::Bigint)?,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ScalarProjectionError {
    FloatOutOfRange,
    Bigint(BigintError),
    Algnum(AlgnumError),
    Storage(StorageError),
}

pub fn project_exact_value_f64(
    value: &ExactValue,
    policy: ProjectionPolicy,
) -> Result<CertifiedScalarProjection, ScalarProjectionError> {
    let comparable = ComparableExact::new(value).map_err(ScalarProjectionError::from_float)?;
    let rounded = nearest_f64(&comparable).map_err(ScalarProjectionError::from_float)?;
    let enclosure = enclose_exact_without_cache(value, policy.absolute_bits())
        .map_err(ScalarProjectionError::from_float)?;
    let exact_float = Dyadic::from_f64_exact(rounded).map_err(ScalarProjectionError::Bigint)?;
    let absolute_error =
        absolute_error(&exact_float, &enclosure).map_err(ScalarProjectionError::Bigint)?;
    Ok(CertifiedScalarProjection {
        policy,
        certified: CertifiedF64 {
            value: rounded,
            enclosure,
            absolute_error,
        },
    })
}

impl ScalarProjectionError {
    fn from_float(error: FloatError) -> Self {
        match error {
            FloatError::OutOfRange => Self::FloatOutOfRange,
            FloatError::Bigint(error) | FloatError::EnclosureBigint { error, .. } => {
                Self::Bigint(error)
            }
            FloatError::Algnum(error) | FloatError::EnclosureAlgnum { error, .. } => {
                Self::Algnum(error)
            }
            FloatError::Storage(error) => Self::Storage(error),
        }
    }
}

impl core::fmt::Display for ScalarProjectionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::FloatOutOfRange => formatter.write_str("exact value is outside finite f64 range"),
            Self::Bigint(error) => error.fmt(formatter),
            Self::Algnum(error) => error.fmt(formatter),
            Self::Storage(error) => error.fmt(formatter),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ScalarProjectionError {}

#[derive(Debug, Eq, PartialEq)]
pub enum EnclosureProjectionError {
    Bigint(BigintError),
    InsufficientWidth {
        requested: AbsoluteBits,
        width: Dyadic,
    },
}

impl core::fmt::Display for EnclosureProjectionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Bigint(error) => error.fmt(formatter),
            Self::InsufficientWidth { requested, width } => write!(
                formatter,
                "enclosure width {width:?} exceeds the maximum width for {requested:?}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EnclosureProjectionError {}

pub fn project_enclosure_f64(
    enclosure: DyadicEnclosure,
    bits: AbsoluteBits,
) -> Result<CertifiedF64, EnclosureProjectionError> {
    let width = enclosure
        .width()
        .map_err(EnclosureProjectionError::Bigint)?;
    let maximum = Dyadic::new(
        BigInt::one().map_err(EnclosureProjectionError::Bigint)?,
        bits.get(),
    );
    if width > maximum {
        return Err(EnclosureProjectionError::InsufficientWidth {
            requested: bits,
            width,
        });
    }
    let midpoint = enclosure
        .midpoint()
        .map_err(EnclosureProjectionError::Bigint)?;
    let value = midpoint
        .round_to_f64_ties_even()
        .map_err(EnclosureProjectionError::Bigint)?;
    let exact_float = Dyadic::from_f64_exact(value).map_err(EnclosureProjectionError::Bigint)?;
    let absolute_error =
        absolute_error(&exact_float, &enclosure).map_err(EnclosureProjectionError::Bigint)?;
    Ok(CertifiedF64 {
        value,
        enclosure,
        absolute_error,
    })
}

#[derive(Debug)]
pub(crate) enum FloatError {
    OutOfRange,
    Bigint(BigintError),
    Algnum(AlgnumError),
    EnclosureBigint {
        requested: AbsoluteBits,
        error: BigintError,
    },
    EnclosureAlgnum {
        requested: AbsoluteBits,
        error: AlgnumError,
    },
    Storage(StorageError),
}

pub(crate) enum PrecisionResolutionError {
    Float(FloatError),
    Width {
        requested: AbsoluteBits,
        width: Dyadic,
    },
    WidthFailure {
        requested: AbsoluteBits,
        error: BigintError,
    },
}

enum ComparableExact {
    Rational(ReducedRational),
    FormSum(FormSum),
    Algebraic(IsolatingInterval),
}

impl ComparableExact {
    fn new(value: &ExactValue) -> Result<Self, FloatError> {
        match value {
            ExactValue::Monomial(value) => FormSum::from_monomial(value)
                .map_err(map_formsum_error)
                .and_then(Self::from_form_sum),
            ExactValue::FormSum(value) => value
                .try_clone()
                .map_err(map_formsum_error)
                .and_then(Self::from_form_sum),
            ExactValue::Algebraic(value) => value
                .enclose(64)
                .map(Self::Algebraic)
                .map_err(FloatError::Algnum),
        }
    }

    fn from_form_sum(value: FormSum) -> Result<Self, FloatError> {
        match value.terms() {
            [] => ReducedRational::from_bigint(BigInt::zero())
                .map(Self::Rational)
                .map_err(FloatError::Bigint),
            [(basis, coefficient)] if basis.factors().is_empty() => coefficient
                .try_clone()
                .map(Self::Rational)
                .map_err(FloatError::Bigint),
            _ => Ok(Self::FormSum(value)),
        }
    }

    fn compare(&self, rhs: &Dyadic) -> Result<Ordering, FloatError> {
        match self {
            Self::Rational(value) => compare_rational_dyadic(value, rhs),
            Self::FormSum(value) => {
                let rhs = form_sum_from_dyadic(rhs)?;
                value
                    .sub(&rhs)
                    .and_then(|difference| difference.sign())
                    .map(|sign| match sign {
                        Sign::Negative => Ordering::Less,
                        Sign::Zero => Ordering::Equal,
                        Sign::Positive => Ordering::Greater,
                    })
                    .map_err(map_formsum_error)
            }
            Self::Algebraic(interval) if interval.enclosure().upper() < rhs => Ok(Ordering::Less),
            Self::Algebraic(interval) if interval.enclosure().lower() > rhs => {
                Ok(Ordering::Greater)
            }
            Self::Algebraic(interval) => interval
                .value()
                .compare_dyadic(rhs)
                .map_err(FloatError::Algnum),
        }
    }
}

fn compare_rational_dyadic(value: &ReducedRational, rhs: &Dyadic) -> Result<Ordering, FloatError> {
    let numerator = value.numerator();
    let left = if rhs.exponent() == 0 || numerator.is_zero() {
        numerator.try_clone()
    } else {
        numerator
            .magnitude()
            .shl_bits(rhs.exponent() as usize)
            .map(|magnitude| BigInt::from_sign_magnitude(numerator.sign(), magnitude))
    }
    .map_err(FloatError::Bigint)?;
    let denominator = value
        .denominator()
        .try_clone()
        .map(|magnitude| BigInt::from_sign_magnitude(Sign::Positive, magnitude))
        .map_err(FloatError::Bigint)?;
    let right = rhs
        .integer()
        .mul(&denominator)
        .map_err(FloatError::Bigint)?;
    Ok(left.cmp(&right))
}

fn form_sum_from_dyadic(value: &Dyadic) -> Result<FormSum, FloatError> {
    let denominator = BigUint::one()
        .and_then(|one| one.shl_bits(value.exponent() as usize))
        .map_err(FloatError::Bigint)?;
    let numerator = value.integer().try_clone().map_err(FloatError::Bigint)?;
    RawFormSum::new(alloc::vec![RawTerm::new(
        RawRational::new(numerator, denominator),
        RawMonomial::positive(alloc::vec::Vec::new()),
    )])
    .normalize()
    .map_err(|errors| map_formsum_error(errors.into_parts().0))
}

fn map_formsum_error(error: FormSumErrorKind) -> FloatError {
    match error {
        FormSumErrorKind::Bigint(error) => FloatError::Bigint(error),
        error => FloatError::Algnum(AlgnumError::FormSum(error)),
    }
}

fn nearest_f64(value: &ComparableExact) -> Result<f64, FloatError> {
    let minimum = Dyadic::from_f64_exact(f64::from_bits(SIGN_MASK | MAX_FINITE_BITS))
        .map_err(FloatError::Bigint)?;
    let maximum =
        Dyadic::from_f64_exact(f64::from_bits(MAX_FINITE_BITS)).map_err(FloatError::Bigint)?;
    match value.compare(&minimum)? {
        Ordering::Less => return Err(FloatError::OutOfRange),
        Ordering::Equal => return Ok(f64::from_bits(SIGN_MASK | MAX_FINITE_BITS)),
        Ordering::Greater => {}
    }
    match value.compare(&maximum)? {
        Ordering::Greater => return Err(FloatError::OutOfRange),
        Ordering::Equal => return Ok(f64::from_bits(MAX_FINITE_BITS)),
        Ordering::Less => {}
    }

    let mut lower = 0_u64;
    let mut upper = LAST_ORDERED;
    while lower < upper {
        let middle = lower + (upper - lower) / 2 + 1;
        let candidate = exact_ordered(middle)?;
        match value.compare(&candidate)? {
            Ordering::Less => upper = middle - 1,
            Ordering::Equal => return Ok(f64::from_bits(ordered_to_bits(middle))),
            Ordering::Greater => lower = middle,
        }
    }

    let lower_bits = ordered_to_bits(lower);
    let upper_bits = ordered_to_bits(lower + 1);
    let lower_value =
        Dyadic::from_f64_exact(f64::from_bits(lower_bits)).map_err(FloatError::Bigint)?;
    let upper_value =
        Dyadic::from_f64_exact(f64::from_bits(upper_bits)).map_err(FloatError::Bigint)?;
    let midpoint = lower_value
        .midpoint(&upper_value)
        .map_err(FloatError::Bigint)?;
    let chosen = match value.compare(&midpoint)? {
        Ordering::Less => lower_bits,
        Ordering::Greater => upper_bits,
        Ordering::Equal if lower_bits & 1 == 0 => lower_bits,
        Ordering::Equal => upper_bits,
    };
    Ok(f64::from_bits(if chosen == SIGN_MASK { 0 } else { chosen }))
}

fn exact_ordered(ordered: u64) -> Result<Dyadic, FloatError> {
    Dyadic::from_f64_exact(f64::from_bits(ordered_to_bits(ordered))).map_err(FloatError::Bigint)
}

fn ordered_to_bits(ordered: u64) -> u64 {
    if ordered < NEGATIVE_COUNT {
        !(NEGATIVE_KEY_START + ordered)
    } else {
        ordered - NEGATIVE_COUNT
    }
}

fn enclose_exact(
    value: &ExactValue,
    expr: ExprId,
    bits: AbsoluteBits,
    isolation: &mut IsolationCache,
) -> Result<DyadicEnclosure, FloatError> {
    match value {
        ExactValue::Algebraic(value) => {
            if let Some(cached) = isolation.get(expr, bits) {
                return cached.enclosure().try_clone().map_err(|error| {
                    FloatError::EnclosureBigint {
                        requested: bits,
                        error,
                    }
                });
            }
            let interval = enclose_algebraic(value, bits.get())
                .map_err(|error| enclosure_error(error, bits))?;
            let enclosure =
                interval
                    .enclosure()
                    .try_clone()
                    .map_err(|error| FloatError::EnclosureBigint {
                        requested: bits,
                        error,
                    })?;
            isolation
                .insert(expr, bits, interval)
                .map_err(FloatError::Storage)?;
            Ok(enclosure)
        }
        _ => enclose_exact_without_cache(value, bits).map_err(|error| enclosure_error(error, bits)),
    }
}

fn enclosure_error(error: FloatError, requested: AbsoluteBits) -> FloatError {
    match error {
        FloatError::Bigint(error) => FloatError::EnclosureBigint { requested, error },
        FloatError::Algnum(error) => FloatError::EnclosureAlgnum { requested, error },
        other => other,
    }
}

fn enclose_exact_without_cache(
    value: &ExactValue,
    bits: AbsoluteBits,
) -> Result<DyadicEnclosure, FloatError> {
    match value {
        ExactValue::Monomial(value) => {
            let value = FormSum::from_monomial(value).map_err(map_formsum_error)?;
            enclose_form_sum(&value, bits.get())
        }
        ExactValue::FormSum(value) => enclose_form_sum(value, bits.get()),
        ExactValue::Algebraic(value) => enclose_algebraic(value, bits.get())?
            .enclosure()
            .try_clone()
            .map_err(FloatError::Bigint),
    }
}

fn enclose_form_sum(value: &FormSum, bits: u32) -> Result<DyadicEnclosure, FloatError> {
    #[cfg(test)]
    if enclosure_failure_is(EnclosureContact::FormSum) {
        let required = BigUint::try_from(33_u8).map_err(FloatError::Bigint)?;
        return Err(FloatError::Bigint(BigintError::ExponentOverflow {
            required,
            maximum: 32,
        }));
    }
    value.enclose(bits).map_err(map_formsum_error)
}

fn enclose_algebraic(
    value: &neco_algnum::RealAlgebraic,
    bits: u32,
) -> Result<IsolatingInterval, FloatError> {
    #[cfg(test)]
    ENCLOSURE_CALLS.with(|calls| calls.set(calls.get() + 1));
    #[cfg(test)]
    if enclosure_failure_is(EnclosureContact::Algebraic) {
        let required = BigUint::try_from(41_u8).map_err(FloatError::Bigint)?;
        let maximum = BigUint::try_from(40_u8).map_err(FloatError::Bigint)?;
        return Err(FloatError::Algnum(AlgnumError::RepresentationLimit {
            resource: RepresentationResource::RootDegree,
            required,
            maximum,
        }));
    }
    value.enclose(bits).map_err(FloatError::Algnum)
}

#[cfg(test)]
#[derive(Clone, Copy, Eq, PartialEq)]
enum EnclosureContact {
    FormSum,
    Algebraic,
}

#[cfg(test)]
std::thread_local! {
    static ENCLOSURE_FAILURE: core::cell::Cell<Option<EnclosureContact>> = const { core::cell::Cell::new(None) };
    static ENCLOSURE_CALLS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

#[cfg(test)]
fn enclosure_failure_is(contact: EnclosureContact) -> bool {
    ENCLOSURE_FAILURE.with(|configured| configured.get() == Some(contact))
}

#[cfg(test)]
fn with_enclosure_failure<R>(contact: EnclosureContact, operation: impl FnOnce() -> R) -> R {
    ENCLOSURE_FAILURE.with(|configured| configured.set(Some(contact)));
    let result = operation();
    ENCLOSURE_FAILURE.with(|configured| configured.set(None));
    result
}

#[cfg(test)]
fn with_enclosure_call_count<R>(operation: impl FnOnce() -> R) -> (R, usize) {
    ENCLOSURE_CALLS.with(|calls| calls.set(0));
    let result = operation();
    let calls = ENCLOSURE_CALLS.with(core::cell::Cell::get);
    (result, calls)
}

fn absolute_error(value: &Dyadic, enclosure: &DyadicEnclosure) -> Result<Dyadic, BigintError> {
    let lower = abs_dyadic(&value.sub(enclosure.lower())?)?;
    let upper = abs_dyadic(&enclosure.upper().sub(value)?)?;
    Ok(if lower >= upper { lower } else { upper })
}

fn abs_dyadic(value: &Dyadic) -> Result<Dyadic, BigintError> {
    let magnitude: BigUint = value.integer().abs()?;
    Ok(Dyadic::new(
        BigInt::from_sign_magnitude(Sign::Positive, magnitude),
        value.exponent(),
    ))
}

#[cfg(test)]
mod tests {
    use neco_algnum::{AlgnumError, RealAlgebraic, RepresentationResource};
    use neco_bigint::{BigInt, BigUint, BigintError, Dyadic, RawRational, Sign};
    use neco_formsum::{FormSum, RawFormSum, RawTerm};
    use neco_monomial::{Monomial, RawMonomial, RawPower};

    use super::{
        form_sum_from_dyadic, nearest_f64, ordered_to_bits, with_enclosure_call_count,
        with_enclosure_failure, CertifiedF64, ComparableExact, EnclosureContact, FloatError,
        PrecisionResolutionError, LAST_ORDERED, MAX_FINITE_BITS, NEGATIVE_COUNT, SIGN_MASK,
    };
    use crate::{
        project_exact_value_f64, AbsoluteBits, Assignments, AtomId, AtomStore, ConsumerId,
        ExactValue, ExprGraph, ExprId, ExprNode, IsolationCache, PrecisionRequirements,
        ProjectionPolicy, ResolveError, Resolver,
    };

    #[test]
    fn ordered_bits_exclude_negative_zero() {
        assert_eq!(ordered_to_bits(NEGATIVE_COUNT - 1), 0x8000_0000_0000_0001);
        assert_eq!(ordered_to_bits(NEGATIVE_COUNT), 0);
        assert_eq!(ordered_to_bits(NEGATIVE_COUNT + 1), 1);
    }

    #[test]
    fn ordered_bits_cover_signed_normal_subnormal_zero_and_finite_endpoints() {
        let negative_one_bits = (-1.0_f64).to_bits();
        let negative_one_ordered = (!negative_one_bits) - super::NEGATIVE_KEY_START;
        let positive_one_ordered = NEGATIVE_COUNT + 1.0_f64.to_bits();
        let observations = [
            ordered_to_bits(0),
            ordered_to_bits(negative_one_ordered),
            ordered_to_bits(NEGATIVE_COUNT - 1),
            ordered_to_bits(NEGATIVE_COUNT),
            ordered_to_bits(NEGATIVE_COUNT + 1),
            ordered_to_bits(positive_one_ordered),
            ordered_to_bits(LAST_ORDERED),
        ];
        assert_eq!(
            observations,
            [
                (-f64::MAX).to_bits(),
                (-1.0_f64).to_bits(),
                0x8000_0000_0000_0001,
                0,
                1,
                1.0_f64.to_bits(),
                f64::MAX.to_bits(),
            ]
        );
        assert!(observations.windows(2).all(|pair| {
            f64::from_bits(pair[0]) < f64::from_bits(pair[1]) || (pair[0] == 0 && pair[1] == 1)
        }));
    }

    fn resolve_with_injected_enclosure(
        value: ExactValue,
        contact: EnclosureContact,
    ) -> ResolveError {
        let mut graph = ExprGraph::new();
        let expression = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
        let mut atoms = AtomStore::new();
        atoms.insert(AtomId::new(0), value).unwrap();
        let mut requirements = PrecisionRequirements::new();
        requirements
            .insert(ConsumerId::new(0), AbsoluteBits::new(20))
            .unwrap();
        let mut assignments = Assignments::new();
        assignments.insert(ConsumerId::new(0), expression).unwrap();
        let (_, _, resolved) = with_enclosure_failure(contact, || {
            Resolver::new().resolve_all(&graph, &atoms, &requirements, &assignments)
        })
        .unwrap();
        match resolved.get(ConsumerId::new(0)).unwrap() {
            Err(error) => match error {
                ResolveError::Bigint(BigintError::ExponentOverflow { required, maximum }) => {
                    ResolveError::Bigint(BigintError::ExponentOverflow {
                        required: required.try_clone().unwrap(),
                        maximum: *maximum,
                    })
                }
                ResolveError::Algnum(AlgnumError::RepresentationLimit {
                    resource,
                    required,
                    maximum,
                }) => ResolveError::Algnum(AlgnumError::RepresentationLimit {
                    resource: *resource,
                    required: required.try_clone().unwrap(),
                    maximum: maximum.try_clone().unwrap(),
                }),
                _ => panic!("unexpected enclosure error: {error:?}"),
            },
            Ok(_) => panic!("injected enclosure failure was not observed"),
        }
    }

    #[test]
    fn form_sum_enclosure_bigint_failure_keeps_the_payload() {
        assert_eq!(
            resolve_with_injected_enclosure(
                ExactValue::FormSum(FormSum::one().unwrap()),
                EnclosureContact::FormSum,
            ),
            ResolveError::Bigint(BigintError::ExponentOverflow {
                required: BigUint::try_from(33_u8).unwrap(),
                maximum: 32,
            })
        );
    }

    #[test]
    fn algebraic_enclosure_failure_keeps_the_payload() {
        let form = FormSum::one().unwrap();
        assert_eq!(
            resolve_with_injected_enclosure(
                ExactValue::Algebraic(RealAlgebraic::from_form_sum(&form).unwrap()),
                EnclosureContact::Algebraic,
            ),
            ResolveError::Algnum(AlgnumError::RepresentationLimit {
                resource: RepresentationResource::RootDegree,
                required: BigUint::try_from(41_u8).unwrap(),
                maximum: BigUint::try_from(40_u8).unwrap(),
            })
        );
    }

    #[test]
    fn high_level_resolution_keeps_representation_limit_as_precision_cause() {
        let form = FormSum::one().unwrap();
        let exact = ExactValue::Algebraic(RealAlgebraic::from_form_sum(&form).unwrap());
        let mut isolation = IsolationCache::new();
        let result = with_enclosure_failure(EnclosureContact::Algebraic, || {
            CertifiedF64::resolve_for_requirement(
                &exact,
                ExprId::new(0),
                AbsoluteBits::new(80),
                &mut isolation,
            )
        });
        assert!(matches!(
            result,
            Err(PrecisionResolutionError::Float(
                FloatError::EnclosureAlgnum {
                    requested,
                    error: AlgnumError::RepresentationLimit { .. },
                }
            )) if requested.get() == 80
        ));
    }

    #[test]
    fn algebraic_enclosure_is_computed_once_for_each_expression_and_precision() {
        let form = FormSum::one().unwrap();
        let mut graph = ExprGraph::new();
        let expression = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
        let mut atoms = AtomStore::new();
        atoms
            .insert(
                AtomId::new(0),
                ExactValue::Algebraic(RealAlgebraic::from_form_sum(&form).unwrap()),
            )
            .unwrap();
        let mut requirements = PrecisionRequirements::new();
        let mut assignments = Assignments::new();
        for (consumer, bits) in [(0, 20), (1, 20), (2, 40)] {
            requirements
                .insert(ConsumerId::new(consumer), AbsoluteBits::new(bits))
                .unwrap();
            assignments
                .insert(ConsumerId::new(consumer), expression)
                .unwrap();
        }
        let (result, calls) = with_enclosure_call_count(|| {
            Resolver::new().resolve_all(&graph, &atoms, &requirements, &assignments)
        });
        assert!(result.is_ok());
        assert_eq!(calls, 2);
    }

    #[test]
    fn scalar_projection_carries_its_policy_value_enclosure_and_error() {
        let policy = ProjectionPolicy::new(AbsoluteBits::new(20));
        let exact = ExactValue::FormSum(FormSum::one().unwrap());
        let projection = project_exact_value_f64(&exact, policy).expect("scalar projection");
        let selected = neco_bigint::Dyadic::from_f64_exact(projection.value()).unwrap();

        assert_eq!(projection.policy(), policy);
        assert_eq!(projection.value(), 1.0);
        assert!(projection.enclosure().contains_dyadic(&selected));
        let zero = BigInt::zero();
        assert_eq!(projection.absolute_error().integer(), &zero);
    }

    fn rational(numerator: i64, denominator: u64) -> FormSum {
        RawFormSum::new(alloc::vec![RawTerm::new(
            RawRational::new(
                BigInt::try_from(numerator).unwrap(),
                BigUint::try_from(denominator).unwrap(),
            ),
            RawMonomial::positive(alloc::vec::Vec::new()),
        )])
        .normalize()
        .unwrap()
    }

    fn power_of_two(exponent: i32) -> FormSum {
        let shifted = BigUint::one()
            .unwrap()
            .shl_bits(exponent.unsigned_abs() as usize)
            .unwrap();
        let (numerator, denominator) = if exponent >= 0 {
            (
                BigInt::from_sign_magnitude(Sign::Positive, shifted),
                BigUint::one().unwrap(),
            )
        } else {
            (BigInt::one().unwrap(), shifted)
        };
        RawFormSum::new(alloc::vec![RawTerm::new(
            RawRational::new(numerator, denominator),
            RawMonomial::positive(alloc::vec::Vec::new()),
        )])
        .normalize()
        .unwrap()
    }

    fn float_form_sum(bits: u64) -> FormSum {
        form_sum_from_dyadic(&Dyadic::from_f64_exact(f64::from_bits(bits)).unwrap()).unwrap()
    }

    fn scaled(value: &FormSum, numerator: i64, denominator: u64) -> FormSum {
        value.mul(&rational(numerator, denominator)).unwrap()
    }

    fn rational_samples() -> alloc::vec::Vec<FormSum> {
        let mut samples = alloc::vec::Vec::new();
        for (numerator, denominator) in [
            (0, 1),
            (1, 1),
            (-1, 1),
            (7, 1),
            (-123_456_789, 1),
            (1, 2),
            (-3, 8),
            (5, 1024),
            (1, 3),
            (-1, 3),
            (22, 7),
            (-22, 7),
            (1, 10),
            (-1, 10),
            (9_007_199_254_740_993, 1),
            (-9_007_199_254_740_993, 1),
            (9_007_199_254_740_993, 8),
            (1, 1_000_003),
        ] {
            samples.push(rational(numerator, denominator));
        }

        let tiny = scaled(&power_of_two(-1200), 1, 3);
        let ordered_bits = [
            0_u64,
            1,
            2,
            3,
            0x000f_ffff_ffff_fffe,
            0x000f_ffff_ffff_ffff,
            0x0010_0000_0000_0000,
            0x0010_0000_0000_0001,
            0x3fef_ffff_ffff_ffff,
            0x3ff0_0000_0000_0000,
            0x3ff0_0000_0000_0001,
            0x3ff0_0000_0000_0002,
            0x7fef_ffff_ffff_fffd,
            0x7fef_ffff_ffff_fffe,
            MAX_FINITE_BITS,
        ];
        for bits in ordered_bits {
            for sign in [0, SIGN_MASK] {
                let value = float_form_sum(bits | sign);
                let next = float_form_sum(if bits == MAX_FINITE_BITS {
                    bits | sign
                } else {
                    (bits + 1) | sign
                });
                let midpoint = scaled(&value.add(&next).unwrap(), 1, 2);
                samples.push(value.try_clone().unwrap());
                samples.push(midpoint.try_clone().unwrap());
                samples.push(midpoint.add(&tiny).unwrap());
                samples.push(midpoint.sub(&tiny).unwrap());
                samples.push(value.add(&tiny).unwrap());
                samples.push(value.sub(&tiny).unwrap());
            }
        }

        let maximum = float_form_sum(MAX_FINITE_BITS);
        let half_ulp = power_of_two(970);
        let above_midpoint = maximum.add(&half_ulp).unwrap();
        for sign in [1, -1] {
            samples.push(scaled(&above_midpoint, sign, 1));
            samples.push(scaled(&above_midpoint.add(&tiny).unwrap(), sign, 1));
            samples.push(scaled(&above_midpoint.sub(&tiny).unwrap(), sign, 1));
            samples.push(scaled(&maximum.add(&rational(1, 1)).unwrap(), sign, 1));
            samples.push(scaled(&maximum.sub(&rational(1, 1)).unwrap(), sign, 1));
            samples.push(scaled(&power_of_two(1024), sign, 1));
            samples.push(scaled(&power_of_two(1025), sign, 1));
            samples.push(scaled(&power_of_two(-1075), sign, 1));
            samples.push(scaled(&power_of_two(-1076), sign, 1));
            samples.push(scaled(&power_of_two(-1074), sign, 1));
            samples.push(scaled(&power_of_two(-1075).add(&tiny).unwrap(), sign, 1));
            samples.push(scaled(&power_of_two(-1075).sub(&tiny).unwrap(), sign, 1));
            samples.push(scaled(&power_of_two(-1022), sign, 1));
            samples.push(scaled(&power_of_two(-1023), sign, 1));
        }
        samples
    }

    fn outcome(result: Result<f64, FloatError>) -> Result<u64, &'static str> {
        match result {
            Ok(value) => Ok(value.to_bits()),
            Err(FloatError::OutOfRange) => Err("out of range"),
            Err(_) => Err("other"),
        }
    }

    #[test]
    fn rational_form_matches_form_sum_comparison_and_nearest() {
        let probes: alloc::vec::Vec<Dyadic> = [
            0_u64,
            1,
            0x000f_ffff_ffff_ffff,
            0x0010_0000_0000_0000,
            0x3ff0_0000_0000_0000,
            MAX_FINITE_BITS,
            SIGN_MASK | 1,
            SIGN_MASK | 0x3ff0_0000_0000_0000,
            SIGN_MASK | MAX_FINITE_BITS,
        ]
        .into_iter()
        .map(|bits| Dyadic::from_f64_exact(f64::from_bits(bits)).unwrap())
        .chain([
            Dyadic::new(BigInt::try_from(3_i32).unwrap(), 7),
            Dyadic::new(BigInt::try_from(-5_i32).unwrap(), 1100),
        ])
        .collect();

        let samples = rational_samples();
        assert!(samples.len() > 200);
        for sample in &samples {
            let fast =
                ComparableExact::new(&ExactValue::FormSum(sample.try_clone().unwrap())).unwrap();
            assert!(matches!(fast, ComparableExact::Rational(_)));
            let general = ComparableExact::FormSum(sample.try_clone().unwrap());
            for probe in &probes {
                assert_eq!(
                    fast.compare(probe).unwrap(),
                    general.compare(probe).unwrap(),
                    "{sample:?} {probe:?}"
                );
            }
            assert_eq!(
                outcome(nearest_f64(&fast)),
                outcome(nearest_f64(&general)),
                "{sample:?}"
            );
        }
    }

    #[test]
    fn monomial_and_zero_select_the_rational_form() {
        let zero = ComparableExact::new(&ExactValue::Monomial(Monomial::zero())).unwrap();
        assert!(matches!(zero, ComparableExact::Rational(_)));
        let one = ComparableExact::new(&ExactValue::Monomial(Monomial::one())).unwrap();
        assert!(matches!(one, ComparableExact::Rational(_)));
        assert_eq!(nearest_f64(&one).unwrap().to_bits(), 1.0_f64.to_bits());
        assert_eq!(nearest_f64(&zero).unwrap().to_bits(), 0);

        let root = RawMonomial::positive(alloc::vec![RawPower::new(
            BigUint::try_from(2_u32).unwrap(),
            RawRational::new(BigInt::one().unwrap(), BigUint::try_from(2_u32).unwrap()),
        )])
        .normalize()
        .unwrap();
        let radical = ComparableExact::new(&ExactValue::Monomial(root)).unwrap();
        assert!(matches!(radical, ComparableExact::FormSum(_)));
        let mixed = rational(1, 1)
            .add(&FormSum::from_monomial(&Monomial::one()).unwrap())
            .unwrap();
        assert!(matches!(
            ComparableExact::new(&ExactValue::FormSum(mixed)).unwrap(),
            ComparableExact::Rational(_)
        ));
    }
}
