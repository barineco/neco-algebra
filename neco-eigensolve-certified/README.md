# neco-eigensolve-certified

[日本語](README-ja.md)

`neco-eigensolve-certified` computes, for each eigenpair of a generalized eigenvalue problem, a certified upper bound on the forward error and a certified frequency interval.
It delegates the numerical computation to one of two solvers in the same repository, selected by the input's `MassNormalizationMethod`.

- `Identity`: treats the mass matrix as the identity matrix and delegates to `neco-eigensolve`
- `PositiveDefiniteCholesky`: treats the mass matrix as positive definite, factors it by Cholesky decomposition, and delegates to `neco-eigensolve-faer`

## Public API

- `solve_certified_generalized_eigen`: the function that computes a certified error bound for each eigenpair
- `CertifiedEigenSolution`: a solution that gathers eigenspaces, the convergence state, a lower bound on the smallest eigenvalue of the mass matrix, and a reference to the input
- `CertifiedEigenspace`: one eigenvalue and its certified basis
- `CertifiedEigenpair`: an eigenpair with three certified values
- `CertifiedFrequency`: a frequency estimate with an outward-rounded interval
- `CertifiedEigenError`: solver failures (described below)
- `certify_spectral_clusters`: partitions a complete spectrum into clusters with determined mode numbers
- `SpectralClusters`, `SpectralCluster`: the whole partition and one cluster in it
- `ModeOrdinal`: a determined mode number, or a number range within a cluster
- `correspond_to_reference`: bounds the distance between the subspaces of two solutions
- `ModeCorrespondence`, `BlockCorrespondence`: that correspondence and the blocks that make it up
- `ClusterError`: the failures returned by the two functions above

## What the bounds guarantee

A pair of a stiffness matrix $K$ and a mass matrix $M$ is called a matrix pencil.
The generalized eigenvalue problem for a matrix pencil is written as follows.

$$ K x = \lambda M x $$

Each per-eigenpair bound is an upper bound on the distance to at least one eigenvalue of the symmetric part of the exact pencil represented by the input floating-point matrices.
The cluster values described below give, for the same exact pencil, eigenvalue counts, cluster gaps, condition number bounds, and subspace angle bounds.
These are quantities separate from a distance to an eigenvalue.

- For each eigenpair, the only claim is that an eigenvalue exists within that distance. A mode number, by contrast, is determined only for a cluster that is a singleton in a partition of the complete spectrum (described below)
- The exact stiffness and mass matrices are assumed to be symmetric. This crate has no way to check that assumption

`CertifiedFrequency` treats an eigenvalue as the square of an angular frequency.

- `value`: an estimate of $\sqrt{\lambda} / 2\pi$ for the eigenvalue $\lambda$
- `lower_bound`, `upper_bound`: the interval endpoints in the same unit

## Error model

The floating-point residual (`EigenResidual`) is used only to decide residual convergence.
Cancellation can round this residual below the true residual, so it is never used as evidence for a forward-error bound.

Each entry of an input matrix is its exact value rounded to f64.
The projection error of a matrix is the sum, over all entries, of the upper bounds on the absolute rounding error.
The projection errors of the stiffness and mass matrices are written $\eta_K$ and $\eta_M$.
Both are available through `absolute_error_bound` on the matrix input.

The mass lower bound $\sigma$ is a lower bound on the smallest eigenvalue of the mass matrix.
With `Identity`, $\sigma = 1$.
With `PositiveDefiniteCholesky`, $\sigma$ comes from Gershgorin discs: for each row, subtract the sum of the absolute values of the off-diagonal entries from the diagonal entry, and take the minimum over all rows.

`CertifiedEigenpair` holds three exact values individually.

- `computational_bound`: an upper bound on the distance from the eigenvalue estimate to an eigenvalue of the same floating-point pencil, computed with exact `Dyadic` arithmetic
- `projection_bound`: an upper bound on the distance between the floating-point pencil and the exact pencil, derived from the projection errors $\eta_K$ and $\eta_M$
- `enclosure_radius`: the sum of the two bounds above

`CertifiedFrequency` returns an estimate and an interval rounded outward by one bit at every operation.
Whenever the value of `enclosure_radius` is valid, this interval contains the frequency corresponding to at least one eigenvalue of the symmetric part of the exact pencil.

## Reading the input lineage

`CertifiedEigenSolution::projection_reference` returns the input unchanged, so every certified value can always be traced back to its source expression.

```text
let reference = solution.projection_reference();
let entry = reference.stiffness().entry(row, column)?;
entry.value();       // the f64 value the solver used
entry.expression();  // the identifier of the source expression (ExprId)
reference.stiffness().absolute_error_bound();  // the absolute error bound of the stiffness matrix (Dyadic)
```

## Clusters and mode numbers

Clusters are built by `certify_spectral_clusters`.
This function accepts only a complete spectrum, one whose returned eigenpair count equals the matrix dimension, and merges eigenpairs whose intervals overlap until the intervals are pairwise disjoint.
As a result, each cluster contains exactly as many exact eigenvalues as it has eigenpairs, and the positions of those eigenvalues in ascending order are determined.

- `ModeOrdinal::Certified`: the eigenpair is alone in its cluster, and its mode number is determined
- `ModeOrdinal::WithinCluster`: the eigenpair shares its cluster with other eigenpairs, and only the number range of the cluster is known
- `SpectralCluster`: the number range, the interval and its radius, the gap to the neighboring clusters, the condition number bound, and the subspace angle bound
- `SpectralClusters`: the clusters of one solution, with the mode number claim for each eigenpair

Repeated eigenvalues, and eigenvalues closer to each other than the certified error bound, always form one cluster containing two or more eigenpairs.
As a result, these eigenpairs are given no individual mode numbers.
The subspace angle bound is an upper bound on the sine of the largest principal angle between the span of the cluster's eigenvectors and the exact eigenspace.
The angle is measured in the mass inner product of the exact pencil.
The bound saturates at 1. The value 1 is the largest value the sine can take, which is the trivial bound.

## Correspondence with another solution

`correspond_to_reference` is a function that compares a candidate solution with the clusters of a reference solution.
The comparison is independent of eigenvector signs and order.
For each block of mode numbers, it bounds the distance between the subspaces spanned by the two sets of eigenvectors.
A block is the finest run of numbers that starts and ends at positions that are cluster boundaries in both solutions.

The target of the comparison is the exact pencil of the reference solution, and the difference between the two floating-point pencils is added to the projection error.
The call therefore needs no assumption that both solutions come from the same pencil.

- `ModeCorrespondence`: the collection of blocks
- `BlockCorrespondence`: the mode numbers of the block, the cluster index range on each side, and the distance bound

A distance bound is a value between 0 and 1.
It bounds the 2-norm of the difference between the projections onto the two subspaces, measured in the mass inner product of the exact pencil of the reference solution.
The bound saturates at 1. The value 1 is the largest value this 2-norm can take, which is the trivial bound.

The function that builds clusters and the function that computes the correspondence both return their failures as `ClusterError`.

- `IncompleteSpectrum`: the returned eigenpair count differs from the dimension
- `DimensionMismatch`: the reference and candidate dimensions differ
- `GramNotPositive`: no positive lower bound shows that the eigenvectors of a range are linearly independent
- `MassPerturbationTooLarge`: the mass difference reached the candidate's mass lower bound
- `ExactArithmetic`: exact arithmetic could not represent a value
- `Allocation`: allocating a vector or matrix failed

## Solver failures

`CertifiedEigenError` distinguishes solver failures by kind.

- `InvalidProblemShape`: the shapes of the stiffness and mass matrices are incompatible
- `InvalidStiffness`, `InvalidMass`: a failure caused by a matrix, returned by the delegated solver
- `SolverInternal`: any other failure returned by the delegated solver
- `RequestedModesExceedDimension`: the requested mode count exceeded the dimension
- `ResidualNotConverged`: at least one residual missed its tolerance
- `Undeterminable`: the bound could not be certified; holds one of the following reasons per eigenpair
  - `MassLowerBoundUnavailable`: the mass lower bound $\sigma$ was zero or negative
  - `ProjectionPerturbationTooLarge`: twice the projection error of the mass matrix, $2\eta_M$, exceeded the mass lower bound $\sigma$
  - `NegativeEigenvalue`: the eigenvalue is negative
  - `EnclosureLowerBoundBelowZero`: the lower end of the interval is negative
  - `NonFiniteFrequencyBound`: an endpoint of the frequency interval rounded to infinity or NaN
- `ExactArithmetic`: exact arithmetic could not represent a value
- `Allocation`: allocating a vector or matrix failed

## Dependencies

- `neco-eigensolve`, `neco-eigensolve-faer`: the delegated numerical solvers
- `neco-generalized-eigen`: problem, eigenpair, and convergence state types
- `neco-linear-exact`: the certified matrix input and its projection error
- `neco-bigint`: the exact `Dyadic` arithmetic behind each bound

## Runtime configuration

This crate depends on `neco-eigensolve-faer` and therefore requires the standard library.

## License

MIT License.
