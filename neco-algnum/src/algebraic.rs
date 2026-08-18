use alloc::{vec, vec::Vec};

use core::cmp::Ordering;

use neco_bigint::{BigInt, BigUint, Dyadic, DyadicEnclosure, ReducedRational, Sign};
use neco_formsum::FormSum;

use crate::error::{reserve_elements_at, AlgnumError, AllocationContact, AllocationResource};
use crate::factor::IrreduciblePolynomial;
use crate::polynomial::{Polynomial, RationalPolynomial, SquareFreePolynomial};
use crate::resultant::resultant;
use crate::sturm::SturmSequence;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RootIndex(usize);

#[derive(Debug, Eq, PartialEq)]
pub struct MinimalPolynomial {
    polynomial: IrreduciblePolynomial,
}

#[derive(Debug, Eq, PartialEq)]
pub struct PolynomialQuotient {
    minimal_polynomial: MinimalPolynomial,
}

#[derive(Debug, Eq, PartialEq)]
pub struct GeneratorRepresentative {
    minimal_polynomial: MinimalPolynomial,
}

#[derive(Debug, Eq, PartialEq)]
pub struct RealAlgebraic {
    minimal_polynomial: MinimalPolynomial,
    root_index: RootIndex,
}

#[derive(Debug, Eq, PartialEq)]
pub struct RationalCoefficientConversion {
    pub(crate) coefficients: Vec<RealAlgebraic>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct IsolatingInterval {
    value: RealAlgebraic,
    enclosure: DyadicEnclosure,
}

#[derive(Debug, Eq, PartialEq)]
pub struct CertifiedAlgebraic {
    value: RealAlgebraic,
    enclosure: DyadicEnclosure,
}

impl RootIndex {
    pub fn get(&self) -> usize {
        self.0
    }

    pub(crate) fn new(index: usize) -> Self {
        Self(index)
    }
}

impl MinimalPolynomial {
    pub fn polynomial(&self) -> &crate::Polynomial {
        self.polynomial.polynomial()
    }

    pub fn try_clone(&self) -> Result<Self, AlgnumError> {
        Ok(Self {
            polynomial: self.polynomial.try_clone()?,
        })
    }

    pub fn quotient(&self) -> Result<PolynomialQuotient, AlgnumError> {
        Ok(PolynomialQuotient {
            minimal_polynomial: self.try_clone()?,
        })
    }

    pub(crate) fn new(polynomial: IrreduciblePolynomial) -> Self {
        Self { polynomial }
    }
}

impl PolynomialQuotient {
    pub fn minimal_polynomial(&self) -> &MinimalPolynomial {
        &self.minimal_polynomial
    }

    pub fn try_clone(&self) -> Result<Self, AlgnumError> {
        Ok(Self {
            minimal_polynomial: self.minimal_polynomial.try_clone()?,
        })
    }

    pub fn reduce(&self, value: &RationalPolynomial) -> Result<RationalPolynomial, AlgnumError> {
        let modulus = RationalPolynomial::from_integer(self.minimal_polynomial.polynomial())?;
        value.div_rem(&modulus).map(|(_, remainder)| remainder)
    }

    pub fn add(
        &self,
        lhs: &RationalPolynomial,
        rhs: &RationalPolynomial,
    ) -> Result<RationalPolynomial, AlgnumError> {
        self.reduce(&lhs.add(rhs)?)
    }

    pub fn sub(
        &self,
        lhs: &RationalPolynomial,
        rhs: &RationalPolynomial,
    ) -> Result<RationalPolynomial, AlgnumError> {
        self.reduce(&lhs.sub(rhs)?)
    }

    pub fn mul(
        &self,
        lhs: &RationalPolynomial,
        rhs: &RationalPolynomial,
    ) -> Result<RationalPolynomial, AlgnumError> {
        self.reduce(&lhs.mul(rhs)?)
    }

    pub fn generator(&self) -> Result<GeneratorRepresentative, AlgnumError> {
        Ok(GeneratorRepresentative {
            minimal_polynomial: self.minimal_polynomial.try_clone()?,
        })
    }
}

impl GeneratorRepresentative {
    pub fn minimal_polynomial(&self) -> &MinimalPolynomial {
        &self.minimal_polynomial
    }

    pub fn try_clone(&self) -> Result<Self, AlgnumError> {
        Ok(Self {
            minimal_polynomial: self.minimal_polynomial.try_clone()?,
        })
    }

    pub fn as_polynomial(&self) -> Result<RationalPolynomial, AlgnumError> {
        let zero = ReducedRational::from_bigint(BigInt::zero())?;
        let one = ReducedRational::from_bigint(BigInt::one()?)?;
        let mut coefficients = Vec::new();
        reserve_elements_at(
            &mut coefficients,
            2,
            AllocationResource::RationalCoefficients,
            AllocationContact::GeneratorPolynomial,
        )?;
        coefficients.push(zero);
        coefficients.push(one);
        self.minimal_polynomial
            .quotient()?
            .reduce(&RationalPolynomial::from_coefficients(coefficients))
    }
}

impl RationalCoefficientConversion {
    pub fn coefficients(&self) -> &[RealAlgebraic] {
        &self.coefficients
    }

    pub fn into_coefficients(self) -> Vec<RealAlgebraic> {
        self.coefficients
    }

    pub fn try_clone(&self) -> Result<Self, AlgnumError> {
        let mut coefficients = Vec::new();
        reserve_elements_at(
            &mut coefficients,
            self.coefficients.len(),
            AllocationResource::RationalCoefficients,
            AllocationContact::RationalClone,
        )?;
        for coefficient in &self.coefficients {
            coefficients.push(coefficient.try_clone()?);
        }
        Ok(Self { coefficients })
    }
}

impl RealAlgebraic {
    pub fn from_integer(value: BigInt) -> Result<Self, AlgnumError> {
        let one = BigInt::one()?;
        let polynomial = Polynomial::from_coefficients(vec![value.negated()?, one]);
        let candidate = polynomial.candidate()?.square_free()?;
        let factor = candidate
            .factor()?
            .into_iter()
            .next()
            .ok_or(AlgnumError::NoTargetRoot)?;
        let roots = factor.isolate_real_roots()?;
        roots
            .into_iter()
            .next()
            .map(CertifiedAlgebraic::into_value)
            .ok_or(AlgnumError::NoTargetRoot)
    }

    pub fn from_reduced_rational(value: &ReducedRational) -> Result<Self, AlgnumError> {
        let denominator =
            BigInt::from_sign_magnitude(Sign::Positive, value.denominator().try_clone()?);
        let polynomial =
            Polynomial::from_coefficients(vec![value.numerator().negated()?, denominator]);
        let candidate = polynomial.candidate()?.square_free()?;
        let factor = candidate
            .factor()?
            .into_iter()
            .next()
            .ok_or(AlgnumError::NoTargetRoot)?;
        let roots = factor.isolate_real_roots()?;
        roots
            .into_iter()
            .next()
            .map(CertifiedAlgebraic::into_value)
            .ok_or(AlgnumError::NoTargetRoot)
    }

    pub fn minimal_polynomial(&self) -> &MinimalPolynomial {
        &self.minimal_polynomial
    }

    pub fn root_index(&self) -> RootIndex {
        self.root_index
    }

    pub fn try_clone(&self) -> Result<Self, AlgnumError> {
        Ok(Self {
            minimal_polynomial: self.minimal_polynomial.try_clone()?,
            root_index: self.root_index,
        })
    }

    pub fn is_zero(&self) -> bool {
        let coefficients = self.minimal_polynomial.polynomial().coefficients();
        coefficients.len() == 2
            && coefficients[0].is_zero()
            && coefficients[1].sign() == Sign::Positive
            && coefficients[1].magnitude().limbs_le() == [1]
            && self.root_index.0 == 0
    }

    pub fn is_one(&self) -> bool {
        let coefficients = self.minimal_polynomial.polynomial().coefficients();
        coefficients.len() == 2
            && coefficients[0].sign() == Sign::Negative
            && coefficients[0].magnitude().limbs_le() == [1]
            && coefficients[1].sign() == Sign::Positive
            && coefficients[1].magnitude().limbs_le() == [1]
            && self.root_index.0 == 0
    }

    pub fn sign(&self) -> Result<Sign, AlgnumError> {
        if self.is_zero() {
            return Ok(Sign::Zero);
        }
        Ok(
            match self.compare_dyadic(&Dyadic::new(BigInt::zero(), 0))? {
                Ordering::Less => Sign::Negative,
                Ordering::Equal => Sign::Zero,
                Ordering::Greater => Sign::Positive,
            },
        )
    }

    pub fn enclose(&self, bits: u32) -> Result<IsolatingInterval, AlgnumError> {
        let sequence = SturmSequence::new(self.minimal_polynomial.polynomial())?;
        let roots = sequence.isolate_real_roots()?;
        let root = roots
            .into_iter()
            .find(|root| root.index == self.root_index.0)
            .ok_or(AlgnumError::NoTargetRoot)?;
        let enclosure = sequence.refine(&root.enclosure, bits)?;
        Ok(IsolatingInterval::new(self.try_clone()?, enclosure))
    }

    pub fn compare(&self, rhs: &Self) -> Result<Ordering, AlgnumError> {
        self.sub(rhs)?.sign().map(|sign| match sign {
            Sign::Negative => Ordering::Less,
            Sign::Zero => Ordering::Equal,
            Sign::Positive => Ordering::Greater,
        })
    }

    pub fn compare_dyadic(&self, rhs: &Dyadic) -> Result<Ordering, AlgnumError> {
        let sequence = SturmSequence::new(self.minimal_polynomial.polynomial())?;
        let value = dyadic_rational(rhs)?;
        let evaluated = self
            .minimal_polynomial
            .polynomial()
            .evaluate_rational(&value)?;
        if evaluated.is_zero() {
            let roots = sequence.isolate_real_roots()?;
            let rhs_index = roots
                .iter()
                .position(|root| root.enclosure.contains_dyadic(rhs))
                .ok_or(AlgnumError::NoTargetRoot)?;
            return Ok(self.root_index.0.cmp(&rhs_index));
        }

        let mut bits = 0_u32;
        loop {
            let interval = self.enclose(bits)?;
            if interval.enclosure().upper() < rhs {
                return Ok(Ordering::Less);
            }
            if interval.enclosure().lower() > rhs {
                return Ok(Ordering::Greater);
            }
            bits = bits.checked_add(1).ok_or_else(exponent_overflow)?;
        }
    }

    pub fn add(&self, rhs: &Self) -> Result<Self, AlgnumError> {
        let left = constant_y_polynomial(self.minimal_polynomial.polynomial())?;
        let right = substitute_x_minus_y(rhs.minimal_polynomial.polynomial())?;
        let candidate = resultant(&left, &right)?;
        select_candidate(candidate, |bits| {
            let (left_bits, right_bits) = binary_selection_precisions(bits);
            let left = self.enclose(left_bits)?;
            let right = rhs.enclose(right_bits)?;
            enclosure_add(left.enclosure(), right.enclosure())
        })
    }

    pub fn sub(&self, rhs: &Self) -> Result<Self, AlgnumError> {
        let left = constant_y_polynomial(self.minimal_polynomial.polynomial())?;
        let right = substitute_y_minus_x(rhs.minimal_polynomial.polynomial())?;
        let candidate = resultant(&left, &right)?;
        select_candidate(candidate, |bits| {
            let (left_bits, right_bits) = binary_selection_precisions(bits);
            let left = self.enclose(left_bits)?;
            let right = rhs.enclose(right_bits)?;
            enclosure_sub(left.enclosure(), right.enclosure())
        })
    }

    pub fn mul(&self, rhs: &Self) -> Result<Self, AlgnumError> {
        let left = constant_y_polynomial(self.minimal_polynomial.polynomial())?;
        let right = multiplication_substitution(rhs.minimal_polynomial.polynomial())?;
        let candidate = resultant(&left, &right)?;
        select_candidate(candidate, |bits| {
            let (left_bits, right_bits) = binary_selection_precisions(bits);
            let left = self.enclose(left_bits)?;
            let right = rhs.enclose(right_bits)?;
            enclosure_mul(left.enclosure(), right.enclosure())
        })
    }

    pub fn div(&self, rhs: &Self) -> Result<Self, AlgnumError> {
        if rhs.is_zero() {
            return Err(AlgnumError::DivisionByZero);
        }
        self.mul(&rhs.reciprocal()?)
    }

    pub fn pow_integer(&self, exponent: &BigInt) -> Result<Self, AlgnumError> {
        if exponent.is_zero() {
            if self.is_zero() {
                return Err(AlgnumError::UndefinedZeroPower);
            }
            return algebraic_one();
        }
        if self.is_zero() && exponent.sign() == Sign::Negative {
            return Err(AlgnumError::ZeroToNegativePower);
        }
        let mut result = algebraic_one()?;
        let mut power = if exponent.sign() == Sign::Negative {
            self.reciprocal()?
        } else {
            self.try_clone()?
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

    pub fn pow_rational(&self, exponent: &ReducedRational) -> Result<Self, AlgnumError> {
        if exponent.is_zero() {
            if self.is_zero() {
                return Err(AlgnumError::UndefinedZeroPower);
            }
            return algebraic_one();
        }
        if self.is_zero() && exponent.numerator().sign() == Sign::Negative {
            return Err(AlgnumError::ZeroToNegativePower);
        }
        let degree = root_degree(exponent.denominator())?;
        self.nth_root(degree)?.pow_integer(exponent.numerator())
    }

    pub fn nth_root(&self, degree: u32) -> Result<Self, AlgnumError> {
        if degree == 0 {
            return Err(AlgnumError::ZeroRootDegree);
        }
        if self.is_zero() {
            return self.try_clone();
        }
        let sign = self.sign()?;
        if sign == Sign::Negative && degree & 1 == 0 {
            return Err(AlgnumError::EvenRootOfNegative);
        }
        let candidate = substitute_root_power(self.minimal_polynomial.polynomial(), degree)?;
        select_candidate(candidate, |bits| self.root_enclosure(degree, bits))
    }

    pub fn from_form_sum(value: &FormSum) -> Result<Self, AlgnumError> {
        let coefficients = value.annihilating_coefficients()?;
        let mut polynomial_coefficients = Vec::new();
        reserve_elements_at(
            &mut polynomial_coefficients,
            coefficients.coefficients().len(),
            AllocationResource::PolynomialCoefficients,
            AllocationContact::FormSumPolynomial,
        )?;
        for coefficient in coefficients.coefficients() {
            polynomial_coefficients.push(coefficient.try_clone()?);
        }
        let candidate = Polynomial::from_coefficients(polynomial_coefficients);
        select_candidate(candidate, |bits| Ok(value.enclose(bits)?))
    }

    pub fn equals_form_sum(&self, value: &FormSum) -> Result<bool, AlgnumError> {
        if self.is_zero() {
            return Ok(value.sign()? == Sign::Zero);
        }
        let mut evaluated = FormSum::zero();
        for coefficient in self
            .minimal_polynomial
            .polynomial()
            .coefficients()
            .iter()
            .rev()
        {
            evaluated = evaluated.mul(value)?;
            let coefficient = integer_form_sum(coefficient)?;
            evaluated = evaluated.add(&coefficient)?;
        }
        if !evaluated.is_zero() {
            return Ok(false);
        }

        let mut bits = 0_u32;
        loop {
            let form_interval = value.enclose(bits)?;
            let roots =
                SturmSequence::new(self.minimal_polynomial.polynomial())?.isolate_real_roots()?;
            let mut matched_index = None;
            let mut multiple = false;
            for root in roots {
                let refined = SturmSequence::new(self.minimal_polynomial.polynomial())?
                    .refine(&root.enclosure, bits)?;
                if enclosures_intersect(&form_interval, &refined) {
                    if matched_index.is_some() {
                        multiple = true;
                    } else {
                        matched_index = Some(root.index);
                    }
                }
            }
            if !multiple {
                if let Some(index) = matched_index {
                    return Ok(index == self.root_index.0);
                }
            }
            bits = bits.checked_add(1).ok_or_else(exponent_overflow)?;
        }
    }

    fn reciprocal(&self) -> Result<Self, AlgnumError> {
        if self.is_zero() {
            return Err(AlgnumError::DivisionByZero);
        }
        let candidate = reciprocal_polynomial(self.minimal_polynomial.polynomial())?;
        select_candidate(candidate, |bits| {
            let mut precision = bits;
            loop {
                let interval = self.enclose(precision)?;
                match enclosure_reciprocal(interval.enclosure(), bits) {
                    Err(AlgnumError::NoTargetRoot) => {
                        precision = precision.checked_add(1).ok_or_else(exponent_overflow)?;
                    }
                    result => return result,
                }
            }
        })
    }

    fn root_enclosure(&self, degree: u32, bits: u32) -> Result<DyadicEnclosure, AlgnumError> {
        let precision = bits.checked_add(2).ok_or_else(exponent_overflow)?;
        let interval = self.enclose(precision)?;
        enclosure_nth_root(interval.enclosure(), degree, bits)
    }

    pub(crate) fn new(minimal_polynomial: MinimalPolynomial, root_index: RootIndex) -> Self {
        Self {
            minimal_polynomial,
            root_index,
        }
    }
}

impl IsolatingInterval {
    pub fn value(&self) -> &RealAlgebraic {
        &self.value
    }

    pub fn enclosure(&self) -> &DyadicEnclosure {
        &self.enclosure
    }

    pub fn try_clone(&self) -> Result<Self, AlgnumError> {
        Ok(Self {
            value: self.value.try_clone()?,
            enclosure: self.enclosure.try_clone()?,
        })
    }

    pub fn refine(&self, bits: u32) -> Result<Self, AlgnumError> {
        let sequence = SturmSequence::new(self.value.minimal_polynomial().polynomial())?;
        Ok(Self {
            value: self.value.try_clone()?,
            enclosure: sequence.refine(&self.enclosure, bits)?,
        })
    }

    pub(crate) fn new(value: RealAlgebraic, enclosure: DyadicEnclosure) -> Self {
        Self { value, enclosure }
    }
}

impl IrreduciblePolynomial {
    pub fn isolate_real_roots(&self) -> Result<Vec<CertifiedAlgebraic>, AlgnumError> {
        let sequence = SturmSequence::new(self.polynomial())?;
        let roots = sequence.isolate_real_roots()?;
        let mut result = Vec::new();
        reserve_elements_at(
            &mut result,
            roots.len(),
            AllocationResource::RootCandidates,
            AllocationContact::IrreducibleRootOutput,
        )?;
        for root in roots {
            let minimal = MinimalPolynomial::new(self.try_clone()?);
            let value = RealAlgebraic::new(minimal, RootIndex::new(root.index));
            result.push(CertifiedAlgebraic::new(value, root.enclosure));
        }
        Ok(result)
    }

    pub fn certify_root(
        &self,
        lower: Dyadic,
        upper: Dyadic,
    ) -> Result<CertifiedAlgebraic, AlgnumError> {
        let root = SturmSequence::new(self.polynomial())?.certify_root(lower, upper)?;
        let minimal = MinimalPolynomial::new(self.try_clone()?);
        let value = RealAlgebraic::new(minimal, RootIndex::new(root.index));
        Ok(CertifiedAlgebraic::new(value, root.enclosure))
    }
}

fn visit_factors(
    factors: Vec<IrreduciblePolynomial>,
    mut visit: impl FnMut(IrreduciblePolynomial) -> Result<(), AlgnumError>,
) -> Result<(), AlgnumError> {
    for factor in factors.into_iter() {
        visit(factor)?;
    }
    Ok(())
}

impl SquareFreePolynomial {
    pub fn isolate_real_roots(&self) -> Result<Vec<CertifiedAlgebraic>, AlgnumError> {
        let factors = self.factor()?;
        let mut total = 0_usize;
        for factor in &factors {
            total += SturmSequence::new(factor.polynomial())?
                .isolate_real_roots()?
                .len();
        }
        let mut roots = Vec::new();
        reserve_elements_at(
            &mut roots,
            total,
            AllocationResource::RootCandidates,
            AllocationContact::SquareFreeRootOutput,
        )?;
        visit_factors(factors, |factor| {
            roots.extend(factor.isolate_real_roots()?);
            Ok(())
        })?;
        roots.sort_unstable_by(|left, right| {
            left.enclosure().lower().cmp(right.enclosure().lower())
        });
        let mut bits = 0_u32;
        while roots
            .windows(2)
            .any(|pair| pair[0].enclosure().upper() >= pair[1].enclosure().lower())
        {
            bits = bits.checked_add(1).ok_or_else(exponent_overflow)?;
            for root in &mut roots {
                let refined = root.value().enclose(bits)?;
                *root = CertifiedAlgebraic::new(
                    root.value().try_clone()?,
                    refined.enclosure().try_clone()?,
                );
            }
            roots.sort_unstable_by(|left, right| {
                left.enclosure().lower().cmp(right.enclosure().lower())
            });
        }
        Ok(roots)
    }

    pub fn certify_root(
        &self,
        lower: Dyadic,
        upper: Dyadic,
    ) -> Result<CertifiedAlgebraic, AlgnumError> {
        if lower >= upper {
            return Err(AlgnumError::InvalidIsolation);
        }
        let lower_value = dyadic_rational(&lower)?;
        let upper_value = dyadic_rational(&upper)?;
        if self.polynomial().evaluate_rational(&lower_value)?.is_zero()
            || self.polynomial().evaluate_rational(&upper_value)?.is_zero()
        {
            return Err(AlgnumError::InvalidIsolation);
        }
        let factors = self.factor()?;
        let mut selected: Option<CertifiedAlgebraic> = None;
        for factor in factors {
            match factor.certify_root(lower.try_clone()?, upper.try_clone()?) {
                Ok(root) => {
                    if selected.is_some() {
                        return Err(AlgnumError::MultipleTargetRoots);
                    }
                    selected = Some(root);
                }
                Err(AlgnumError::NoTargetRoot) => {}
                Err(error) => return Err(error),
            }
        }
        selected.ok_or(AlgnumError::NoTargetRoot)
    }
}

fn dyadic_rational(value: &Dyadic) -> Result<ReducedRational, AlgnumError> {
    let denominator = neco_bigint::BigUint::one()?.shl_bits(value.exponent() as usize)?;
    Ok(
        neco_bigint::RawRational::new(value.integer().try_clone()?, denominator)
            .reduce()?
            .into_reduced(),
    )
}

fn exponent_overflow() -> AlgnumError {
    let required = match neco_bigint::BigUint::try_from(u64::from(u32::MAX) + 1) {
        Ok(value) => value,
        Err(error) => return error.into(),
    };
    neco_bigint::BigintError::ExponentOverflow {
        required,
        maximum: u32::MAX,
    }
    .into()
}

fn coefficient_count_overflow() -> AlgnumError {
    let maximum = match neco_bigint::BigUint::try_from(usize::MAX) {
        Ok(value) => value,
        Err(error) => return error.into(),
    };
    let one = match neco_bigint::BigUint::one() {
        Ok(value) => value,
        Err(error) => return error.into(),
    };
    let required = match maximum.add(&one) {
        Ok(value) => value,
        Err(error) => return error.into(),
    };
    AlgnumError::coefficient_count_overflow(required)
}

fn select_candidate(
    candidate: Polynomial,
    mut result_enclosure: impl FnMut(u32) -> Result<DyadicEnclosure, AlgnumError>,
) -> Result<RealAlgebraic, AlgnumError> {
    let roots = candidate.candidate()?.square_free()?.isolate_real_roots()?;
    let mut bits = 0_u32;
    loop {
        let expected = result_enclosure(result_selection_precision(bits))?;
        let mut selected = None;
        let mut multiple = false;
        visit_roots(&roots, |root| {
            let actual = root.value().enclose(bits)?;
            if enclosures_intersect(&expected, actual.enclosure()) {
                if selected.is_some() {
                    multiple = true;
                } else {
                    selected = Some(root.value());
                }
            }
            Ok(())
        })?;
        if !multiple {
            match selected {
                None => return Err(AlgnumError::NoTargetRoot),
                Some(value) => return value.try_clone(),
            }
        }
        bits = bits.checked_add(1).ok_or_else(exponent_overflow)?;
    }
}

fn visit_roots<'a>(
    roots: &'a [CertifiedAlgebraic],
    mut visit: impl FnMut(&'a CertifiedAlgebraic) -> Result<(), AlgnumError>,
) -> Result<(), AlgnumError> {
    for root in roots.iter().take(candidate_root_visit_limit(roots.len())) {
        visit(root)?;
    }
    Ok(())
}

fn candidate_root_visit_limit(root_count: usize) -> usize {
    root_count
}

fn result_selection_precision(bits: u32) -> u32 {
    bits
}

fn binary_selection_precisions(bits: u32) -> (u32, u32) {
    (bits, bits)
}

fn constant_y_polynomial(polynomial: &Polynomial) -> Result<Vec<Polynomial>, AlgnumError> {
    let mut result = Vec::new();
    reserve_elements_at(
        &mut result,
        polynomial.coefficients().len(),
        AllocationResource::ResultantCoefficients,
        AllocationContact::ConstantYPolynomial,
    )?;
    for coefficient in polynomial.coefficients() {
        result.push(constant_polynomial(coefficient)?);
    }
    Ok(result)
}

fn substitute_x_minus_y(polynomial: &Polynomial) -> Result<Vec<Polynomial>, AlgnumError> {
    substitute_signed_x_signed_y(polynomial, false, true)
}

fn substitute_y_minus_x(polynomial: &Polynomial) -> Result<Vec<Polynomial>, AlgnumError> {
    substitute_signed_x_signed_y(polynomial, true, false)
}

fn substitute_signed_x_signed_y(
    polynomial: &Polynomial,
    negative_x: bool,
    negative_y: bool,
) -> Result<Vec<Polynomial>, AlgnumError> {
    let count = polynomial.coefficients().len();
    let mut result = Vec::new();
    reserve_elements_at(
        &mut result,
        count,
        AllocationResource::ResultantCoefficients,
        AllocationContact::SignedSubstitution,
    )?;
    for _ in 0..count {
        result.push(Polynomial::zero());
    }
    for (power, coefficient) in polynomial.coefficients().iter().enumerate() {
        let mut binomial = BigUint::one()?;
        for (y_power, slot) in result.iter_mut().enumerate().take(power + 1) {
            let x_power = power - y_power;
            let negate = (negative_y && y_power & 1 != 0) ^ (negative_x && x_power & 1 != 0);
            let sign = if negate {
                opposite_sign(coefficient.sign())
            } else {
                coefficient.sign()
            };
            let magnitude = coefficient.magnitude().mul(&binomial)?;
            let scalar = BigInt::from_sign_magnitude(sign, magnitude);
            let term = monomial_polynomial(x_power, scalar)?;
            *slot = slot.add(&term)?;
            if y_power < power {
                binomial = binomial
                    .mul(&BigUint::try_from(power - y_power)?)?
                    .exact_div(&BigUint::try_from(y_power + 1)?)?;
            }
        }
    }
    Ok(result)
}

fn multiplication_substitution(polynomial: &Polynomial) -> Result<Vec<Polynomial>, AlgnumError> {
    let degree = polynomial.degree().ok_or(AlgnumError::ZeroPolynomial)?;
    let mut result = Vec::new();
    reserve_elements_at(
        &mut result,
        polynomial.coefficients().len(),
        AllocationResource::ResultantCoefficients,
        AllocationContact::MultiplicationSubstitution,
    )?;
    for _ in 0..=degree {
        result.push(Polynomial::zero());
    }
    for (x_power, coefficient) in polynomial.coefficients().iter().enumerate() {
        result[degree - x_power] = monomial_polynomial(x_power, coefficient.try_clone()?)?;
    }
    Ok(result)
}

fn reciprocal_polynomial(polynomial: &Polynomial) -> Result<Polynomial, AlgnumError> {
    let mut coefficients = Vec::new();
    reserve_elements_at(
        &mut coefficients,
        polynomial.coefficients().len(),
        AllocationResource::PolynomialCoefficients,
        AllocationContact::ReciprocalPolynomial,
    )?;
    for coefficient in polynomial.coefficients().iter().rev() {
        coefficients.push(coefficient.try_clone()?);
    }
    Ok(Polynomial::from_coefficients(coefficients))
}

fn substitute_root_power(polynomial: &Polynomial, degree: u32) -> Result<Polynomial, AlgnumError> {
    let input_degree = polynomial.degree().ok_or(AlgnumError::ZeroPolynomial)?;
    let result_degree = BigUint::try_from(input_degree)?.mul(&BigUint::try_from(degree)?)?;
    let result_degree_usize = biguint_usize(
        &result_degree,
        crate::RepresentationResource::PolynomialDegree,
    )?;
    let count = result_degree_usize
        .checked_add(1)
        .ok_or_else(coefficient_count_overflow)?;
    let mut coefficients = Vec::new();
    reserve_elements_at(
        &mut coefficients,
        count,
        AllocationResource::PolynomialCoefficients,
        AllocationContact::RootPowerPolynomial,
    )?;
    for _ in 0..count {
        coefficients.push(BigInt::zero());
    }
    let degree_usize = degree as usize;
    for (index, coefficient) in polynomial.coefficients().iter().enumerate() {
        let result_index = index * degree_usize;
        coefficients[result_index] = coefficient.try_clone()?;
    }
    Ok(Polynomial::from_coefficients(coefficients))
}

fn constant_polynomial(value: &BigInt) -> Result<Polynomial, AlgnumError> {
    if value.is_zero() {
        return Ok(Polynomial::zero());
    }
    let mut coefficients = Vec::new();
    reserve_elements_at(
        &mut coefficients,
        1,
        AllocationResource::ResultantCoefficients,
        AllocationContact::ConstantPolynomial,
    )?;
    coefficients.push(value.try_clone()?);
    Ok(Polynomial::from_coefficients(coefficients))
}

fn monomial_polynomial(power: usize, coefficient: BigInt) -> Result<Polynomial, AlgnumError> {
    if coefficient.is_zero() {
        return Ok(Polynomial::zero());
    }
    let count = power
        .checked_add(1)
        .ok_or_else(coefficient_count_overflow)?;
    let mut coefficients = Vec::new();
    reserve_elements_at(
        &mut coefficients,
        count,
        AllocationResource::ResultantCoefficients,
        AllocationContact::MonomialPolynomial,
    )?;
    for _ in 0..power {
        coefficients.push(BigInt::zero());
    }
    coefficients.push(coefficient);
    Ok(Polynomial::from_coefficients(coefficients))
}

fn enclosure_add(
    left: &DyadicEnclosure,
    right: &DyadicEnclosure,
) -> Result<DyadicEnclosure, AlgnumError> {
    Ok(DyadicEnclosure::new(
        left.lower().add(right.lower())?,
        left.upper().add(right.upper())?,
    )?)
}

fn enclosure_sub(
    left: &DyadicEnclosure,
    right: &DyadicEnclosure,
) -> Result<DyadicEnclosure, AlgnumError> {
    Ok(DyadicEnclosure::new(
        left.lower().sub(right.upper())?,
        left.upper().sub(right.lower())?,
    )?)
}

fn enclosure_mul(
    left: &DyadicEnclosure,
    right: &DyadicEnclosure,
) -> Result<DyadicEnclosure, AlgnumError> {
    let products = [
        left.lower().mul(right.lower())?,
        left.lower().mul(right.upper())?,
        left.upper().mul(right.lower())?,
        left.upper().mul(right.upper())?,
    ];
    let lower = products
        .iter()
        .min()
        .ok_or(AlgnumError::InvalidIsolation)?
        .try_clone()?;
    let upper = products
        .iter()
        .max()
        .ok_or(AlgnumError::InvalidIsolation)?
        .try_clone()?;
    Ok(DyadicEnclosure::new(lower, upper)?)
}

fn enclosure_reciprocal(
    enclosure: &DyadicEnclosure,
    bits: u32,
) -> Result<DyadicEnclosure, AlgnumError> {
    let zero = Dyadic::new(BigInt::zero(), 0);
    if enclosure.contains_dyadic(&zero) {
        return Err(AlgnumError::NoTargetRoot);
    }
    let lower_inverse = reciprocal_rational(enclosure.upper())?;
    let upper_inverse = reciprocal_rational(enclosure.lower())?;
    Ok(DyadicEnclosure::new(
        lower_inverse.dyadic_floor(bits)?,
        upper_inverse.dyadic_ceil(bits)?,
    )?)
}

fn enclosure_nth_root(
    enclosure: &DyadicEnclosure,
    degree: u32,
    bits: u32,
) -> Result<DyadicEnclosure, AlgnumError> {
    let lower = nth_root_endpoint(enclosure.lower(), degree, bits, false)?;
    let upper = nth_root_endpoint(enclosure.upper(), degree, bits, true)?;
    Ok(DyadicEnclosure::new(lower, upper)?)
}

fn nth_root_endpoint(
    value: &Dyadic,
    degree: u32,
    bits: u32,
    upward: bool,
) -> Result<Dyadic, AlgnumError> {
    if value.is_zero() {
        return Ok(Dyadic::new(BigInt::zero(), 0));
    }
    let negative = value.integer().sign() == Sign::Negative;
    if negative && degree & 1 == 0 {
        return Err(AlgnumError::EvenRootOfNegative);
    }
    let (floor, exact) =
        positive_root_floor(value.integer().magnitude(), value.exponent(), degree, bits)?;
    let one = BigUint::one()?;
    let ceil = if exact {
        floor.try_clone()?
    } else {
        floor.add(&one)?
    };
    let magnitude = if negative {
        if upward {
            floor
        } else {
            ceil
        }
    } else if upward {
        ceil
    } else {
        floor
    };
    let sign = if magnitude.is_zero() {
        Sign::Zero
    } else if negative {
        Sign::Negative
    } else {
        Sign::Positive
    };
    Ok(Dyadic::new(
        BigInt::from_sign_magnitude(sign, magnitude),
        bits,
    ))
}

fn positive_root_floor(
    magnitude: &BigUint,
    exponent: u32,
    degree: u32,
    bits: u32,
) -> Result<(BigUint, bool), AlgnumError> {
    let right_shift = u64::from(bits)
        .checked_mul(u64::from(degree))
        .ok_or_else(exponent_overflow)?;
    let right_shift = usize::try_from(right_shift).map_err(|_| exponent_overflow())?;
    let right = magnitude.shl_bits(right_shift)?;
    let exponent_shift = exponent as usize;
    let one = BigUint::one()?;
    let mut lower = BigUint::zero();
    let mut upper = one.try_clone()?;
    while powered_scaled(&upper, degree, exponent_shift)? <= right {
        upper = upper.shl_bits(1)?;
    }
    while upper.checked_sub(&lower)? > one {
        let midpoint = lower.add(&upper)?.shr_bits(1)?;
        if powered_scaled(&midpoint, degree, exponent_shift)? <= right {
            lower = midpoint;
        } else {
            upper = midpoint;
        }
    }
    let exact = powered_scaled(&lower, degree, exponent_shift)? == right;
    Ok((lower, exact))
}

fn powered_scaled(value: &BigUint, degree: u32, shift: usize) -> Result<BigUint, AlgnumError> {
    Ok(value.pow_u32(degree)?.shl_bits(shift)?)
}

fn reciprocal_rational(value: &Dyadic) -> Result<ReducedRational, AlgnumError> {
    let rational = dyadic_rational(value)?;
    let one = ReducedRational::from_bigint(BigInt::one()?)?;
    Ok(one.div(&rational)?)
}

fn integer_form_sum(value: &BigInt) -> Result<FormSum, AlgnumError> {
    let mut result = FormSum::zero();
    let mut power = FormSum::one()?;
    let bit_len = value.magnitude().bit_len();
    for bit in 0..bit_len {
        if value.magnitude().bit(bit) {
            result = result.add(&power)?;
        }
        if bit + 1 < bit_len {
            power = power.add(&power)?;
        }
    }
    if value.sign() == Sign::Negative {
        Ok(FormSum::zero().sub(&result)?)
    } else {
        Ok(result)
    }
}

fn algebraic_one() -> Result<RealAlgebraic, AlgnumError> {
    RealAlgebraic::from_form_sum(&FormSum::one()?)
}

fn root_degree(value: &BigUint) -> Result<u32, AlgnumError> {
    if let Some(value) = value.to_u32() {
        return Ok(value);
    }
    Err(AlgnumError::RepresentationLimit {
        resource: crate::RepresentationResource::RootDegree,
        required: value.try_clone()?,
        maximum: BigUint::try_from(u32::MAX)?,
    })
}

fn biguint_usize(
    value: &BigUint,
    resource: crate::RepresentationResource,
) -> Result<usize, AlgnumError> {
    let maximum_usize = match resource {
        crate::RepresentationResource::PolynomialDegree => usize::MAX - 1,
        _ => usize::MAX,
    };
    let maximum = BigUint::try_from(maximum_usize)?;
    if value <= &maximum && value.bit_len() <= usize::BITS as usize {
        let mut converted = 0_usize;
        for index in 0..value.bit_len() {
            if value.bit(index) {
                converted |= 1_usize << index;
            }
        }
        return Ok(converted);
    }
    Err(AlgnumError::RepresentationLimit {
        resource,
        required: value.try_clone()?,
        maximum,
    })
}

fn enclosures_intersect(left: &DyadicEnclosure, right: &DyadicEnclosure) -> bool {
    left.lower() <= right.upper() && right.lower() <= left.upper()
}

fn opposite_sign(sign: Sign) -> Sign {
    match sign {
        Sign::Negative => Sign::Positive,
        Sign::Zero => Sign::Zero,
        Sign::Positive => Sign::Negative,
    }
}

impl CertifiedAlgebraic {
    pub fn value(&self) -> &RealAlgebraic {
        &self.value
    }

    pub fn enclosure(&self) -> &DyadicEnclosure {
        &self.enclosure
    }

    pub fn try_clone(&self) -> Result<Self, AlgnumError> {
        Ok(Self {
            value: self.value.try_clone()?,
            enclosure: self.enclosure.try_clone()?,
        })
    }

    pub fn into_value(self) -> RealAlgebraic {
        self.value
    }

    pub(crate) fn new(value: RealAlgebraic, enclosure: DyadicEnclosure) -> Self {
        Self { value, enclosure }
    }
}

#[cfg(test)]
mod tests {
    use alloc::{vec, vec::Vec};

    use crate::{AlgnumError, Polynomial, RealAlgebraic, RepresentationResource};
    use neco_bigint::{BigInt, Dyadic, DyadicEnclosure};
    use neco_formsum::FormSum;

    use super::{
        biguint_usize, binary_selection_precisions, candidate_root_visit_limit,
        enclosures_intersect, result_selection_precision, root_degree, visit_factors, visit_roots,
    };

    fn integer(value: i32) -> BigInt {
        BigInt::try_from(value).unwrap()
    }

    fn dyadic(value: i32) -> Dyadic {
        Dyadic::new(integer(value), 0)
    }

    fn positive_root(coefficients: &[i32], lower: i32, upper: i32) -> RealAlgebraic {
        let polynomial = Polynomial::from_coefficients(
            coefficients
                .iter()
                .copied()
                .map(integer)
                .collect::<Vec<_>>(),
        );
        let mut factors = polynomial
            .candidate()
            .unwrap()
            .square_free()
            .unwrap()
            .factor()
            .unwrap();
        factors
            .pop()
            .unwrap()
            .certify_root(dyadic(lower), dyadic(upper))
            .unwrap()
            .into_value()
    }

    fn coefficients(value: &RealAlgebraic) -> Vec<i32> {
        value
            .minimal_polynomial()
            .polynomial()
            .coefficients()
            .iter()
            .map(|coefficient| {
                let magnitude = coefficient.magnitude().to_u32().unwrap() as i32;
                match coefficient.sign() {
                    neco_bigint::Sign::Negative => -magnitude,
                    neco_bigint::Sign::Zero => 0,
                    neco_bigint::Sign::Positive => magnitude,
                }
            })
            .collect()
    }

    #[test]
    fn required_general_operation_vectors_have_canonical_results() {
        let sqrt_two = positive_root(&[-2, 0, 1], 1, 2);
        let sqrt_three = positive_root(&[-3, 0, 1], 1, 2);
        assert_eq!(coefficients(&sqrt_two.mul(&sqrt_two).unwrap()), vec![-2, 1]);
        let sum = sqrt_two.add(&sqrt_three).unwrap();
        assert_eq!(coefficients(&sum), vec![1, 0, -10, 0, 1]);
        assert_eq!(sum.root_index().get(), 3);
        assert_eq!(coefficients(&sqrt_two.sub(&sqrt_two).unwrap()), vec![0, 1]);
    }

    #[test]
    fn division_power_and_root_failures_are_distinct() {
        let sqrt_two = positive_root(&[-2, 0, 1], 1, 2);
        assert_eq!(coefficients(&sqrt_two.div(&sqrt_two).unwrap()), vec![-1, 1]);
        assert_eq!(sqrt_two.nth_root(0), Err(AlgnumError::ZeroRootDegree));
        let negative = positive_root(&[-2, 0, 1], -2, -1);
        assert_eq!(negative.nth_root(2), Err(AlgnumError::EvenRootOfNegative));
    }

    #[test]
    fn form_sum_conversion_and_equality_select_the_same_real_root() {
        let one = FormSum::one().unwrap();
        let algebraic = RealAlgebraic::from_form_sum(&one).unwrap();
        assert_eq!(coefficients(&algebraic), vec![-1, 1]);
        assert!(algebraic.equals_form_sum(&one).unwrap());
        assert!(!algebraic.equals_form_sum(&FormSum::zero()).unwrap());

        let two = one.add(&one).unwrap();
        let algebraic_two = positive_root(&[-2, 1], 1, 3);
        assert!(algebraic_two.equals_form_sum(&two).unwrap());
    }

    #[test]
    fn enclosure_intersection_includes_a_shared_endpoint() {
        let left = DyadicEnclosure::new(dyadic(0), dyadic(1)).unwrap();
        let touching = DyadicEnclosure::new(dyadic(1), dyadic(2)).unwrap();
        let disjoint = DyadicEnclosure::new(dyadic(2), dyadic(3)).unwrap();
        assert!(enclosures_intersect(&left, &touching));
        assert!(!enclosures_intersect(&left, &disjoint));
    }

    #[test]
    fn candidate_selection_refines_the_result_enclosure_at_each_precision() {
        assert_eq!(result_selection_precision(0), 0);
        assert_eq!(result_selection_precision(7), 7);
    }

    #[test]
    fn binary_candidate_selection_refines_both_inputs_at_each_precision() {
        assert_eq!(binary_selection_precisions(0), (0, 0));
        assert_eq!(binary_selection_precisions(7), (7, 7));
    }

    #[test]
    fn real_root_isolation_visits_factors_in_canonical_order() {
        let factors = Polynomial::from_coefficients(vec![
            integer(6),
            integer(0),
            integer(-5),
            integer(0),
            integer(1),
        ])
        .candidate()
        .unwrap()
        .square_free()
        .unwrap()
        .factor()
        .unwrap();
        let mut constants = Vec::new();
        visit_factors(factors, |factor| {
            constants.push(factor.polynomial().coefficients()[0].try_clone()?);
            Ok(())
        })
        .unwrap();
        assert_eq!(constants, vec![integer(-3), integer(-2)]);
    }

    #[test]
    fn candidate_selection_visits_roots_in_ascending_order() {
        assert_eq!(candidate_root_visit_limit(4), 4);
        let roots = Polynomial::from_coefficients(vec![
            integer(6),
            integer(0),
            integer(-5),
            integer(0),
            integer(1),
        ])
        .candidate()
        .unwrap()
        .square_free()
        .unwrap()
        .isolate_real_roots()
        .unwrap();
        let mut visited = Vec::new();
        visit_roots(&roots, |root| {
            visited.push((
                coefficients(root.value())[0],
                root.value().root_index().get(),
            ));
            Ok(())
        })
        .unwrap();
        assert_eq!(visited, vec![(-3, 0), (-2, 0), (-2, 1), (-3, 1)]);
    }

    #[test]
    fn polynomial_degree_rejects_usize_max_before_coefficient_count() {
        let required = neco_bigint::BigUint::try_from(usize::MAX).unwrap();
        let maximum = neco_bigint::BigUint::try_from(usize::MAX - 1).unwrap();
        assert_eq!(
            biguint_usize(&required, RepresentationResource::PolynomialDegree),
            Err(AlgnumError::RepresentationLimit {
                resource: RepresentationResource::PolynomialDegree,
                required,
                maximum,
            })
        );
    }

    #[test]
    fn root_degree_reports_the_exact_biguint_above_u32() {
        let maximum = neco_bigint::BigUint::try_from(u32::MAX).unwrap();
        let required = maximum.add(&neco_bigint::BigUint::one().unwrap()).unwrap();
        assert_eq!(
            root_degree(&required),
            Err(AlgnumError::RepresentationLimit {
                resource: RepresentationResource::RootDegree,
                required,
                maximum,
            })
        );
    }
}
