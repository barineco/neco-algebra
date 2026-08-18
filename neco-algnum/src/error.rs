use alloc::vec::Vec;
use core::fmt;

use neco_bigint::{BigUint, BigintError};
use neco_formsum::FormSumErrorKind;

#[cfg(test)]
extern crate std;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RepresentationResource {
    RootDegree,
    PolynomialDegree,
    CoefficientCount,
    SylvesterDimension,
    SylvesterElementCount,
}

impl fmt::Display for RepresentationResource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::RootDegree => "root degree",
            Self::PolynomialDegree => "polynomial degree",
            Self::CoefficientCount => "coefficient count",
            Self::SylvesterDimension => "Sylvester dimension",
            Self::SylvesterElementCount => "Sylvester element count",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AllocationResource {
    PolynomialCoefficients,
    RationalCoefficients,
    EvaluationPoints,
    Divisors,
    ProductDigits,
    FactorCandidates,
    Factors,
    SturmSequence,
    RootIntervals,
    SylvesterElements,
    Permutation,
    ResultantCoefficients,
    RootCandidates,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AllocationContact {
    PolynomialOne,
    PolynomialClone,
    PolynomialMul,
    PolynomialDerivative,
    PolynomialPrimitivePart,
    PolynomialToRational,
    PolynomialAddSub,
    PolynomialSingleCoefficient,
    RationalClone,
    RationalMul,
    RationalDivRem,
    RationalGcd,
    RationalDerivative,
    RationalToPrimitiveInteger,
    RationalToIntegerExact,
    RationalAddSub,
    FactorCandidates,
    FactorOutput,
    KroneckerPoints,
    KroneckerValues,
    SignedDivisors,
    DivisorColumns,
    CartesianDigits,
    InterpolationBasis,
    InterpolationOutput,
    InterpolationZeroes,
    SturmPolynomials,
    SturmPending,
    SturmResults,
    SturmObservations,
    SturmChildren,
    SturmNegatedRational,
    SturmPrimitiveInteger,
    SturmNegatedInteger,
    SylvesterMatrix,
    DeterminantPermutation,
    DeterminantSum,
    DeterminantProduct,
    ResultantOne,
    ResultantClone,
    ResultantMul,
    GeneratorPolynomial,
    FormSumPolynomial,
    IrreducibleRootOutput,
    SquareFreeRootOutput,
    ConstantYPolynomial,
    SignedSubstitution,
    MultiplicationSubstitution,
    ReciprocalPolynomial,
    RootPowerPolynomial,
    ConstantPolynomial,
    MonomialPolynomial,
}

impl fmt::Display for AllocationResource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::PolynomialCoefficients => "polynomial coefficients",
            Self::RationalCoefficients => "rational coefficients",
            Self::EvaluationPoints => "evaluation points",
            Self::Divisors => "divisors",
            Self::ProductDigits => "product digits",
            Self::FactorCandidates => "factor candidates",
            Self::Factors => "factors",
            Self::SturmSequence => "Sturm sequence",
            Self::RootIntervals => "root intervals",
            Self::SylvesterElements => "Sylvester elements",
            Self::Permutation => "permutation",
            Self::ResultantCoefficients => "resultant coefficients",
            Self::RootCandidates => "root candidates",
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum AlgnumError {
    ZeroPolynomial,
    InvalidIsolation,
    NoTargetRoot,
    MultipleTargetRoots,
    DivisionByZero,
    UndefinedZeroPower,
    ZeroToNegativePower,
    ZeroRootDegree,
    EvenRootOfNegative,
    RepresentationLimit {
        resource: RepresentationResource,
        required: BigUint,
        maximum: BigUint,
    },
    AllocationLimit {
        resource: AllocationResource,
        required: BigUint,
        maximum: BigUint,
    },
    AllocationFailure {
        resource: AllocationResource,
        requested: usize,
    },
    Bigint(BigintError),
    FormSum(FormSumErrorKind),
}

impl AlgnumError {
    pub(crate) fn coefficient_count_overflow(required: BigUint) -> Self {
        let maximum = match BigUint::try_from(usize::MAX) {
            Ok(value) => value,
            Err(error) => return error.into(),
        };
        Self::RepresentationLimit {
            resource: RepresentationResource::CoefficientCount,
            required,
            maximum,
        }
    }
}

impl fmt::Display for AlgnumError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroPolynomial => {
                formatter.write_str("candidate polynomial degree must be at least one")
            }
            Self::InvalidIsolation => {
                formatter.write_str("isolating interval endpoints must be ordered and non-roots")
            }
            Self::NoTargetRoot => formatter.write_str("isolating interval contains no target root"),
            Self::MultipleTargetRoots => {
                formatter.write_str("isolating interval contains multiple target roots")
            }
            Self::DivisionByZero => formatter.write_str("division by zero algebraic number"),
            Self::UndefinedZeroPower => formatter.write_str("zero to the zero power is undefined"),
            Self::ZeroToNegativePower => {
                formatter.write_str("zero cannot be raised to a negative power")
            }
            Self::ZeroRootDegree => formatter.write_str("root degree must be positive"),
            Self::EvenRootOfNegative => {
                formatter.write_str("even root of a negative real is undefined")
            }
            Self::RepresentationLimit {
                resource,
                required,
                maximum,
            } => {
                write!(formatter, "representation limit for {resource}: required ")?;
                fmt_biguint_decimal(required, formatter)?;
                formatter.write_str(", maximum ")?;
                fmt_biguint_decimal(maximum, formatter)
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "allocation failed for {resource}: requested {requested} elements"
            ),
            Self::AllocationLimit {
                resource,
                required,
                maximum,
            } => {
                write!(formatter, "allocation limit for {resource}: required ")?;
                fmt_biguint_decimal(required, formatter)?;
                formatter.write_str(", maximum ")?;
                fmt_biguint_decimal(maximum, formatter)?;
                formatter.write_str(" elements")
            }
            Self::Bigint(error) => error.fmt(formatter),
            Self::FormSum(error) => error.fmt(formatter),
        }
    }
}

fn fmt_biguint_decimal(value: &BigUint, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    if value.is_zero() {
        return formatter.write_str("0");
    }

    let base = BigUint::try_from(1_000_000_000_u32).map_err(|_| fmt::Error)?;
    let mut power = BigUint::one().map_err(|_| fmt::Error)?;
    loop {
        let next = power.mul(&base).map_err(|_| fmt::Error)?;
        if &next > value {
            break;
        }
        power = next;
    }
    let (leading, mut remaining) = value.div_rem(&power).map_err(|_| fmt::Error)?;
    let leading = leading.to_u32().ok_or(fmt::Error)?;
    write!(formatter, "{leading}")?;
    while power > BigUint::one().map_err(|_| fmt::Error)? {
        power = power.exact_div(&base).map_err(|_| fmt::Error)?;
        let (chunk, remainder) = remaining.div_rem(&power).map_err(|_| fmt::Error)?;
        let chunk = chunk.to_u32().ok_or(fmt::Error)?;
        write!(formatter, "{chunk:09}")?;
        remaining = remainder;
    }
    Ok(())
}

#[cfg(feature = "std")]
impl std::error::Error for AlgnumError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Bigint(error) => Some(error),
            Self::FormSum(error) => Some(error),
            _ => None,
        }
    }
}

impl From<BigintError> for AlgnumError {
    fn from(error: BigintError) -> Self {
        Self::Bigint(error)
    }
}

impl From<FormSumErrorKind> for AlgnumError {
    fn from(error: FormSumErrorKind) -> Self {
        Self::FormSum(error)
    }
}

pub(crate) fn reserve_elements_at<T>(
    values: &mut Vec<T>,
    total: usize,
    resource: AllocationResource,
    contact: AllocationContact,
) -> Result<(), AlgnumError> {
    #[cfg(not(test))]
    let _ = contact;
    if total <= values.capacity() {
        return Ok(());
    }

    #[cfg(test)]
    if injected_failure(contact) {
        return Err(allocation_failure(resource, total));
    }

    let additional = total - values.len();
    values
        .try_reserve(additional)
        .map_err(|_| allocation_failure(resource, total))
}

pub(crate) fn allocation_total_to_usize(
    required: &BigUint,
    resource: AllocationResource,
) -> Result<usize, AlgnumError> {
    let maximum = BigUint::try_from(usize::MAX)?;
    if required > &maximum {
        return Err(AlgnumError::AllocationLimit {
            resource,
            required: required.try_clone()?,
            maximum,
        });
    }
    let mut value = 0_usize;
    for bit in 0..required.bit_len() {
        if required.bit(bit) {
            value |= 1_usize << bit;
        }
    }
    Ok(value)
}

fn allocation_failure(resource: AllocationResource, requested: usize) -> AlgnumError {
    AlgnumError::AllocationFailure {
        resource,
        requested,
    }
}

#[cfg(test)]
std::thread_local! {
    static INJECTED_CONTACT: core::cell::Cell<Option<AllocationContact>> = const { core::cell::Cell::new(None) };
}

#[cfg(test)]
fn injected_failure(contact: AllocationContact) -> bool {
    INJECTED_CONTACT.with(|configured| configured.get() == Some(contact))
}

#[cfg(test)]
pub(crate) fn with_injected_failure<R>(
    contact: AllocationContact,
    operation: impl FnOnce() -> R,
) -> R {
    INJECTED_CONTACT.with(|configured| configured.set(Some(contact)));
    let result = operation();
    INJECTED_CONTACT.with(|configured| configured.set(None));
    result
}

#[cfg(test)]
mod allocation_tests {
    use alloc::string::ToString;
    use alloc::vec;
    use alloc::vec::Vec;

    use neco_bigint::{BigInt, BigUint, BigintError, ReducedRational};
    use neco_formsum::{DimensionResource, FormSum, FormSumErrorKind};
    use neco_monomial::MonomialErrorKind;

    use super::{
        allocation_total_to_usize, reserve_elements_at, with_injected_failure, AlgnumError,
        AllocationContact, AllocationResource, RepresentationResource,
    };
    use crate::{Polynomial, RationalPolynomial};

    const ALLOCATION_RESOURCES: [AllocationResource; 13] = [
        AllocationResource::PolynomialCoefficients,
        AllocationResource::RationalCoefficients,
        AllocationResource::EvaluationPoints,
        AllocationResource::Divisors,
        AllocationResource::ProductDigits,
        AllocationResource::FactorCandidates,
        AllocationResource::Factors,
        AllocationResource::SturmSequence,
        AllocationResource::RootIntervals,
        AllocationResource::SylvesterElements,
        AllocationResource::Permutation,
        AllocationResource::ResultantCoefficients,
        AllocationResource::RootCandidates,
    ];

    const ALLOCATION_CONTACTS: [(AllocationContact, AllocationResource); 52] = [
        (
            AllocationContact::PolynomialOne,
            AllocationResource::PolynomialCoefficients,
        ),
        (
            AllocationContact::PolynomialClone,
            AllocationResource::PolynomialCoefficients,
        ),
        (
            AllocationContact::PolynomialMul,
            AllocationResource::PolynomialCoefficients,
        ),
        (
            AllocationContact::PolynomialDerivative,
            AllocationResource::PolynomialCoefficients,
        ),
        (
            AllocationContact::PolynomialPrimitivePart,
            AllocationResource::PolynomialCoefficients,
        ),
        (
            AllocationContact::PolynomialToRational,
            AllocationResource::RationalCoefficients,
        ),
        (
            AllocationContact::PolynomialAddSub,
            AllocationResource::PolynomialCoefficients,
        ),
        (
            AllocationContact::PolynomialSingleCoefficient,
            AllocationResource::PolynomialCoefficients,
        ),
        (
            AllocationContact::RationalClone,
            AllocationResource::RationalCoefficients,
        ),
        (
            AllocationContact::RationalMul,
            AllocationResource::RationalCoefficients,
        ),
        (
            AllocationContact::RationalDivRem,
            AllocationResource::RationalCoefficients,
        ),
        (
            AllocationContact::RationalGcd,
            AllocationResource::RationalCoefficients,
        ),
        (
            AllocationContact::RationalDerivative,
            AllocationResource::RationalCoefficients,
        ),
        (
            AllocationContact::RationalToPrimitiveInteger,
            AllocationResource::PolynomialCoefficients,
        ),
        (
            AllocationContact::RationalToIntegerExact,
            AllocationResource::PolynomialCoefficients,
        ),
        (
            AllocationContact::RationalAddSub,
            AllocationResource::RationalCoefficients,
        ),
        (
            AllocationContact::FactorCandidates,
            AllocationResource::FactorCandidates,
        ),
        (AllocationContact::FactorOutput, AllocationResource::Factors),
        (
            AllocationContact::KroneckerPoints,
            AllocationResource::EvaluationPoints,
        ),
        (
            AllocationContact::KroneckerValues,
            AllocationResource::EvaluationPoints,
        ),
        (
            AllocationContact::SignedDivisors,
            AllocationResource::Divisors,
        ),
        (
            AllocationContact::DivisorColumns,
            AllocationResource::Divisors,
        ),
        (
            AllocationContact::CartesianDigits,
            AllocationResource::ProductDigits,
        ),
        (
            AllocationContact::InterpolationBasis,
            AllocationResource::RationalCoefficients,
        ),
        (
            AllocationContact::InterpolationOutput,
            AllocationResource::RationalCoefficients,
        ),
        (
            AllocationContact::InterpolationZeroes,
            AllocationResource::RationalCoefficients,
        ),
        (
            AllocationContact::SturmPolynomials,
            AllocationResource::SturmSequence,
        ),
        (
            AllocationContact::SturmPending,
            AllocationResource::RootIntervals,
        ),
        (
            AllocationContact::SturmResults,
            AllocationResource::RootIntervals,
        ),
        (
            AllocationContact::SturmObservations,
            AllocationResource::RootIntervals,
        ),
        (
            AllocationContact::SturmChildren,
            AllocationResource::RootIntervals,
        ),
        (
            AllocationContact::SturmNegatedRational,
            AllocationResource::RationalCoefficients,
        ),
        (
            AllocationContact::SturmPrimitiveInteger,
            AllocationResource::PolynomialCoefficients,
        ),
        (
            AllocationContact::SturmNegatedInteger,
            AllocationResource::PolynomialCoefficients,
        ),
        (
            AllocationContact::SylvesterMatrix,
            AllocationResource::SylvesterElements,
        ),
        (
            AllocationContact::DeterminantPermutation,
            AllocationResource::Permutation,
        ),
        (
            AllocationContact::DeterminantSum,
            AllocationResource::ResultantCoefficients,
        ),
        (
            AllocationContact::DeterminantProduct,
            AllocationResource::ResultantCoefficients,
        ),
        (
            AllocationContact::ResultantOne,
            AllocationResource::ResultantCoefficients,
        ),
        (
            AllocationContact::ResultantClone,
            AllocationResource::ResultantCoefficients,
        ),
        (
            AllocationContact::ResultantMul,
            AllocationResource::ResultantCoefficients,
        ),
        (
            AllocationContact::GeneratorPolynomial,
            AllocationResource::RationalCoefficients,
        ),
        (
            AllocationContact::FormSumPolynomial,
            AllocationResource::PolynomialCoefficients,
        ),
        (
            AllocationContact::IrreducibleRootOutput,
            AllocationResource::RootCandidates,
        ),
        (
            AllocationContact::SquareFreeRootOutput,
            AllocationResource::RootCandidates,
        ),
        (
            AllocationContact::ConstantYPolynomial,
            AllocationResource::ResultantCoefficients,
        ),
        (
            AllocationContact::SignedSubstitution,
            AllocationResource::ResultantCoefficients,
        ),
        (
            AllocationContact::MultiplicationSubstitution,
            AllocationResource::ResultantCoefficients,
        ),
        (
            AllocationContact::ReciprocalPolynomial,
            AllocationResource::PolynomialCoefficients,
        ),
        (
            AllocationContact::RootPowerPolynomial,
            AllocationResource::PolynomialCoefficients,
        ),
        (
            AllocationContact::ConstantPolynomial,
            AllocationResource::ResultantCoefficients,
        ),
        (
            AllocationContact::MonomialPolynomial,
            AllocationResource::ResultantCoefficients,
        ),
    ];

    #[test]
    fn resource_display_is_complete_and_stable() {
        let representation = [
            (RepresentationResource::RootDegree, "root degree"),
            (
                RepresentationResource::PolynomialDegree,
                "polynomial degree",
            ),
            (
                RepresentationResource::CoefficientCount,
                "coefficient count",
            ),
            (
                RepresentationResource::SylvesterDimension,
                "Sylvester dimension",
            ),
            (
                RepresentationResource::SylvesterElementCount,
                "Sylvester element count",
            ),
        ];
        for (resource, expected) in representation {
            assert_eq!(resource.to_string(), expected);
        }

        let allocation = [
            "polynomial coefficients",
            "rational coefficients",
            "evaluation points",
            "divisors",
            "product digits",
            "factor candidates",
            "factors",
            "Sturm sequence",
            "root intervals",
            "Sylvester elements",
            "permutation",
            "resultant coefficients",
            "root candidates",
        ];
        for (resource, expected) in ALLOCATION_RESOURCES.into_iter().zip(allocation) {
            assert_eq!(resource.to_string(), expected);
        }
    }

    #[test]
    fn lower_error_conversions_preserve_variants_and_payloads() {
        let bigint_errors = [
            BigintError::CapacityOverflow,
            BigintError::AllocationFailure {
                requested_limbs: 47,
            },
            BigintError::UnsignedUnderflow,
            BigintError::DivisionByZero,
            BigintError::NonExactDivision,
            BigintError::ZeroDenominator,
            BigintError::NonFiniteFloat,
            BigintError::FloatOutOfRange,
            BigintError::InvalidInterval,
            BigintError::ExponentOverflow {
                required: BigUint::try_from(4_294_967_343_u64).unwrap(),
                maximum: u32::MAX,
            },
        ];
        for error in bigint_errors {
            let expected = match &error {
                BigintError::CapacityOverflow => BigintError::CapacityOverflow,
                BigintError::AllocationFailure { requested_limbs } => {
                    BigintError::AllocationFailure {
                        requested_limbs: *requested_limbs,
                    }
                }
                BigintError::UnsignedUnderflow => BigintError::UnsignedUnderflow,
                BigintError::DivisionByZero => BigintError::DivisionByZero,
                BigintError::NonExactDivision => BigintError::NonExactDivision,
                BigintError::ZeroDenominator => BigintError::ZeroDenominator,
                BigintError::NonFiniteFloat => BigintError::NonFiniteFloat,
                BigintError::FloatOutOfRange => BigintError::FloatOutOfRange,
                BigintError::InvalidInterval => BigintError::InvalidInterval,
                BigintError::ExponentOverflow { required, maximum } => {
                    BigintError::ExponentOverflow {
                        required: required.try_clone().unwrap(),
                        maximum: *maximum,
                    }
                }
            };
            assert_eq!(AlgnumError::from(error), AlgnumError::Bigint(expected));
        }

        let form_sum_errors = [
            FormSumErrorKind::DivisionByZero,
            FormSumErrorKind::DimensionOverflow {
                resource: DimensionResource::MatrixElementCount,
                required: BigUint::try_from(53_u32).unwrap(),
                maximum: BigUint::try_from(43_u32).unwrap(),
            },
            FormSumErrorKind::AllocationFailure {
                resource: DimensionResource::BasisCount,
                requested: 59,
            },
            FormSumErrorKind::Bigint(BigintError::AllocationFailure {
                requested_limbs: 61,
            }),
            FormSumErrorKind::Monomial(MonomialErrorKind::AllocationFailure {
                requested_elements: 67,
            }),
            FormSumErrorKind::Monomial(MonomialErrorKind::Bigint(BigintError::AllocationFailure {
                requested_limbs: 71,
            })),
        ];
        for error in form_sum_errors {
            let expected = match &error {
                FormSumErrorKind::DivisionByZero => FormSumErrorKind::DivisionByZero,
                FormSumErrorKind::DimensionOverflow {
                    resource,
                    required,
                    maximum,
                } => FormSumErrorKind::DimensionOverflow {
                    resource: *resource,
                    required: required.try_clone().unwrap(),
                    maximum: maximum.try_clone().unwrap(),
                },
                FormSumErrorKind::AllocationFailure {
                    resource,
                    requested,
                } => FormSumErrorKind::AllocationFailure {
                    resource: *resource,
                    requested: *requested,
                },
                FormSumErrorKind::Bigint(BigintError::AllocationFailure { requested_limbs }) => {
                    FormSumErrorKind::Bigint(BigintError::AllocationFailure {
                        requested_limbs: *requested_limbs,
                    })
                }
                FormSumErrorKind::Monomial(MonomialErrorKind::AllocationFailure {
                    requested_elements,
                }) => FormSumErrorKind::Monomial(MonomialErrorKind::AllocationFailure {
                    requested_elements: *requested_elements,
                }),
                FormSumErrorKind::Monomial(MonomialErrorKind::Bigint(
                    BigintError::AllocationFailure { requested_limbs },
                )) => FormSumErrorKind::Monomial(MonomialErrorKind::Bigint(
                    BigintError::AllocationFailure {
                        requested_limbs: *requested_limbs,
                    },
                )),
                _ => unreachable!(),
            };
            assert_eq!(AlgnumError::from(error), AlgnumError::FormSum(expected));
        }
    }

    #[test]
    fn every_allocation_contact_reports_the_total_requested() {
        for (contact, resource) in ALLOCATION_CONTACTS {
            let mut values = Vec::<u8>::new();
            let result = with_injected_failure(contact, || {
                reserve_elements_at(&mut values, 7, resource, contact)
            });
            assert_eq!(
                result,
                Err(AlgnumError::AllocationFailure {
                    resource,
                    requested: 7,
                })
            );
        }
    }

    fn integer(value: i32) -> BigInt {
        BigInt::try_from(value).unwrap()
    }

    fn polynomial(values: &[i32]) -> Polynomial {
        Polynomial::from_coefficients(values.iter().copied().map(integer).collect())
    }

    fn assert_allocation<T>(
        result: Result<T, AlgnumError>,
        resource: AllocationResource,
        requested: usize,
    ) {
        match result {
            Err(AlgnumError::AllocationFailure {
                resource: actual_resource,
                requested: actual_requested,
            }) => {
                assert_eq!(actual_resource, resource);
                assert_eq!(actual_requested, requested);
            }
            Err(error) => panic!("unexpected failure: {error}"),
            Ok(_) => panic!("injected allocation failure was not observed"),
        }
    }

    #[test]
    fn every_allocation_contact_is_observed_through_its_production_operation() {
        let integer_polynomial = polynomial(&[1, 1, 1]);
        let rational_polynomial = RationalPolynomial::from_coefficients(vec![
            ReducedRational::from_bigint(integer(1)).unwrap(),
            ReducedRational::from_bigint(integer(1)).unwrap(),
        ]);
        for (contact, resource, requested, operation) in [
            (
                AllocationContact::PolynomialOne,
                AllocationResource::PolynomialCoefficients,
                1,
                0,
            ),
            (
                AllocationContact::PolynomialClone,
                AllocationResource::PolynomialCoefficients,
                3,
                1,
            ),
            (
                AllocationContact::PolynomialMul,
                AllocationResource::PolynomialCoefficients,
                5,
                2,
            ),
            (
                AllocationContact::PolynomialDerivative,
                AllocationResource::PolynomialCoefficients,
                2,
                3,
            ),
            (
                AllocationContact::PolynomialPrimitivePart,
                AllocationResource::PolynomialCoefficients,
                3,
                4,
            ),
            (
                AllocationContact::PolynomialToRational,
                AllocationResource::RationalCoefficients,
                3,
                4,
            ),
            (
                AllocationContact::PolynomialAddSub,
                AllocationResource::PolynomialCoefficients,
                3,
                5,
            ),
            (
                AllocationContact::PolynomialSingleCoefficient,
                AllocationResource::PolynomialCoefficients,
                1,
                6,
            ),
            (
                AllocationContact::RationalClone,
                AllocationResource::RationalCoefficients,
                2,
                7,
            ),
            (
                AllocationContact::RationalMul,
                AllocationResource::RationalCoefficients,
                3,
                8,
            ),
            (
                AllocationContact::RationalDivRem,
                AllocationResource::RationalCoefficients,
                1,
                9,
            ),
            (
                AllocationContact::RationalGcd,
                AllocationResource::RationalCoefficients,
                2,
                10,
            ),
            (
                AllocationContact::RationalAddSub,
                AllocationResource::RationalCoefficients,
                2,
                11,
            ),
        ] {
            let result = with_injected_failure(contact, || match operation {
                0 => Polynomial::one().map(|_| ()),
                1 => integer_polynomial.try_clone().map(|_| ()),
                2 => integer_polynomial.mul(&integer_polynomial).map(|_| ()),
                3 => integer_polynomial.derivative().map(|_| ()),
                4 => integer_polynomial
                    .try_clone()
                    .unwrap()
                    .candidate()?
                    .square_free()
                    .map(|_| ()),
                5 => integer_polynomial.add(&integer_polynomial).map(|_| ()),
                6 => integer_polynomial.compose(&polynomial(&[1, 1])).map(|_| ()),
                7 => rational_polynomial.try_clone().map(|_| ()),
                8 => rational_polynomial.mul(&rational_polynomial).map(|_| ()),
                9 => rational_polynomial
                    .div_rem(&rational_polynomial)
                    .map(|_| ()),
                10 => rational_polynomial.gcd(&rational_polynomial).map(|_| ()),
                11 => rational_polynomial.add(&rational_polynomial).map(|_| ()),
                _ => unreachable!(),
            });
            assert_allocation(result, resource, requested);
        }

        let reducible = polynomial(&[-1, 0, 1])
            .candidate()
            .unwrap()
            .square_free()
            .unwrap();
        for (contact, resource, requested) in [
            (
                AllocationContact::RationalDerivative,
                AllocationResource::RationalCoefficients,
                2,
            ),
            (
                AllocationContact::RationalToPrimitiveInteger,
                AllocationResource::PolynomialCoefficients,
                3,
            ),
            (
                AllocationContact::RationalToIntegerExact,
                AllocationResource::PolynomialCoefficients,
                2,
            ),
            (
                AllocationContact::FactorCandidates,
                AllocationResource::FactorCandidates,
                2,
            ),
            (
                AllocationContact::FactorOutput,
                AllocationResource::Factors,
                2,
            ),
            (
                AllocationContact::KroneckerPoints,
                AllocationResource::EvaluationPoints,
                2,
            ),
            (
                AllocationContact::KroneckerValues,
                AllocationResource::EvaluationPoints,
                2,
            ),
            (
                AllocationContact::SignedDivisors,
                AllocationResource::Divisors,
                2,
            ),
            (
                AllocationContact::DivisorColumns,
                AllocationResource::Divisors,
                2,
            ),
            (
                AllocationContact::CartesianDigits,
                AllocationResource::ProductDigits,
                2,
            ),
            (
                AllocationContact::InterpolationBasis,
                AllocationResource::RationalCoefficients,
                2,
            ),
            (
                AllocationContact::InterpolationOutput,
                AllocationResource::RationalCoefficients,
                2,
            ),
            (
                AllocationContact::InterpolationZeroes,
                AllocationResource::RationalCoefficients,
                2,
            ),
        ] {
            let result = with_injected_failure(contact, || {
                if matches!(
                    contact,
                    AllocationContact::RationalDerivative
                        | AllocationContact::RationalToPrimitiveInteger
                ) {
                    polynomial(&[-1, 0, 1])
                        .candidate()?
                        .square_free()
                        .map(|_| ())
                } else {
                    reducible.factor().map(|_| ())
                }
            });
            assert_allocation(result, resource, requested);
        }

        let irreducible_polynomial = polynomial(&[-2, 0, 1]);
        let sequence = crate::sturm::SturmSequence::new(&irreducible_polynomial).unwrap();
        for (contact, resource, requested) in [
            (
                AllocationContact::SturmPolynomials,
                AllocationResource::SturmSequence,
                3,
            ),
            (
                AllocationContact::SturmPending,
                AllocationResource::RootIntervals,
                2,
            ),
            (
                AllocationContact::SturmResults,
                AllocationResource::RootIntervals,
                2,
            ),
            (
                AllocationContact::SturmObservations,
                AllocationResource::RootIntervals,
                2,
            ),
            (
                AllocationContact::SturmChildren,
                AllocationResource::RootIntervals,
                2,
            ),
            (
                AllocationContact::SturmNegatedRational,
                AllocationResource::RationalCoefficients,
                1,
            ),
            (
                AllocationContact::SturmPrimitiveInteger,
                AllocationResource::PolynomialCoefficients,
                2,
            ),
            (
                AllocationContact::SturmNegatedInteger,
                AllocationResource::PolynomialCoefficients,
                1,
            ),
        ] {
            let result = with_injected_failure(contact, || {
                if contact == AllocationContact::SturmNegatedInteger {
                    crate::sturm::SturmSequence::new(&polynomial(&[1, 1, 1])).map(|_| ())
                } else if contact == AllocationContact::SturmPolynomials
                    || matches!(
                        contact,
                        AllocationContact::SturmNegatedRational
                            | AllocationContact::SturmPrimitiveInteger
                    )
                {
                    crate::sturm::SturmSequence::new(&irreducible_polynomial).map(|_| ())
                } else {
                    sequence.isolate_real_roots().map(|_| ())
                }
            });
            assert_allocation(result, resource, requested);
        }

        let left = vec![polynomial(&[-2]), Polynomial::zero(), polynomial(&[1])];
        let right = vec![polynomial(&[-1]), polynomial(&[1])];
        for (contact, resource, requested) in [
            (
                AllocationContact::SylvesterMatrix,
                AllocationResource::SylvesterElements,
                9,
            ),
            (
                AllocationContact::DeterminantPermutation,
                AllocationResource::Permutation,
                3,
            ),
            (
                AllocationContact::DeterminantProduct,
                AllocationResource::ResultantCoefficients,
                1,
            ),
            (
                AllocationContact::DeterminantSum,
                AllocationResource::ResultantCoefficients,
                1,
            ),
        ] {
            assert_allocation(
                with_injected_failure(contact, || crate::resultant::resultant(&left, &right)),
                resource,
                requested,
            );
        }
        let constant = [polynomial(&[2])];
        let linear = [polynomial(&[-1]), polynomial(&[1])];
        for contact in [
            AllocationContact::ResultantOne,
            AllocationContact::ResultantClone,
            AllocationContact::ResultantMul,
        ] {
            assert_allocation(
                with_injected_failure(contact, || crate::resultant::resultant(&constant, &linear)),
                AllocationResource::ResultantCoefficients,
                1,
            );
        }

        let factor = irreducible_polynomial
            .candidate()
            .unwrap()
            .square_free()
            .unwrap()
            .factor()
            .unwrap()
            .pop()
            .unwrap();
        let roots = factor.isolate_real_roots().unwrap();
        let positive = roots[1].value();
        let square_free = polynomial(&[-1, 0, 1])
            .candidate()
            .unwrap()
            .square_free()
            .unwrap();
        let one = FormSum::one().unwrap();
        for (contact, resource, requested, operation) in [
            (
                AllocationContact::GeneratorPolynomial,
                AllocationResource::RationalCoefficients,
                2,
                0,
            ),
            (
                AllocationContact::FormSumPolynomial,
                AllocationResource::PolynomialCoefficients,
                2,
                1,
            ),
            (
                AllocationContact::IrreducibleRootOutput,
                AllocationResource::RootCandidates,
                2,
                2,
            ),
            (
                AllocationContact::SquareFreeRootOutput,
                AllocationResource::RootCandidates,
                2,
                3,
            ),
            (
                AllocationContact::ConstantYPolynomial,
                AllocationResource::ResultantCoefficients,
                3,
                4,
            ),
            (
                AllocationContact::SignedSubstitution,
                AllocationResource::ResultantCoefficients,
                3,
                4,
            ),
            (
                AllocationContact::MultiplicationSubstitution,
                AllocationResource::ResultantCoefficients,
                3,
                5,
            ),
            (
                AllocationContact::ReciprocalPolynomial,
                AllocationResource::PolynomialCoefficients,
                3,
                6,
            ),
            (
                AllocationContact::RootPowerPolynomial,
                AllocationResource::PolynomialCoefficients,
                5,
                7,
            ),
            (
                AllocationContact::ConstantPolynomial,
                AllocationResource::ResultantCoefficients,
                1,
                4,
            ),
            (
                AllocationContact::MonomialPolynomial,
                AllocationResource::ResultantCoefficients,
                1,
                4,
            ),
        ] {
            let result = with_injected_failure(contact, || match operation {
                0 => positive
                    .minimal_polynomial()
                    .quotient()?
                    .generator()?
                    .as_polynomial()
                    .map(|_| ()),
                1 => crate::RealAlgebraic::from_form_sum(&one).map(|_| ()),
                2 => factor.isolate_real_roots().map(|_| ()),
                3 => square_free.isolate_real_roots().map(|_| ()),
                4 => positive.add(positive).map(|_| ()),
                5 => positive.mul(positive).map(|_| ()),
                6 => positive.div(positive).map(|_| ()),
                7 => positive.nth_root(2).map(|_| ()),
                _ => unreachable!(),
            });
            assert_allocation(result, resource, requested);
        }
    }

    #[test]
    fn error_display_includes_exact_payloads() {
        let required = neco_bigint::BigUint::try_from(12_345_678_901_u64).unwrap();
        let maximum = neco_bigint::BigUint::try_from(4_294_967_295_u64).unwrap();
        let representation = AlgnumError::RepresentationLimit {
            resource: RepresentationResource::RootDegree,
            required,
            maximum,
        };
        assert_eq!(
            representation.to_string(),
            "representation limit for root degree: required 12345678901, maximum 4294967295"
        );

        let internal_zero_chunks = AlgnumError::RepresentationLimit {
            resource: RepresentationResource::RootDegree,
            required: neco_bigint::BigUint::try_from(1_000_000_000_000_000_001_u64).unwrap(),
            maximum: neco_bigint::BigUint::try_from(1_u32).unwrap(),
        };
        assert_eq!(
            internal_zero_chunks.to_string(),
            "representation limit for root degree: required 1000000000000000001, maximum 1"
        );

        let allocation = AlgnumError::AllocationFailure {
            resource: AllocationResource::RootIntervals,
            requested: 17,
        };
        assert_eq!(
            allocation.to_string(),
            "allocation failed for root intervals: requested 17 elements"
        );

        let allocation_limit = AlgnumError::AllocationLimit {
            resource: AllocationResource::FactorCandidates,
            required: neco_bigint::BigUint::try_from(19_u32).unwrap(),
            maximum: neco_bigint::BigUint::try_from(17_u32).unwrap(),
        };
        assert_eq!(
            allocation_limit.to_string(),
            "allocation limit for factor candidates: required 19, maximum 17 elements"
        );
    }

    #[test]
    fn allocation_limit_preserves_the_exact_total_above_usize() {
        let maximum = neco_bigint::BigUint::try_from(usize::MAX).unwrap();
        let required = maximum.add(&neco_bigint::BigUint::one().unwrap()).unwrap();
        assert_eq!(
            allocation_total_to_usize(&required, AllocationResource::FactorCandidates),
            Err(AlgnumError::AllocationLimit {
                resource: AllocationResource::FactorCandidates,
                required,
                maximum,
            })
        );
    }
}
