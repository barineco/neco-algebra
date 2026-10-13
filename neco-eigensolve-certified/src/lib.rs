#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod bounds;
mod clusters;
mod correspondence;
mod frequency;
mod solve;
mod types;

pub use clusters::{
    certify_spectral_clusters, ClusterError, ModeOrdinal, SpectralCluster, SpectralClusters,
};
pub use correspondence::{correspond_to_reference, BlockCorrespondence, ModeCorrespondence};
pub use solve::solve_certified_generalized_eigen;
pub use types::{
    CertifiedEigenError, CertifiedEigenSolution, CertifiedEigenpair, CertifiedEigenspace,
    CertifiedFrequency, NonConvergentEigenpair, SolverFailure, UndeterminableEigenpair,
    UndeterminableReason,
};
