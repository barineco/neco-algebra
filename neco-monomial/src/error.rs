use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;
use core::mem::size_of;

use neco_bigint::BigintError;

#[derive(Debug, Eq, PartialEq)]
pub enum MonomialErrorKind {
    DivisionByZero,
    ZeroToNegativePower,
    UndefinedZeroPower,
    EvenRootOfNegative,
    InvalidRadicalBasis,
    CapacityOverflow,
    AllocationFailure { requested_elements: usize },
    Bigint(BigintError),
}

impl MonomialErrorKind {
    pub fn try_clone(&self) -> Result<Self, MonomialErrorKind> {
        self.try_clone_internal()
    }

    pub(crate) fn try_clone_internal(&self) -> Result<Self, MonomialErrorKind> {
        match self {
            Self::DivisionByZero => Ok(Self::DivisionByZero),
            Self::ZeroToNegativePower => Ok(Self::ZeroToNegativePower),
            Self::UndefinedZeroPower => Ok(Self::UndefinedZeroPower),
            Self::EvenRootOfNegative => Ok(Self::EvenRootOfNegative),
            Self::InvalidRadicalBasis => Ok(Self::InvalidRadicalBasis),
            Self::CapacityOverflow => Ok(Self::CapacityOverflow),
            Self::AllocationFailure { requested_elements } => Ok(Self::AllocationFailure {
                requested_elements: *requested_elements,
            }),
            Self::Bigint(error) => Ok(Self::Bigint(try_clone_bigint_error(error)?)),
        }
    }
}

impl Ord for MonomialErrorKind {
    fn cmp(&self, other: &Self) -> Ordering {
        compare_monomial_errors(self, other)
    }
}

pub(crate) fn compare_monomial_errors(
    left: &MonomialErrorKind,
    right: &MonomialErrorKind,
) -> Ordering {
    error_discriminant(left)
        .cmp(&error_discriminant(right))
        .then_with(|| match (left, right) {
            (
                MonomialErrorKind::AllocationFailure {
                    requested_elements: left,
                },
                MonomialErrorKind::AllocationFailure {
                    requested_elements: right,
                },
            ) => left.cmp(right),
            (MonomialErrorKind::Bigint(left), MonomialErrorKind::Bigint(right)) => {
                compare_bigint_error(left, right)
            }
            _ => Ordering::Equal,
        })
}

impl PartialOrd for MonomialErrorKind {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for MonomialErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::DivisionByZero => "division by zero",
            Self::ZeroToNegativePower => "zero raised to a negative power",
            Self::UndefinedZeroPower => "zero raised to the zero power",
            Self::EvenRootOfNegative => "even root of a negative value",
            Self::InvalidRadicalBasis => "invalid radical basis",
            Self::CapacityOverflow => "monomial element capacity exceeds isize",
            Self::AllocationFailure { .. } => "monomial element allocation failed",
            Self::Bigint(error) => return error.fmt(formatter),
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for MonomialErrorKind {}

#[derive(Debug, Eq, PartialEq)]
pub struct NormalizationErrors<E> {
    first: E,
    additional: Vec<E>,
}

impl<E> NormalizationErrors<E> {
    pub fn from_one(error: E) -> Self {
        Self {
            first: error,
            additional: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        1 + self.additional.len()
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn errors(&self) -> impl Iterator<Item = &E> {
        core::iter::once(&self.first).chain(self.additional.iter())
    }

    pub fn into_parts(self) -> (E, Vec<E>) {
        (self.first, self.additional)
    }
}

impl<E: Ord> NormalizationErrors<E> {
    pub fn from_errors(mut errors: Vec<E>) -> Option<Self> {
        errors.sort_unstable();
        errors.dedup();
        if errors.is_empty() {
            return None;
        }
        let first = errors.remove(0);
        Some(Self {
            first,
            additional: errors,
        })
    }
}

impl NormalizationErrors<MonomialErrorKind> {
    pub fn try_clone(&self) -> Result<Self, MonomialErrorKind> {
        let total = self.additional.len();
        let mut additional = Vec::new();
        if total != 0 {
            reserve_elements_for(&mut additional, total, AllocationTarget::ErrorSet)?;
        }
        for error in &self.additional {
            additional.push(error.try_clone_internal()?);
        }
        Ok(Self {
            first: self.first.try_clone_internal()?,
            additional,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AllocationTarget {
    RawPowerClone,
    NormalizationIndex,
    NormalFactor,
    MergeResult,
    ErrorSet,
    RadicalBasis,
}

pub(crate) fn reserve_elements_for<T>(
    values: &mut Vec<T>,
    total_required: usize,
    target: AllocationTarget,
) -> Result<(), MonomialErrorKind> {
    #[cfg(test)]
    if let Some(failure) = injected_failure(target, total_required) {
        return Err(failure);
    }
    total_required
        .checked_mul(size_of::<T>())
        .ok_or(MonomialErrorKind::CapacityOverflow)?;
    let maximum = if size_of::<T>() == 0 {
        usize::MAX
    } else {
        (isize::MAX as usize) / size_of::<T>()
    };

    let additional = total_required
        .checked_sub(values.len())
        .ok_or(MonomialErrorKind::CapacityOverflow)?;
    reserve_elements_for_with(target, total_required, maximum, |_, _| {
        values.try_reserve(additional).map_err(|_| ())
    })
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
    static OBSERVED_TOTAL: core::cell::Cell<Option<usize>> = const { core::cell::Cell::new(None) };
}

#[cfg(test)]
fn injected_failure(target: AllocationTarget, total_required: usize) -> Option<MonomialErrorKind> {
    INJECTED_TARGET.with(|configured| match configured.get() {
        Some((expected, failure)) if expected == target => {
            OBSERVED_TOTAL.with(|observed| observed.set(Some(total_required)));
            Some(match failure {
                InjectedFailure::Capacity => MonomialErrorKind::CapacityOverflow,
                InjectedFailure::Allocation => MonomialErrorKind::AllocationFailure {
                    requested_elements: total_required,
                },
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
) -> (R, Option<usize>) {
    INJECTED_TARGET.with(|configured| configured.set(Some((target, failure))));
    OBSERVED_TOTAL.with(|observed| observed.set(None));
    let result = operation();
    let total = OBSERVED_TOTAL.with(core::cell::Cell::get);
    INJECTED_TARGET.with(|configured| configured.set(None));
    (result, total)
}

pub(crate) fn reserve_elements_for_with<F>(
    target: AllocationTarget,
    total_required: usize,
    maximum: usize,
    reserve_fn: F,
) -> Result<(), MonomialErrorKind>
where
    F: FnOnce(AllocationTarget, usize) -> Result<(), ()>,
{
    if total_required > maximum {
        return Err(MonomialErrorKind::CapacityOverflow);
    }
    reserve_fn(target, total_required).map_err(|()| MonomialErrorKind::AllocationFailure {
        requested_elements: total_required,
    })
}

fn error_discriminant(error: &MonomialErrorKind) -> u8 {
    match error {
        MonomialErrorKind::DivisionByZero => 0,
        MonomialErrorKind::ZeroToNegativePower => 1,
        MonomialErrorKind::UndefinedZeroPower => 2,
        MonomialErrorKind::EvenRootOfNegative => 3,
        MonomialErrorKind::InvalidRadicalBasis => 4,
        MonomialErrorKind::CapacityOverflow => 5,
        MonomialErrorKind::AllocationFailure { .. } => 6,
        MonomialErrorKind::Bigint(_) => 7,
    }
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

fn compare_bigint_error(left: &BigintError, right: &BigintError) -> Ordering {
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
                    required: left_required,
                    maximum: left_maximum,
                },
                BigintError::ExponentOverflow {
                    required: right_required,
                    maximum: right_maximum,
                },
            ) => left_required
                .cmp(right_required)
                .then_with(|| left_maximum.cmp(right_maximum)),
            _ => Ordering::Equal,
        })
}

fn try_clone_bigint_error(error: &BigintError) -> Result<BigintError, MonomialErrorKind> {
    Ok(match error {
        BigintError::CapacityOverflow => BigintError::CapacityOverflow,
        BigintError::AllocationFailure { requested_limbs } => BigintError::AllocationFailure {
            requested_limbs: *requested_limbs,
        },
        BigintError::UnsignedUnderflow => BigintError::UnsignedUnderflow,
        BigintError::DivisionByZero => BigintError::DivisionByZero,
        BigintError::NonExactDivision => BigintError::NonExactDivision,
        BigintError::ZeroDenominator => BigintError::ZeroDenominator,
        BigintError::NonFiniteFloat => BigintError::NonFiniteFloat,
        BigintError::FloatOutOfRange => BigintError::FloatOutOfRange,
        BigintError::InvalidInterval => BigintError::InvalidInterval,
        BigintError::ExponentOverflow { required, maximum } => BigintError::ExponentOverflow {
            required: required.try_clone().map_err(MonomialErrorKind::Bigint)?,
            maximum: *maximum,
        },
    })
}

impl From<BigintError> for MonomialErrorKind {
    fn from(error: BigintError) -> Self {
        Self::Bigint(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        reserve_elements_for_with, with_injected_failure, AllocationTarget, InjectedFailure,
        MonomialErrorKind,
    };
    use alloc::vec;
    use neco_bigint::{BigInt, BigUint, BigintError, RawRational};

    use crate::{Monomial, RawMonomial, RawPower};

    fn power(base: u8, numerator: i8, denominator: u8) -> RawPower {
        RawPower::new(
            BigUint::try_from(base).unwrap(),
            RawRational::new(
                BigInt::try_from(numerator).unwrap(),
                BigUint::try_from(denominator).unwrap(),
            ),
        )
    }

    fn positive(powers: alloc::vec::Vec<RawPower>) -> Monomial {
        RawMonomial::positive(powers).normalize().unwrap()
    }

    #[test]
    fn reserve_reports_capacity_before_calling_allocator() {
        let mut called = false;
        let result = reserve_elements_for_with(AllocationTarget::RawPowerClone, 5, 4, |_, _| {
            called = true;
            Ok(())
        });
        assert_eq!(result, Err(MonomialErrorKind::CapacityOverflow));
        assert!(!called);
    }

    #[test]
    fn every_storage_target_reports_total_required_on_selective_failure() {
        let targets = [
            AllocationTarget::RawPowerClone,
            AllocationTarget::NormalizationIndex,
            AllocationTarget::NormalFactor,
            AllocationTarget::MergeResult,
            AllocationTarget::ErrorSet,
            AllocationTarget::RadicalBasis,
        ];
        for failed in targets {
            for current in targets {
                let result = reserve_elements_for_with(current, 7, 7, |target, total| {
                    assert_eq!(total, 7);
                    if target == failed {
                        Err(())
                    } else {
                        Ok(())
                    }
                });
                if current == failed {
                    assert_eq!(
                        result,
                        Err(MonomialErrorKind::AllocationFailure {
                            requested_elements: 7
                        })
                    );
                } else {
                    assert_eq!(result, Ok(()));
                }
            }
        }
    }

    #[test]
    fn every_storage_target_is_reached_from_its_operation() {
        for failure in [InjectedFailure::Capacity, InjectedFailure::Allocation] {
            let expected = |total| match failure {
                InjectedFailure::Capacity => MonomialErrorKind::CapacityOverflow,
                InjectedFailure::Allocation => MonomialErrorKind::AllocationFailure {
                    requested_elements: total,
                },
            };

            let raw = RawMonomial::positive(vec![power(2, 1, 1), power(3, 1, 1)]);
            let (result, observed) =
                with_injected_failure(AllocationTarget::RawPowerClone, failure, || raw.try_clone());
            assert_eq!(result, Err(expected(2)));
            assert_eq!(observed, Some(2));

            let raw = RawMonomial::positive(vec![power(2, 1, 1), power(3, 1, 1)]);
            let (result, observed) =
                with_injected_failure(AllocationTarget::NormalizationIndex, failure, || {
                    raw.normalize()
                });
            assert!(result.unwrap_err().errors().eq([&expected(2)]));
            assert_eq!(observed, Some(2));

            let raw = RawMonomial::positive(vec![power(6, 1, 1)]);
            let (result, observed) =
                with_injected_failure(AllocationTarget::NormalFactor, failure, || raw.normalize());
            assert!(result.unwrap_err().errors().eq([&expected(2)]));
            assert_eq!(observed, Some(2));

            let left = positive(vec![power(2, 1, 1)]);
            let right = positive(vec![power(3, 1, 1)]);
            let (result, observed) =
                with_injected_failure(AllocationTarget::MergeResult, failure, || left.mul(&right));
            assert_eq!(result, Err(expected(2)));
            assert_eq!(observed, Some(2));

            let raw = RawMonomial::positive(vec![power(0, -1, 1), power(0, 0, 1)]);
            let (result, observed) =
                with_injected_failure(AllocationTarget::ErrorSet, failure, || raw.normalize());
            assert!(result.unwrap_err().errors().eq([&expected(2)]));
            assert_eq!(observed, Some(2));

            let radical = positive(vec![power(6, 1, 2)]);
            let (result, observed) =
                with_injected_failure(AllocationTarget::RadicalBasis, failure, || {
                    radical.split_radical()
                });
            assert_eq!(result, Err(expected(2)));
            assert_eq!(observed, Some(2));
        }
    }

    #[test]
    fn singleton_invalid_does_not_request_error_storage() {
        let raw = RawMonomial::positive(vec![power(0, -1, 1)]);
        let (result, observed) = with_injected_failure(
            AllocationTarget::ErrorSet,
            InjectedFailure::Allocation,
            || raw.normalize(),
        );
        assert!(result
            .unwrap_err()
            .errors()
            .eq([&MonomialErrorKind::ZeroToNegativePower]));
        assert_eq!(observed, None);
    }

    #[test]
    fn payload_comparison_uses_numeric_order() {
        let mut errors = vec![
            MonomialErrorKind::Bigint(BigintError::ExponentOverflow {
                required: BigUint::try_from(11_u8).unwrap(),
                maximum: 9,
            }),
            MonomialErrorKind::AllocationFailure {
                requested_elements: 8,
            },
            MonomialErrorKind::Bigint(BigintError::AllocationFailure { requested_limbs: 5 }),
            MonomialErrorKind::AllocationFailure {
                requested_elements: 3,
            },
            MonomialErrorKind::Bigint(BigintError::ExponentOverflow {
                required: BigUint::try_from(7_u8).unwrap(),
                maximum: 12,
            }),
        ];
        errors.sort();

        assert_eq!(
            errors,
            vec![
                MonomialErrorKind::AllocationFailure {
                    requested_elements: 3
                },
                MonomialErrorKind::AllocationFailure {
                    requested_elements: 8
                },
                MonomialErrorKind::Bigint(BigintError::AllocationFailure { requested_limbs: 5 }),
                MonomialErrorKind::Bigint(BigintError::ExponentOverflow {
                    required: BigUint::try_from(7_u8).unwrap(),
                    maximum: 12
                }),
                MonomialErrorKind::Bigint(BigintError::ExponentOverflow {
                    required: BigUint::try_from(11_u8).unwrap(),
                    maximum: 9
                }),
            ]
        );
    }
}
