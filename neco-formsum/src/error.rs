use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;
use core::mem::size_of;

use neco_bigint::{BigUint, BigintError};
use neco_monomial::MonomialErrorKind;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DimensionResource {
    Denominator,
    BasisCount,
    MatrixElementCount,
}

#[derive(Debug, Eq, PartialEq)]
pub enum FormSumErrorKind {
    DivisionByZero,
    DimensionOverflow {
        resource: DimensionResource,
        required: BigUint,
        maximum: BigUint,
    },
    AllocationFailure {
        resource: DimensionResource,
        requested: usize,
    },
    Bigint(BigintError),
    Monomial(MonomialErrorKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AllocationTarget {
    RawTermClone,
    NormalizationIndex,
    ErrorSet,
    NormalTerms,
    MergeResult,
    ProductResult,
    ProductFactors,
    ExtensionPrimes,
    ExtensionDenominators,
    ExtensionFactors,
    CoordinateValues,
    CoordinateTerms,
    CoordinateFactors,
    MultiplicationMatrix,
    GaussianMatrix,
    GaussianRightHandSide,
    AnnihilatingCoefficients,
    RecurrenceCoefficients,
    AnnihilatingInputMatrix,
    RecurrenceStateMatrix,
    RecurrenceProductMatrix,
    IntegerCoefficients,
}

impl Ord for FormSumErrorKind {
    fn cmp(&self, other: &Self) -> Ordering {
        error_discriminant(self)
            .cmp(&error_discriminant(other))
            .then_with(|| match (self, other) {
                (
                    Self::DimensionOverflow {
                        resource: lr,
                        required: lq,
                        maximum: lm,
                    },
                    Self::DimensionOverflow {
                        resource: rr,
                        required: rq,
                        maximum: rm,
                    },
                ) => lr.cmp(rr).then_with(|| lq.cmp(rq)).then_with(|| lm.cmp(rm)),
                (
                    Self::AllocationFailure {
                        resource: lr,
                        requested: lq,
                    },
                    Self::AllocationFailure {
                        resource: rr,
                        requested: rq,
                    },
                ) => lr.cmp(rr).then_with(|| lq.cmp(rq)),
                (Self::Bigint(left), Self::Bigint(right)) => compare_bigint(left, right),
                (Self::Monomial(left), Self::Monomial(right)) => left.cmp(right),
                _ => Ordering::Equal,
            })
    }
}

impl PartialOrd for FormSumErrorKind {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for FormSumErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DivisionByZero => formatter.write_str("division by zero"),
            Self::DimensionOverflow { .. } => {
                formatter.write_str("form-sum dimension exceeds usize")
            }
            Self::AllocationFailure { .. } => formatter.write_str("form-sum allocation failed"),
            Self::Bigint(error) => error.fmt(formatter),
            Self::Monomial(error) => error.fmt(formatter),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FormSumErrorKind {}

impl From<BigintError> for FormSumErrorKind {
    fn from(error: BigintError) -> Self {
        Self::Bigint(error)
    }
}

impl From<MonomialErrorKind> for FormSumErrorKind {
    fn from(error: MonomialErrorKind) -> Self {
        Self::Monomial(error)
    }
}

pub(crate) fn reserve_elements<T>(
    values: &mut Vec<T>,
    total: usize,
    resource: DimensionResource,
    target: AllocationTarget,
) -> Result<(), FormSumErrorKind> {
    #[cfg(not(test))]
    let _ = target;
    let maximum = if size_of::<T>() == 0 {
        usize::MAX
    } else {
        (isize::MAX as usize) / size_of::<T>()
    };
    #[cfg(test)]
    if let Some(failure) = injected_failure(target, resource, total, maximum) {
        return Err(failure);
    }
    if total.checked_mul(size_of::<T>()).is_none() {
        return Err(overflow(resource, total, maximum)?);
    }
    if total > maximum {
        return Err(overflow(resource, total, maximum)?);
    }
    let additional = total
        .checked_sub(values.len())
        .ok_or_else(|| allocation_failure(resource, total))?;
    values
        .try_reserve(additional)
        .map_err(|_| allocation_failure(resource, total))
}

fn allocation_failure(resource: DimensionResource, requested: usize) -> FormSumErrorKind {
    FormSumErrorKind::AllocationFailure {
        resource,
        requested,
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InjectedFailure {
    Capacity,
    Allocation,
}

#[cfg(test)]
std::thread_local! {
    static INJECTED_TARGET: core::cell::Cell<Option<(AllocationTarget, InjectedFailure)>> = const { core::cell::Cell::new(None) };
    static OBSERVED_RESERVATION: core::cell::RefCell<Option<(DimensionResource, usize, usize)>> = const { core::cell::RefCell::new(None) };
}

#[cfg(test)]
fn injected_failure(
    target: AllocationTarget,
    resource: DimensionResource,
    total: usize,
    maximum: usize,
) -> Option<FormSumErrorKind> {
    INJECTED_TARGET.with(|configured| match configured.get() {
        Some((expected, failure)) if expected == target => {
            OBSERVED_RESERVATION
                .with(|observed| *observed.borrow_mut() = Some((resource, total, maximum)));
            Some(match failure {
                InjectedFailure::Capacity => {
                    let synthetic_maximum = total.saturating_sub(1);
                    overflow(resource, total, synthetic_maximum)
                        .unwrap_or_else(|_| allocation_failure(resource, total))
                }
                InjectedFailure::Allocation => allocation_failure(resource, total),
            })
        }
        _ => None,
    })
}

#[cfg(test)]
fn with_injected_failure<R>(
    target: AllocationTarget,
    failure: InjectedFailure,
    operation: impl FnOnce() -> R,
) -> (R, Option<(DimensionResource, usize, usize)>) {
    INJECTED_TARGET.with(|configured| configured.set(Some((target, failure))));
    OBSERVED_RESERVATION.with(|observed| *observed.borrow_mut() = None);
    let result = operation();
    let observed = OBSERVED_RESERVATION.with(|value| *value.borrow());
    INJECTED_TARGET.with(|configured| configured.set(None));
    (result, observed)
}

fn overflow(
    resource: DimensionResource,
    required: usize,
    maximum: usize,
) -> Result<FormSumErrorKind, FormSumErrorKind> {
    let required = BigUint::try_from(required).map_err(FormSumErrorKind::Bigint)?;
    let maximum = BigUint::try_from(maximum).map_err(FormSumErrorKind::Bigint)?;
    Ok(FormSumErrorKind::DimensionOverflow {
        resource,
        required,
        maximum,
    })
}

pub(crate) fn checked_square_dimension(dimension: usize) -> Result<usize, FormSumErrorKind> {
    let exact_dimension = BigUint::try_from(dimension)?;
    let required = exact_dimension.mul(&exact_dimension)?;
    match dimension.checked_mul(dimension) {
        Some(elements) => Ok(elements),
        None => Err(dimension_overflow_exact(
            DimensionResource::MatrixElementCount,
            required,
        )?),
    }
}

fn dimension_overflow_exact(
    resource: DimensionResource,
    required: BigUint,
) -> Result<FormSumErrorKind, FormSumErrorKind> {
    Ok(FormSumErrorKind::DimensionOverflow {
        resource,
        required,
        maximum: BigUint::try_from(usize::MAX)?,
    })
}

fn error_discriminant(error: &FormSumErrorKind) -> u8 {
    match error {
        FormSumErrorKind::DivisionByZero => 0,
        FormSumErrorKind::DimensionOverflow { .. } => 1,
        FormSumErrorKind::AllocationFailure { .. } => 2,
        FormSumErrorKind::Bigint(_) => 3,
        FormSumErrorKind::Monomial(_) => 4,
    }
}

fn compare_bigint(left: &BigintError, right: &BigintError) -> Ordering {
    bigint_discriminant(left)
        .cmp(&bigint_discriminant(right))
        .then_with(|| match (left, right) {
            (
                BigintError::AllocationFailure {
                    requested_limbs: left,
                },
                BigintError::AllocationFailure {
                    requested_limbs: right,
                },
            ) => left.cmp(right),
            (
                BigintError::ExponentOverflow {
                    required: lr,
                    maximum: lm,
                },
                BigintError::ExponentOverflow {
                    required: rr,
                    maximum: rm,
                },
            ) => lr.cmp(rr).then_with(|| lm.cmp(rm)),
            _ => Ordering::Equal,
        })
}

fn bigint_discriminant(error: &BigintError) -> u8 {
    match error {
        BigintError::CapacityOverflow => 0,
        BigintError::AllocationFailure { .. } => 1,
        BigintError::UnsignedUnderflow => 2,
        BigintError::DivisionByZero => 3,
        BigintError::NonExactDivision => 4,
        BigintError::ZeroDenominator => 5,
        BigintError::NonFiniteFloat => 6,
        BigintError::FloatOutOfRange => 7,
        BigintError::InvalidInterval => 8,
        BigintError::ExponentOverflow { .. } => 9,
    }
}

#[cfg(test)]
mod storage_tests {
    use alloc::vec;

    use neco_bigint::{BigInt, BigUint, RawRational};
    use neco_monomial::{RawMonomial, RawPower};

    use super::{
        checked_square_dimension, with_injected_failure, AllocationTarget, DimensionResource,
        FormSumErrorKind, InjectedFailure,
    };
    use crate::{FormSum, RawFormSum, RawTerm};

    fn rational(value: i32, denominator: u8) -> RawRational {
        RawRational::new(
            BigInt::try_from(value).unwrap(),
            BigUint::try_from(denominator).unwrap(),
        )
    }

    fn integer(value: i32) -> RawTerm {
        RawTerm::new(rational(value, 1), RawMonomial::positive(vec![]))
    }

    fn radical(base: u8) -> RawTerm {
        RawTerm::new(
            rational(1, 1),
            RawMonomial::positive(vec![RawPower::new(
                BigUint::try_from(base).unwrap(),
                rational(1, 2),
            )]),
        )
    }

    fn form(terms: alloc::vec::Vec<RawTerm>) -> FormSum {
        RawFormSum::new(terms).normalize().unwrap()
    }

    fn injected_error(result: Result<(), FormSumErrorKind>) -> FormSumErrorKind {
        result.unwrap_err()
    }

    fn assert_path(
        target: AllocationTarget,
        resource: DimensionResource,
        total: usize,
        operation: impl Fn() -> Result<(), FormSumErrorKind>,
    ) {
        for failure in [InjectedFailure::Capacity, InjectedFailure::Allocation] {
            let (result, observed) = with_injected_failure(target, failure, &operation);
            let error = injected_error(result);
            assert_eq!(
                observed.map(|value| (value.0, value.1)),
                Some((resource, total))
            );
            match failure {
                InjectedFailure::Capacity => assert_eq!(
                    error,
                    FormSumErrorKind::DimensionOverflow {
                        resource,
                        required: BigUint::try_from(total).unwrap(),
                        maximum: BigUint::try_from(total - 1).unwrap(),
                    }
                ),
                InjectedFailure::Allocation => assert_eq!(
                    error,
                    FormSumErrorKind::AllocationFailure {
                        resource,
                        requested: total,
                    }
                ),
            }
        }
    }

    #[test]
    fn every_storage_path_reports_its_resource_and_total() {
        let raw_two = || RawFormSum::new(vec![integer(1), radical(2)]);
        assert_path(
            AllocationTarget::RawTermClone,
            DimensionResource::BasisCount,
            2,
            || raw_two().try_clone().map(|_| ()),
        );
        assert_path(
            AllocationTarget::NormalizationIndex,
            DimensionResource::BasisCount,
            2,
            || {
                raw_two()
                    .normalize()
                    .map(|_| ())
                    .map_err(|errors| errors.into_parts().0)
            },
        );
        assert_path(
            AllocationTarget::NormalTerms,
            DimensionResource::BasisCount,
            2,
            || {
                raw_two()
                    .normalize()
                    .map(|_| ())
                    .map_err(|errors| errors.into_parts().0)
            },
        );

        assert_path(
            AllocationTarget::NormalTerms,
            DimensionResource::BasisCount,
            1,
            || FormSum::one().map(|_| ()),
        );
        let monomial = RawMonomial::positive(vec![]).normalize().unwrap();
        assert_path(
            AllocationTarget::NormalTerms,
            DimensionResource::BasisCount,
            1,
            || FormSum::from_monomial(&monomial).map(|_| ()),
        );
        let normal = form(vec![integer(1), radical(2)]);
        assert_path(
            AllocationTarget::NormalTerms,
            DimensionResource::BasisCount,
            2,
            || normal.try_clone().map(|_| ()),
        );

        let invalid = || {
            RawFormSum::new(vec![
                RawTerm::new(rational(1, 0), RawMonomial::positive(vec![])),
                RawTerm::new(
                    rational(1, 1),
                    RawMonomial::positive(vec![RawPower::new(BigUint::zero(), rational(0, 1))]),
                ),
            ])
        };
        assert_path(
            AllocationTarget::ErrorSet,
            DimensionResource::BasisCount,
            2,
            || {
                invalid()
                    .normalize()
                    .map(|_| ())
                    .map_err(|errors| errors.into_parts().0)
            },
        );

        let one_root_two = || form(vec![integer(1), radical(2)]);
        let root_three = || form(vec![radical(3)]);
        assert_path(
            AllocationTarget::MergeResult,
            DimensionResource::BasisCount,
            3,
            || one_root_two().add(&root_three()).map(|_| ()),
        );
        assert_path(
            AllocationTarget::ProductResult,
            DimensionResource::BasisCount,
            1,
            || form(vec![radical(2)]).mul(&root_three()).map(|_| ()),
        );
        assert_path(
            AllocationTarget::ProductFactors,
            DimensionResource::BasisCount,
            2,
            || form(vec![radical(2)]).mul(&root_three()).map(|_| ()),
        );

        let extension = form(vec![radical(2)])
            .extension_with(&FormSum::zero())
            .unwrap();
        assert_path(
            AllocationTarget::ExtensionPrimes,
            DimensionResource::Denominator,
            1,
            || extension.try_clone().map(|_| ()),
        );
        assert_path(
            AllocationTarget::ExtensionDenominators,
            DimensionResource::Denominator,
            1,
            || extension.try_clone().map(|_| ()),
        );

        assert_path(
            AllocationTarget::ExtensionPrimes,
            DimensionResource::Denominator,
            1,
            || {
                form(vec![radical(2)])
                    .extension_with(&FormSum::zero())
                    .map(|_| ())
            },
        );
        assert_path(
            AllocationTarget::ExtensionDenominators,
            DimensionResource::Denominator,
            1,
            || {
                form(vec![radical(2)])
                    .extension_with(&FormSum::zero())
                    .map(|_| ())
            },
        );

        let coordinates_operation = |target| {
            assert_path(
                target,
                match target {
                    AllocationTarget::ExtensionFactors => DimensionResource::Denominator,
                    _ => DimensionResource::BasisCount,
                },
                match target {
                    AllocationTarget::CoordinateValues => 2,
                    _ => 1,
                },
                || {
                    let value = form(vec![radical(2)]);
                    let extension = value.extension_with(&FormSum::zero())?;
                    value.coordinates_with(&extension).map(|_| ())
                },
            );
        };
        coordinates_operation(AllocationTarget::ExtensionFactors);
        coordinates_operation(AllocationTarget::CoordinateValues);

        let coordinate_value = form(vec![radical(2)]);
        let coordinate_extension = coordinate_value.extension_with(&FormSum::zero()).unwrap();
        let coordinates = coordinate_value
            .coordinates_with(&coordinate_extension)
            .unwrap();
        assert_path(
            AllocationTarget::CoordinateValues,
            DimensionResource::BasisCount,
            2,
            || coordinates.try_clone().map(|_| ()),
        );
        assert_path(
            AllocationTarget::CoordinateTerms,
            DimensionResource::BasisCount,
            1,
            || {
                coordinate_value
                    .coordinates_with(&coordinate_extension)
                    .map(|_| ())
            },
        );

        for target in [
            AllocationTarget::CoordinateTerms,
            AllocationTarget::CoordinateFactors,
        ] {
            assert_path(target, DimensionResource::BasisCount, 1, || {
                coordinates.try_clone()?.into_form_sum().map(|_| ())
            });
        }
        assert_path(
            AllocationTarget::MultiplicationMatrix,
            DimensionResource::MatrixElementCount,
            4,
            || {
                let value = form(vec![radical(2)]);
                let extension = value.extension_with(&FormSum::zero())?;
                value
                    .coordinates_with(&extension)?
                    .multiplication_matrix()
                    .map(|_| ())
            },
        );
        assert_path(
            AllocationTarget::GaussianRightHandSide,
            DimensionResource::BasisCount,
            2,
            || form(vec![radical(2)]).inverse().map(|_| ()),
        );
        assert_path(
            AllocationTarget::GaussianMatrix,
            DimensionResource::MatrixElementCount,
            4,
            || form(vec![radical(2)]).inverse().map(|_| ()),
        );

        for (target, resource, total) in [
            (
                AllocationTarget::RecurrenceCoefficients,
                DimensionResource::BasisCount,
                3,
            ),
            (
                AllocationTarget::AnnihilatingInputMatrix,
                DimensionResource::MatrixElementCount,
                4,
            ),
            (
                AllocationTarget::RecurrenceStateMatrix,
                DimensionResource::MatrixElementCount,
                4,
            ),
            (
                AllocationTarget::RecurrenceProductMatrix,
                DimensionResource::MatrixElementCount,
                4,
            ),
            (
                AllocationTarget::IntegerCoefficients,
                DimensionResource::BasisCount,
                3,
            ),
        ] {
            assert_path(target, resource, total, || {
                form(vec![radical(2)])
                    .annihilating_coefficients()
                    .map(|_| ())
            });
        }
        assert_path(
            AllocationTarget::AnnihilatingCoefficients,
            DimensionResource::BasisCount,
            3,
            || {
                form(vec![radical(2)])
                    .annihilating_coefficients()?
                    .try_clone()
                    .map(|_| ())
            },
        );
    }

    #[test]
    fn matrix_overflow_reports_the_exact_square() {
        #[cfg(target_pointer_width = "64")]
        let dimension = 4_294_967_297_usize;
        #[cfg(target_pointer_width = "32")]
        let dimension = 65_537_usize;
        let exact = BigUint::try_from(dimension).unwrap();
        let required = exact.mul(&exact).unwrap();
        assert_eq!(
            checked_square_dimension(dimension).unwrap_err(),
            FormSumErrorKind::DimensionOverflow {
                resource: DimensionResource::MatrixElementCount,
                required,
                maximum: BigUint::try_from(usize::MAX).unwrap(),
            }
        );
    }
}
