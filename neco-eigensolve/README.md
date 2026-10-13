# neco-eigensolve

[日本語](README-ja.md)

`neco-eigensolve` is a numerical solver that solves real symmetric generalized eigenvalue problems by a deterministic procedure, and it can also be used in the `core + alloc` configuration without the standard library.

## Public API

- `EigensolveConfig`: requested mode count, tolerances, and iteration limit
- `EigensolveError`: failures in validating inputs, configurations, and results, and allocation failures
- `EigensolveRequest<R>`: the problem, the configuration, and a projection reference
- `EigensolveResult<R>`: eigenspaces, convergence status, spectral shift, and the projection reference moved from the request
- `solve_symmetric_f64`: solves a dense real symmetric problem
- `solve_request_symmetric_f64`: consumes a request, solves it, and moves its projection reference into the result
- `projection_reference`: borrows the projection reference held by a request or result
- `into_projection_reference`: takes out the projection reference owned by a result
- `solve_csr_symmetric_f64`: solves a real symmetric problem in CSR format

The projection reference held by a request and a result is an arbitrary value of the caller's type `R`.
The solver moves this value unchanged from the request to the result.

A projection reference attaches to the result the provenance of matrices projected from exact values to f64.
A representative type is `CertifiedGeneralizedEigenInput`, provided by `neco-linear-exact`, which carries the provenance of the stiffness and mass matrices.

## Engine

The Jacobi engine accepts only an exact identity matrix as the mass matrix.
It selects the largest off-diagonal entry in row and column order and applies deterministic rotations.
In the returned eigenspaces, each basis vector is normalized to unit 2-norm, and its sign is chosen so that the first component of largest absolute value is positive.

The eigenspace and convergence rules determine which basis vectors form an eigenspace, the number of modes returned, the convergence decision, and the value of the spectral shift.

- Only basis vectors whose computed eigenvalues are exactly equal form one eigenspace; tolerances are used only for residual-based convergence decisions
- Even when the requested mode count runs out in the middle of an eigenspace, the complete eigenspace is returned without splitting it, so the returned count can exceed the requested count
- A mode converges when its absolute or relative residual satisfies the corresponding tolerance
- The returned spectral shift is always `Complex<f64>::zero()`

## Dependencies

- `neco-generalized-eigen`: generalized eigenvalue problem and result types
- `neco-linear-dense`: dense problem matrices
- `neco-sparse`: problem matrices in CSR format
- `neco-complex`: complex spectral shifts

## Runtime configuration

The standard library is used by default.
Disabling default features selects the `core + alloc` configuration, which assumes only dynamic memory allocation.

## License

MIT License.
