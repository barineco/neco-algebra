use core::ops::{Range, RangeInclusive};

use neco_bigint::{BigintError, Dyadic};
use neco_linear_dense::DenseMatrix;
use neco_linear_exact::CertifiedGeneralizedEigenInput;

use crate::bounds::next_up_nonnegative;
use crate::clusters::{absolute, complete_pairs, zero, ClusterError, Evaluation, SpectralClusters};
use crate::types::CertifiedEigenSolution;

/// A run of mode numbers whose subspace correspondence is certified.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockCorrespondence {
    ordinals: RangeInclusive<usize>,
    reference_clusters: Range<usize>,
    candidate_clusters: Range<usize>,
    distance_bound: f64,
}

impl BlockCorrespondence {
    /// The one-based mode numbers covered by the block.
    pub fn ordinals(&self) -> &RangeInclusive<usize> {
        &self.ordinals
    }

    /// The indices of the reference clusters inside the block.
    pub fn reference_clusters(&self) -> &Range<usize> {
        &self.reference_clusters
    }

    /// The indices of the candidate clusters inside the block.
    pub fn candidate_clusters(&self) -> &Range<usize> {
        &self.candidate_clusters
    }

    /// An upper bound, between 0 and 1, on the 2-norm of the difference of the
    /// two subspace projections in the reference pencil's mass inner product.
    /// The value 1 means the accuracy is not certified.
    pub fn distance_bound(&self) -> f64 {
        self.distance_bound
    }
}

/// The subspace correspondence between a candidate and a reference
/// evaluation, as blocks of consecutive mode numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct ModeCorrespondence {
    blocks: Vec<BlockCorrespondence>,
}

impl ModeCorrespondence {
    pub fn blocks(&self) -> &[BlockCorrespondence] {
        &self.blocks
    }

    /// The block holding the one-based mode number `ordinal`.
    pub fn block_of(&self, ordinal: usize) -> Option<&BlockCorrespondence> {
        self.blocks
            .iter()
            .find(|block| block.ordinals.contains(&ordinal))
    }
}

/// Bounds, per block of mode numbers, the distance between the candidate's
/// eigenvector span and the reference's eigenvector span, treating the
/// reference evaluation's exact pencil as the target.
pub fn correspond_to_reference(
    reference: &SpectralClusters<'_>,
    candidate: &CertifiedEigenSolution,
) -> Result<ModeCorrespondence, ClusterError> {
    let reference_input = reference.solution().projection_reference();
    let candidate_input = candidate.projection_reference();
    let reference_dimension = reference_input.stiffness().matrix().rows();
    let candidate_dimension = candidate_input.stiffness().matrix().rows();
    if reference_dimension != candidate_dimension {
        return Err(ClusterError::DimensionMismatch {
            reference: reference_dimension,
            candidate: candidate_dimension,
        });
    }
    let pairs = complete_pairs(candidate)?;
    let (eta_k, eta_m) = perturbation_between(reference_input, candidate_input)?;
    let evaluation = Evaluation::from_pairs(candidate, &pairs, &eta_k, &eta_m)?;
    let drafts = evaluation.drafts()?;

    let reference_ranges: Vec<Range<usize>> = reference
        .clusters()
        .iter()
        .map(|cluster| cluster.pair_range().clone())
        .collect();
    let candidate_ranges: Vec<Range<usize>> =
        drafts.iter().map(|draft| draft.range.clone()).collect();

    let mut blocks = Vec::new();
    for block in common_coarsening(&reference_ranges, &candidate_ranges) {
        let reference_angle = reference.evaluation.subspace_angle(
            block.clone(),
            reference
                .clusters()
                .iter()
                .map(|cluster| (cluster.pair_range(), cluster.interval())),
        )?;
        let candidate_angle = evaluation.subspace_angle(
            block.clone(),
            drafts
                .iter()
                .map(|draft| (&draft.range, &draft.evidence.interval)),
        )?;
        let distance_bound = distance_bound_of(reference_angle, candidate_angle)?;
        blocks.push(BlockCorrespondence {
            ordinals: block.start + 1..=block.end,
            reference_clusters: cluster_indices(&reference_ranges, &block),
            candidate_clusters: cluster_indices(&candidate_ranges, &block),
            distance_bound,
        });
    }
    Ok(ModeCorrespondence { blocks })
}

pub(crate) fn common_coarsening(
    reference: &[Range<usize>],
    candidate: &[Range<usize>],
) -> Vec<Range<usize>> {
    let mut blocks = Vec::new();
    let mut start = match reference.first() {
        Some(first) => first.start,
        None => return blocks,
    };
    for range in reference {
        let cut = range.end;
        if candidate.iter().any(|other| other.end == cut) {
            blocks.push(start..cut);
            start = cut;
        }
    }
    blocks
}

pub(crate) fn perturbation_between(
    reference: &CertifiedGeneralizedEigenInput,
    candidate: &CertifiedGeneralizedEigenInput,
) -> Result<(Dyadic, Dyadic), BigintError> {
    let eta_k = reference
        .stiffness()
        .absolute_error_bound()
        .add(&absolute_difference_sum(
            reference.stiffness().matrix(),
            candidate.stiffness().matrix(),
        )?)?;
    let eta_m = reference
        .mass()
        .absolute_error_bound()
        .add(&absolute_difference_sum(
            reference.mass().matrix().matrix(),
            candidate.mass().matrix().matrix(),
        )?)?;
    Ok((eta_k, eta_m))
}

fn absolute_difference_sum(
    reference: &DenseMatrix<f64>,
    candidate: &DenseMatrix<f64>,
) -> Result<Dyadic, BigintError> {
    let mut sum = zero()?;
    for (left, right) in reference.values().iter().zip(candidate.values()) {
        let difference = Dyadic::from_f64_exact(*left)?.sub(&Dyadic::from_f64_exact(*right)?)?;
        sum = sum.add(&absolute(&difference)?)?;
    }
    Ok(sum)
}

fn distance_bound_of(reference_angle: f64, candidate_angle: f64) -> Result<f64, BigintError> {
    let sum =
        Dyadic::from_f64_exact(reference_angle)?.add(&Dyadic::from_f64_exact(candidate_angle)?)?;
    Ok(round_up_to_f64(&sum)?.min(1.0))
}

fn round_up_to_f64(value: &Dyadic) -> Result<f64, BigintError> {
    let nearest = value.round_to_f64_ties_even()?;
    if Dyadic::from_f64_exact(nearest)? < *value {
        Ok(next_up_nonnegative(nearest))
    } else {
        Ok(nearest)
    }
}

fn cluster_indices(ranges: &[Range<usize>], block: &Range<usize>) -> Range<usize> {
    let first = ranges.partition_point(|range| range.end <= block.start);
    let last = ranges.partition_point(|range| range.end <= block.end);
    first..last
}

#[cfg(test)]
mod tests {
    use super::{cluster_indices, common_coarsening, distance_bound_of};
    use neco_bigint::Dyadic;

    fn dyadic(value: f64) -> Dyadic {
        Dyadic::from_f64_exact(value).expect("finite value")
    }

    #[test]
    fn distance_bound_rounds_the_exact_sum_upward() {
        let bound = distance_bound_of(0.1, 0.7).expect("bound");
        let exact = dyadic(0.1).add(&dyadic(0.7)).expect("sum");
        let below = f64::from_bits(bound.to_bits() - 1);

        assert!(dyadic(bound) >= exact);
        assert!(dyadic(below) < exact);
        assert_eq!(distance_bound_of(0.25, 0.5), Ok(0.75));
    }

    #[test]
    fn distance_bound_saturates_at_one() {
        assert_eq!(distance_bound_of(0.6, 0.6), Ok(1.0));
        assert_eq!(distance_bound_of(0.0, 0.0), Ok(0.0));
    }

    #[test]
    fn cluster_indices_select_the_clusters_inside_a_block() {
        let ranges = [0..1, 1..3, 3..4];
        assert_eq!(cluster_indices(&ranges, &(0..1)), 0..1);
        assert_eq!(cluster_indices(&ranges, &(1..4)), 1..3);
        assert_eq!(cluster_indices(&ranges, &(3..4)), 2..3);
        assert_eq!(cluster_indices(&ranges, &(0..4)), 0..3);
    }

    #[test]
    fn common_coarsening_keeps_only_boundaries_present_in_both_partitions() {
        assert_eq!(
            common_coarsening(&[0..1, 1..2, 2..3], &[0..2, 2..3]),
            vec![0..2, 2..3]
        );
        assert_eq!(common_coarsening(&[0..1, 1..3], &[0..2, 2..3]), vec![0..3]);
    }
}
