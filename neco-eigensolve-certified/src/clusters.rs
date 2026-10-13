use core::cmp::Ordering;
use core::fmt;
use core::ops::{Range, RangeInclusive};

use neco_bigint::{BigintError, Dyadic, DyadicEnclosure};
use neco_linear_dense::DenseMatrix;
use neco_linear_types::{LinearError, Vector};

use crate::bounds::{
    certify_linear_bound, certify_quadratic_bound, dyadic_dot, dyadic_from_f64_slice,
    dyadic_matrix_vector_product,
};
use crate::types::{CertifiedEigenSolution, CertifiedEigenpair};

/// The mode number a certified eigenpair can claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModeOrdinal {
    /// The one-based position among the exact ascending eigenvalues.
    Certified(usize),
    /// Only known to be one of the positions `first..=last`.
    WithinCluster { first: usize, last: usize },
}

/// Typed failures of the spectral cluster and correspondence functions.
#[derive(Debug, PartialEq)]
pub enum ClusterError {
    IncompleteSpectrum { returned: usize, dimension: usize },
    DimensionMismatch { reference: usize, candidate: usize },
    GramNotPositive { pair_range: Range<usize> },
    MassPerturbationTooLarge,
    ExactArithmetic(BigintError),
    Allocation(LinearError),
}

impl fmt::Display for ClusterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncompleteSpectrum {
                returned,
                dimension,
            } => write!(
                formatter,
                "{returned} eigenpair(s) returned for dimension {dimension}"
            ),
            Self::DimensionMismatch {
                reference,
                candidate,
            } => write!(
                formatter,
                "reference dimension {reference} differs from candidate dimension {candidate}"
            ),
            Self::GramNotPositive { pair_range } => write!(
                formatter,
                "no positive Gram lower bound for eigenpairs {}..{}",
                pair_range.start, pair_range.end
            ),
            Self::MassPerturbationTooLarge => {
                write!(formatter, "mass perturbation exceeds the mass lower bound")
            }
            Self::ExactArithmetic(error) => error.fmt(formatter),
            Self::Allocation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ClusterError {}

impl From<BigintError> for ClusterError {
    fn from(error: BigintError) -> Self {
        Self::ExactArithmetic(error)
    }
}

impl From<LinearError> for ClusterError {
    fn from(error: LinearError) -> Self {
        Self::Allocation(error)
    }
}

/// A run of adjacent eigenpairs whose enclosure is disjoint from every other
/// cluster's enclosure, together with its certified spectral facts.
#[derive(Debug, PartialEq)]
pub struct SpectralCluster {
    pub(crate) ordinals: RangeInclusive<usize>,
    pub(crate) pair_range: Range<usize>,
    pub(crate) interval: DyadicEnclosure,
    pub(crate) radius: Dyadic,
    pub(crate) separation: Option<Dyadic>,
    pub(crate) condition_bound: Option<f64>,
    pub(crate) subspace_angle_bound: f64,
}

impl SpectralCluster {
    /// The one-based positions among the exact ascending eigenvalues.
    pub fn ordinals(&self) -> &RangeInclusive<usize> {
        &self.ordinals
    }

    /// The indices of the cluster's eigenpairs in ascending eigenvalue order.
    pub fn pair_range(&self) -> &Range<usize> {
        &self.pair_range
    }

    /// An interval holding exactly `multiplicity()` exact eigenvalues.
    pub fn interval(&self) -> &DyadicEnclosure {
        &self.interval
    }

    pub fn radius(&self) -> &Dyadic {
        &self.radius
    }

    /// A lower bound on the distance to every exact eigenvalue outside the
    /// cluster, or `None` when the cluster holds the whole spectrum.
    pub fn separation(&self) -> Option<&Dyadic> {
        self.separation.as_ref()
    }

    /// An upper bound on the reciprocal of `separation`.
    pub fn condition_bound(&self) -> Option<f64> {
        self.condition_bound
    }

    /// An upper bound, between 0 and 1, on the sine of the largest principal
    /// angle to the exact eigenspace, measured in the mass inner product of
    /// the target pencil. The value 1 means the accuracy is not certified.
    pub fn subspace_angle_bound(&self) -> f64 {
        self.subspace_angle_bound
    }

    pub fn multiplicity(&self) -> usize {
        self.pair_range.len()
    }
}

/// The partition of a complete spectrum into certified clusters.
#[derive(Debug)]
pub struct SpectralClusters<'a> {
    pub(crate) solution: &'a CertifiedEigenSolution,
    pub(crate) clusters: Vec<SpectralCluster>,
    pub(crate) evaluation: Evaluation,
}

impl<'a> SpectralClusters<'a> {
    pub fn clusters(&self) -> &[SpectralCluster] {
        &self.clusters
    }

    pub fn solution(&self) -> &'a CertifiedEigenSolution {
        self.solution
    }

    /// The eigenpair at `index` in ascending eigenvalue order.
    pub fn pair(&self, index: usize) -> Option<&'a CertifiedEigenpair> {
        self.solution
            .eigenspaces()
            .iter()
            .flat_map(|space| space.basis())
            .nth(index)
    }

    /// The mode number claim for the eigenpair at `pair_index`: certified only
    /// when the pair is alone in its cluster.
    pub fn mode_ordinal(&self, pair_index: usize) -> Option<ModeOrdinal> {
        let cluster = self
            .clusters
            .iter()
            .find(|cluster| cluster.pair_range.contains(&pair_index))?;
        if cluster.multiplicity() == 1 {
            Some(ModeOrdinal::Certified(*cluster.ordinals.start()))
        } else {
            Some(ModeOrdinal::WithinCluster {
                first: *cluster.ordinals.start(),
                last: *cluster.ordinals.end(),
            })
        }
    }
}

/// Partitions a complete spectrum into clusters whose intervals are disjoint,
/// assigning exact eigenvalue counts and mode numbers per cluster.
pub fn certify_spectral_clusters(
    solution: &CertifiedEigenSolution,
) -> Result<SpectralClusters<'_>, ClusterError> {
    let pairs = complete_pairs(solution)?;
    let input = solution.projection_reference();
    let evaluation = Evaluation::from_pairs(
        solution,
        &pairs,
        input.stiffness().absolute_error_bound(),
        input.mass().absolute_error_bound(),
    )?;
    let clusters = evaluation.clusters()?;
    Ok(SpectralClusters {
        solution,
        clusters,
        evaluation,
    })
}

pub(crate) fn complete_pairs(
    solution: &CertifiedEigenSolution,
) -> Result<Vec<&CertifiedEigenpair>, ClusterError> {
    let dimension = solution.projection_reference().stiffness().matrix().rows();
    let pairs: Vec<&CertifiedEigenpair> = solution
        .eigenspaces()
        .iter()
        .flat_map(|space| space.basis())
        .collect();
    if pairs.len() != dimension {
        return Err(ClusterError::IncompleteSpectrum {
            returned: pairs.len(),
            dimension,
        });
    }
    Ok(pairs)
}

pub(crate) struct PairEvidence {
    mu: Dyadic,
    x: Vec<Dyadic>,
    mx: Vec<Dyadic>,
    q: Dyadic,
    nu: Dyadic,
    d: Dyadic,
}

impl PairEvidence {
    pub(crate) fn from_f64(
        mu: f64,
        x: &Vector<f64>,
        mass: &DenseMatrix<f64>,
        d: Dyadic,
    ) -> Result<Self, BigintError> {
        let x = dyadic_from_f64_slice(x.values())?;
        let mx = dyadic_matrix_vector_product(mass, x.len(), &x)?;
        Ok(Self {
            mu: Dyadic::from_f64_exact(mu)?,
            q: dyadic_dot(&x, &x)?,
            nu: dyadic_dot(&x, &mx)?,
            x,
            mx,
            d,
        })
    }
}

pub(crate) fn gram(pairs: &[PairEvidence]) -> Result<Vec<Dyadic>, ClusterError> {
    let count = pairs
        .len()
        .checked_mul(pairs.len())
        .ok_or(LinearError::CapacityOverflow {
            requested: pairs.len(),
        })?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| LinearError::AllocationFailure { requested: count })?;
    for row in pairs {
        for column in pairs {
            values.push(dyadic_dot(&row.x, &column.mx)?);
        }
    }
    Ok(values)
}

#[derive(Debug, PartialEq)]
pub(crate) struct GramBounds {
    pub(crate) lower: Dyadic,
    pub(crate) upper: Dyadic,
}

pub(crate) fn gram_bounds(
    gram: &[Dyadic],
    dimension: usize,
    range: Range<usize>,
    x2: &Dyadic,
    eta_m: &Dyadic,
) -> Result<GramBounds, ClusterError> {
    let mut lower: Option<Dyadic> = None;
    let mut upper: Option<Dyadic> = None;
    for row in range.clone() {
        let mut off_diagonal_sum = zero()?;
        for column in range.clone() {
            if column != row {
                off_diagonal_sum =
                    off_diagonal_sum.add(&absolute(&gram[row * dimension + column])?)?;
            }
        }
        let diagonal = &gram[row * dimension + row];
        lower = Some(smaller(lower, diagonal.sub(&off_diagonal_sum)?));
        upper = Some(larger(upper, diagonal.add(&off_diagonal_sum)?));
    }
    let (Some(lower), Some(upper)) = (lower, upper) else {
        return Err(ClusterError::GramNotPositive { pair_range: range });
    };
    let correction = eta_m.mul(x2)?;
    let lower = lower.sub(&correction)?;
    if lower <= zero()? {
        return Err(ClusterError::GramNotPositive { pair_range: range });
    }
    Ok(GramBounds {
        lower,
        upper: upper.add(&correction)?,
    })
}

pub(crate) fn pair_scale(
    sigma: &Dyadic,
    eta_k: &Dyadic,
    eta_m: &Dyadic,
    pair: &PairEvidence,
) -> Result<Dyadic, BigintError> {
    let sigma_e = sigma.sub(eta_m)?;
    let computational_rhs = sigma.mul(&pair.d.mul(&pair.d)?)?.mul(&pair.nu)?;
    let computational = certify_quadratic_bound(&computational_rhs, &sigma_e)?;
    let coefficient = eta_k.add(&absolute(&pair.mu)?.mul(eta_m)?)?;
    let perturbation_rhs = coefficient.mul(&coefficient)?.mul(&pair.q)?;
    let perturbation = certify_quadratic_bound(&perturbation_rhs, &sigma_e)?;
    computational.add(&perturbation)
}

pub(crate) fn cluster_radius(
    mus: &[Dyadic],
    scales: &[Dyadic],
    bounds: &GramBounds,
) -> Result<Dyadic, BigintError> {
    let (minimum, maximum) = extremes(mus)?;
    let rho = maximum.sub(&minimum)?.mul(&Dyadic::from_f64_exact(0.5)?)?;
    let one = Dyadic::from_f64_exact(1.0)?;
    let h = certify_quadratic_bound(&rho.mul(&rho)?.mul(&bounds.upper)?, &one)?;
    let mut scale_squares = zero()?;
    for scale in scales {
        scale_squares = scale_squares.add(&scale.mul(scale)?)?;
    }
    let w = certify_quadratic_bound(&scale_squares, &one)?;
    let spread = h.add(&w)?;
    certify_quadratic_bound(&spread.mul(&spread)?, &bounds.lower)
}

pub(crate) fn merge_until_disjoint(
    count: usize,
    mut evaluate: impl FnMut(Range<usize>) -> Result<DyadicEnclosure, ClusterError>,
) -> Result<Vec<Range<usize>>, ClusterError> {
    let mut clusters: Vec<(Range<usize>, DyadicEnclosure)> = Vec::with_capacity(count);
    for index in 0..count {
        clusters.push((index..index + 1, evaluate(index..index + 1)?));
    }
    let mut index = 0;
    while index + 1 < clusters.len() {
        if clusters[index].1.upper() >= clusters[index + 1].1.lower() {
            let merged = clusters[index].0.start..clusters[index + 1].0.end;
            let enclosure = evaluate(merged.clone())?;
            clusters.remove(index + 1);
            clusters[index] = (merged, enclosure);
            index = index.saturating_sub(1);
        } else {
            index += 1;
        }
    }
    Ok(clusters.into_iter().map(|(range, _)| range).collect())
}

pub(crate) fn angle_bound(beta: &Dyadic, distance: &Dyadic) -> Result<f64, BigintError> {
    if *beta >= *distance {
        return Ok(1.0);
    }
    let bound = certify_linear_bound(beta, distance)?.round_to_f64_ties_even()?;
    Ok(bound.min(1.0))
}

pub(crate) struct RangeEvidence {
    pub(crate) center: Dyadic,
    pub(crate) radius: Dyadic,
    pub(crate) interval: DyadicEnclosure,
}

pub(crate) struct ClusterDraft {
    pub(crate) range: Range<usize>,
    pub(crate) evidence: RangeEvidence,
}

#[derive(Debug)]
pub(crate) struct Evaluation {
    mus: Vec<Dyadic>,
    qs: Vec<Dyadic>,
    scales: Vec<Dyadic>,
    gram: Vec<Dyadic>,
    eta_m: Dyadic,
}

impl Evaluation {
    pub(crate) fn from_pairs(
        solution: &CertifiedEigenSolution,
        pairs: &[&CertifiedEigenpair],
        eta_k: &Dyadic,
        eta_m: &Dyadic,
    ) -> Result<Self, ClusterError> {
        let sigma = solution.mass_lower_bound();
        if sigma.sub(eta_m)? <= zero()? {
            return Err(ClusterError::MassPerturbationTooLarge);
        }
        let mass = solution.projection_reference().mass().matrix().matrix();
        let mut evidence = Vec::with_capacity(pairs.len());
        for pair in pairs {
            evidence.push(PairEvidence::from_f64(
                pair.eigenvalue(),
                pair.eigenvector(),
                mass,
                pair.computational_bound().try_clone()?,
            )?);
        }
        let gram = gram(&evidence)?;
        let mut scales = Vec::with_capacity(evidence.len());
        for pair in &evidence {
            scales.push(pair_scale(sigma, eta_k, eta_m, pair)?);
        }
        Ok(Self {
            mus: evidence
                .iter()
                .map(|pair| pair.mu.try_clone())
                .collect::<Result<_, _>>()?,
            qs: evidence
                .iter()
                .map(|pair| pair.q.try_clone())
                .collect::<Result<_, _>>()?,
            scales,
            gram,
            eta_m: eta_m.try_clone()?,
        })
    }

    pub(crate) fn evaluate(&self, range: Range<usize>) -> Result<RangeEvidence, ClusterError> {
        let mut x2 = zero()?;
        for q in &self.qs[range.clone()] {
            x2 = x2.add(q)?;
        }
        let mus = &self.mus[range.clone()];
        let bounds = gram_bounds(&self.gram, self.mus.len(), range.clone(), &x2, &self.eta_m)?;
        let radius = cluster_radius(mus, &self.scales[range], &bounds)?;
        let (minimum, maximum) = extremes(mus)?;
        let center = minimum.midpoint(&maximum)?;
        let interval = DyadicEnclosure::new(center.sub(&radius)?, center.add(&radius)?)?;
        Ok(RangeEvidence {
            center,
            radius,
            interval,
        })
    }

    pub(crate) fn drafts(&self) -> Result<Vec<ClusterDraft>, ClusterError> {
        let ranges =
            merge_until_disjoint(self.mus.len(), |range| Ok(self.evaluate(range)?.interval))?;
        let mut drafts = Vec::with_capacity(ranges.len());
        for range in ranges {
            let evidence = self.evaluate(range.clone())?;
            drafts.push(ClusterDraft { range, evidence });
        }
        Ok(drafts)
    }

    pub(crate) fn clusters(&self) -> Result<Vec<SpectralCluster>, ClusterError> {
        let drafts = self.drafts()?;
        let one = Dyadic::from_f64_exact(1.0)?;
        let mut clusters = Vec::with_capacity(drafts.len());
        for (index, draft) in drafts.iter().enumerate() {
            let interval = &draft.evidence.interval;
            let mut separation: Option<Dyadic> = None;
            if let Some(next) = drafts.get(index + 1) {
                let gap = next.evidence.interval.lower().sub(interval.upper())?;
                separation = Some(smaller(separation, gap));
            }
            if let Some(previous) = index
                .checked_sub(1)
                .and_then(|previous| drafts.get(previous))
            {
                let gap = interval.lower().sub(previous.evidence.interval.upper())?;
                separation = Some(smaller(separation, gap));
            }
            let condition_bound = match &separation {
                Some(separation) => {
                    Some(certify_linear_bound(&one, separation)?.round_to_f64_ties_even()?)
                }
                None => None,
            };
            let subspace_angle_bound = self.subspace_angle(
                draft.range.clone(),
                drafts
                    .iter()
                    .map(|other| (&other.range, &other.evidence.interval)),
            )?;
            clusters.push(SpectralCluster {
                ordinals: draft.range.start + 1..=draft.range.end,
                pair_range: draft.range.clone(),
                interval: interval.try_clone()?,
                radius: draft.evidence.radius.try_clone()?,
                separation,
                condition_bound,
                subspace_angle_bound,
            });
        }
        Ok(clusters)
    }

    pub(crate) fn subspace_angle<'a>(
        &self,
        block: Range<usize>,
        clusters: impl Iterator<Item = (&'a Range<usize>, &'a DyadicEnclosure)>,
    ) -> Result<f64, ClusterError> {
        if block.start == 0 && block.end == self.mus.len() {
            return Ok(0.0);
        }
        let evidence = self.evaluate(block.clone())?;
        let mut nearest: Option<Dyadic> = None;
        for (range, interval) in clusters {
            if range.end <= block.start || range.start >= block.end {
                let distance = distance_to_interval(&evidence.center, interval)?;
                nearest = Some(smaller(nearest, distance));
            }
        }
        match nearest {
            None => Ok(0.0),
            Some(distance) if distance <= zero()? => Ok(1.0),
            Some(distance) => Ok(angle_bound(&evidence.radius, &distance)?),
        }
    }
}

pub(crate) fn zero() -> Result<Dyadic, BigintError> {
    Dyadic::from_f64_exact(0.0)
}

pub(crate) fn absolute(value: &Dyadic) -> Result<Dyadic, BigintError> {
    if *value < zero()? {
        zero()?.sub(value)
    } else {
        value.try_clone()
    }
}

fn smaller(current: Option<Dyadic>, candidate: Dyadic) -> Dyadic {
    match current {
        Some(current) if current.cmp(&candidate) != Ordering::Greater => current,
        _ => candidate,
    }
}

fn larger(current: Option<Dyadic>, candidate: Dyadic) -> Dyadic {
    match current {
        Some(current) if current.cmp(&candidate) != Ordering::Less => current,
        _ => candidate,
    }
}

fn extremes(values: &[Dyadic]) -> Result<(Dyadic, Dyadic), BigintError> {
    let mut minimum: Option<Dyadic> = None;
    let mut maximum: Option<Dyadic> = None;
    for value in values {
        minimum = Some(smaller(minimum, value.try_clone()?));
        maximum = Some(larger(maximum, value.try_clone()?));
    }
    match (minimum, maximum) {
        (Some(minimum), Some(maximum)) => Ok((minimum, maximum)),
        _ => Ok((zero()?, zero()?)),
    }
}

fn distance_to_interval(point: &Dyadic, interval: &DyadicEnclosure) -> Result<Dyadic, BigintError> {
    if point < interval.lower() {
        interval.lower().sub(point)
    } else if point > interval.upper() {
        point.sub(interval.upper())
    } else {
        zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neco_linear_types::Shape;

    fn dyadic(value: f64) -> Dyadic {
        Dyadic::from_f64_exact(value).expect("finite value")
    }

    fn vector(values: Vec<f64>) -> Vector<f64> {
        Vector::try_from_vec(values).expect("vector")
    }

    fn dense(dimension: usize, row_major: Vec<f64>) -> DenseMatrix<f64> {
        DenseMatrix::from_row_major(Shape::new(dimension, dimension), row_major).expect("matrix")
    }

    fn identity_evidence(mu: f64, x: Vec<f64>, d: f64) -> PairEvidence {
        let dimension = x.len();
        let mut identity = vec![0.0; dimension * dimension];
        for index in 0..dimension {
            identity[index * dimension + index] = 1.0;
        }
        PairEvidence::from_f64(mu, &vector(x), &dense(dimension, identity), dyadic(d))
            .expect("evidence")
    }

    #[test]
    fn gram_bounds_use_gershgorin_and_projection_correction() {
        let pairs = vec![
            identity_evidence(1.0, vec![1.0, 0.0], 0.0),
            identity_evidence(2.0, vec![0.5, 1.0], 0.0),
        ];
        let gram = gram(&pairs).expect("gram");

        let both = gram_bounds(&gram, 2, 0..2, &dyadic(2.25), &dyadic(0.125)).expect("bounds");
        assert_eq!(both.lower, dyadic(0.21875));
        assert_eq!(both.upper, dyadic(2.03125));

        let first = gram_bounds(&gram, 2, 0..1, &dyadic(1.0), &dyadic(0.125)).expect("bounds");
        assert_eq!(first.lower, dyadic(0.875));
        assert_eq!(first.upper, dyadic(1.125));
    }

    #[test]
    fn gram_bounds_rejects_a_dependent_basis() {
        let pairs = vec![
            identity_evidence(1.0, vec![1.0, 0.0], 0.0),
            identity_evidence(1.0, vec![1.0, 0.0], 0.0),
        ];
        let gram = gram(&pairs).expect("gram");

        assert_eq!(
            gram_bounds(&gram, 2, 0..2, &dyadic(2.0), &dyadic(0.0)),
            Err(ClusterError::GramNotPositive { pair_range: 0..2 })
        );
        assert!(gram_bounds(&gram, 2, 0..1, &dyadic(1.0), &dyadic(0.0)).is_ok());
    }

    #[test]
    fn pair_scale_matches_hand_computation() {
        let pair = identity_evidence(2.0, vec![1.0, 0.0], 0.25);
        let scale = pair_scale(&dyadic(1.0), &dyadic(0.5), &dyadic(0.75), &pair).expect("scale");
        assert_eq!(scale, dyadic(4.5));

        let exact = pair_scale(&dyadic(1.0), &dyadic(0.0), &dyadic(0.0), &pair).expect("scale");
        assert_eq!(exact, dyadic(0.25));
    }

    #[test]
    fn cluster_radius_combines_spread_residual_and_gram_spread() {
        let bounds = |lower: f64, upper: f64| GramBounds {
            lower: dyadic(lower),
            upper: dyadic(upper),
        };
        let radius = |mus: &[f64], scales: &[f64], bounds: GramBounds| {
            let mus: Vec<Dyadic> = mus.iter().map(|value| dyadic(*value)).collect();
            let scales: Vec<Dyadic> = scales.iter().map(|value| dyadic(*value)).collect();
            cluster_radius(&mus, &scales, &bounds).expect("radius")
        };

        assert_eq!(
            radius(&[1.0, 3.0], &[0.75, 1.0], bounds(1.0, 1.0)),
            dyadic(2.25)
        );
        assert_eq!(
            radius(&[1.0, 3.0], &[0.0, 0.0], bounds(1.0, 4.0)),
            dyadic(2.0)
        );
        assert_eq!(radius(&[2.0], &[0.5], bounds(0.25, 1.0)), dyadic(1.0));
        assert_eq!(radius(&[2.0], &[0.25], bounds(1.0, 1.0)), dyadic(0.25));
    }

    fn pseudo_evaluation(
        mus: &'static [f64],
    ) -> impl FnMut(Range<usize>) -> Result<DyadicEnclosure, ClusterError> {
        move |range: Range<usize>| {
            let members = &mus[range];
            let minimum = members.iter().copied().fold(f64::INFINITY, f64::min);
            let maximum = members.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let radius = 0.5 + 0.25 * (members.len() - 1) as f64 + (maximum - minimum) / 2.0;
            let center = (minimum + maximum) / 2.0;
            Ok(DyadicEnclosure::new(
                dyadic(center - radius),
                dyadic(center + radius),
            )?)
        }
    }

    #[test]
    fn merge_cascades_until_intervals_are_disjoint() {
        let merged = merge_until_disjoint(3, pseudo_evaluation(&[0.0, 0.75, 2.0])).expect("merge");
        assert_eq!(merged, vec![0..3]);
    }

    #[test]
    fn merge_stops_when_intervals_do_not_touch() {
        let merged = merge_until_disjoint(3, pseudo_evaluation(&[0.0, 0.75, 2.5])).expect("merge");
        assert_eq!(merged, vec![0..2, 2..3]);
    }

    #[test]
    fn angle_bound_saturates_at_one() {
        assert_eq!(angle_bound(&dyadic(1.5), &dyadic(1.0)), Ok(1.0));
        assert_eq!(angle_bound(&dyadic(0.5), &dyadic(2.0)), Ok(0.25));
        assert_eq!(angle_bound(&dyadic(0.0), &dyadic(3.0)), Ok(0.0));
    }

    fn assert_least_quotient(bound: f64, numerator: f64, denominator: f64) {
        let denominator = dyadic(denominator);
        let numerator = dyadic(numerator);
        let below = f64::from_bits(bound.to_bits() - 1);
        assert!(dyadic(bound).mul(&denominator).expect("product") >= numerator);
        assert!(dyadic(below).mul(&denominator).expect("product") < numerator);
    }

    fn point_evaluation(mus: &[f64], scale: f64) -> Evaluation {
        let count = mus.len();
        let mut gram = Vec::with_capacity(count * count);
        for row in 0..count {
            for column in 0..count {
                gram.push(dyadic(if row == column { 1.0 } else { 0.0 }));
            }
        }
        Evaluation {
            mus: mus.iter().map(|mu| dyadic(*mu)).collect(),
            qs: mus.iter().map(|_| dyadic(1.0)).collect(),
            scales: mus.iter().map(|_| dyadic(scale)).collect(),
            gram,
            eta_m: dyadic(0.0),
        }
    }

    #[test]
    fn angle_bound_is_the_least_certified_quotient() {
        let bound = angle_bound(&dyadic(1.0), &dyadic(3.0)).expect("bound");
        assert_least_quotient(bound, 1.0, 3.0);
    }

    #[test]
    fn gram_bounds_use_absolute_off_diagonal_entries() {
        let pairs = vec![
            identity_evidence(1.0, vec![1.0, 0.0], 0.0),
            identity_evidence(2.0, vec![-0.5, 1.0], 0.0),
        ];
        let gram = gram(&pairs).expect("gram");

        let bounds = gram_bounds(&gram, 2, 0..2, &dyadic(2.25), &dyadic(0.125)).expect("bounds");
        assert_eq!(bounds.lower, dyadic(0.21875));
        assert_eq!(bounds.upper, dyadic(2.03125));
    }

    #[test]
    fn merge_restarts_from_the_previous_cluster_after_a_merge() {
        let merged = merge_until_disjoint(3, pseudo_evaluation(&[0.0, 1.2, 1.9])).expect("merge");
        assert_eq!(merged, vec![0..3]);
    }

    #[test]
    fn separation_and_angle_use_the_nearest_neighbour_of_each_cluster() {
        let clusters = point_evaluation(&[1.0, 3.0, 10.0], 0.25)
            .clusters()
            .expect("clusters");
        assert_eq!(clusters.len(), 3);

        let separations = [1.5, 1.5, 6.5];
        let angle_distances = [1.75, 1.75, 6.75];
        for ((cluster, separation), distance) in
            clusters.iter().zip(separations).zip(angle_distances)
        {
            assert_eq!(cluster.radius, dyadic(0.25));
            assert_eq!(cluster.separation, Some(dyadic(separation)));
            assert_least_quotient(
                cluster.condition_bound.expect("condition bound"),
                1.0,
                separation,
            );
            assert_least_quotient(cluster.subspace_angle_bound, 0.25, distance);
        }
    }

    #[test]
    fn condition_bound_exceeds_one_for_a_separation_below_one() {
        let clusters = point_evaluation(&[1.0, 1.5], 0.0)
            .clusters()
            .expect("clusters");
        assert_eq!(clusters.len(), 2);
        for cluster in &clusters {
            assert_eq!(cluster.separation, Some(dyadic(0.5)));
            assert_eq!(cluster.condition_bound, Some(2.0));
            assert_eq!(cluster.subspace_angle_bound, 0.0);
        }
    }

    #[test]
    fn a_single_merged_cluster_has_no_separation_and_a_zero_angle() {
        let clusters = point_evaluation(&[1.0, 1.25], 0.25)
            .clusters()
            .expect("clusters");
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].pair_range, 0..2);
        assert_eq!(clusters[0].separation, None);
        assert_eq!(clusters[0].condition_bound, None);
        assert_eq!(clusters[0].subspace_angle_bound, 0.0);
    }

    #[test]
    fn sign_and_order_of_basis_vectors_do_not_change_cluster_evidence() {
        let mass = dense(2, vec![2.0, 0.0, 0.0, 3.0]);
        let evidence = |mu: f64, x: Vec<f64>, d: f64| {
            PairEvidence::from_f64(mu, &vector(x), &mass, dyadic(d)).expect("evidence")
        };
        let original = vec![
            evidence(1.0, vec![0.5, 0.0], 0.125),
            evidence(2.0, vec![0.0, 0.25], 0.0625),
        ];
        let flipped_and_swapped = vec![
            evidence(2.0, vec![0.0, -0.25], 0.0625),
            evidence(1.0, vec![0.5, 0.0], 0.125),
        ];

        let summarize = |pairs: &[PairEvidence]| {
            let gram = gram(pairs).expect("gram");
            let x2 = pairs
                .iter()
                .try_fold(dyadic(0.0), |sum, pair| sum.add(&pair.q))
                .expect("x2");
            let eta_m = dyadic(0.25);
            let bounds = gram_bounds(&gram, 2, 0..2, &x2, &eta_m).expect("bounds");
            let mut scales: Vec<Dyadic> = pairs
                .iter()
                .map(|pair| pair_scale(&dyadic(2.0), &dyadic(0.125), &eta_m, pair).expect("scale"))
                .collect();
            let mus: Vec<Dyadic> = pairs
                .iter()
                .map(|pair| pair.mu.try_clone().expect("mu"))
                .collect();
            let radius = cluster_radius(&mus, &scales, &bounds).expect("radius");
            scales.sort();
            (bounds, scales, radius)
        };

        let before = summarize(&original);
        let after = summarize(&flipped_and_swapped);
        assert_eq!(before.0.lower, dyadic(0.109375));
        assert_eq!(before.0.upper, dyadic(0.578125));
        assert_eq!(before, after);
    }
}
