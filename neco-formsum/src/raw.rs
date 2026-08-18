use alloc::vec::Vec;
use core::cmp::Ordering;

use neco_bigint::RawRational;
use neco_monomial::RawMonomial;

use crate::error::{reserve_elements, AllocationTarget, DimensionResource, FormSumErrorKind};
use crate::formsum::normalize_raw;
use crate::{FormSum, NormalizationErrors};

#[derive(Debug, Eq, PartialEq)]
pub struct RawTerm {
    coefficient: RawRational,
    monomial: RawMonomial,
}

impl RawTerm {
    pub fn new(coefficient: RawRational, monomial: RawMonomial) -> Self {
        Self {
            coefficient,
            monomial,
        }
    }

    pub fn coefficient(&self) -> &RawRational {
        &self.coefficient
    }

    pub fn monomial(&self) -> &RawMonomial {
        &self.monomial
    }

    pub fn try_clone(&self) -> Result<Self, FormSumErrorKind> {
        Ok(Self {
            coefficient: self.coefficient.try_clone()?,
            monomial: self.monomial.try_clone()?,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct RawFormSum {
    terms: Vec<RawTerm>,
}

impl RawFormSum {
    pub fn new(terms: Vec<RawTerm>) -> Self {
        Self { terms }
    }

    pub fn terms(&self) -> &[RawTerm] {
        &self.terms
    }

    pub fn try_clone(&self) -> Result<Self, FormSumErrorKind> {
        let mut terms = Vec::new();
        reserve_elements(
            &mut terms,
            self.terms.len(),
            DimensionResource::BasisCount,
            AllocationTarget::RawTermClone,
        )?;
        for term in &self.terms {
            terms.push(term.try_clone()?);
        }
        Ok(Self { terms })
    }

    pub fn normalize(&self) -> Result<FormSum, NormalizationErrors<FormSumErrorKind>> {
        normalize_raw(self)
    }

    pub(crate) fn sorted_indices(&self) -> Result<Vec<usize>, FormSumErrorKind> {
        let mut indices = Vec::new();
        reserve_elements(
            &mut indices,
            self.terms.len(),
            DimensionResource::BasisCount,
            AllocationTarget::NormalizationIndex,
        )?;
        indices.extend(0..self.terms.len());
        indices.sort_unstable_by(|left, right| {
            compare_raw_terms(&self.terms[*left], &self.terms[*right])
        });
        Ok(indices)
    }
}

fn compare_raw_terms(left: &RawTerm, right: &RawTerm) -> Ordering {
    left.coefficient
        .numerator()
        .cmp(right.coefficient.numerator())
        .then_with(|| {
            left.coefficient
                .denominator()
                .cmp(right.coefficient.denominator())
        })
        .then_with(|| left.monomial.is_zero().cmp(&right.monomial.is_zero()))
        .then_with(|| left.monomial.sign().cmp(&right.monomial.sign()))
        .then_with(|| compare_powers(left.monomial.powers(), right.monomial.powers()))
}

fn compare_powers(left: &[neco_monomial::RawPower], right: &[neco_monomial::RawPower]) -> Ordering {
    for (left, right) in left.iter().zip(right) {
        let ordering = left
            .base()
            .cmp(right.base())
            .then_with(|| {
                left.exponent()
                    .numerator()
                    .cmp(right.exponent().numerator())
            })
            .then_with(|| {
                left.exponent()
                    .denominator()
                    .cmp(right.exponent().denominator())
            });
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}
