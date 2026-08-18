# neco-eigensolve

[日本語](README-ja.md)

`neco-eigensolve` provides deterministic numerical eigensolvers for real symmetric generalized eigenvalue problems in `no_std + alloc` environments.

## Public API

- `EigensolveConfig`: requested mode count, tolerances, and iteration limit
- `EigensolveError`: solver validation and allocation failures
- `EigensolveRequest<R>`: problem, configuration, and consumer-owned projection reference
- `EigensolveResult<R>`: eigenspaces, convergence status, spectral shift, and projection reference
- `solve_symmetric_f64`: dense real symmetric problems
- `solve_request_symmetric_f64`: owned request with reference propagation
- `solve_csr_symmetric_f64`: CSR real symmetric problems

## Engine

The Jacobi engine accepts an exact identity mass matrix. It selects the largest off-diagonal entry in row and column order, applies deterministic rotations, and returns eigenspaces with canonical basis-vector signs.

The eigenspace and convergence rules:

- Only exactly equal computed eigenvalues share one eigenspace; tolerances apply to residual convergence, not to eigenvalue identity
- A requested mode count never splits an eigenspace, so the returned count can exceed the requested count
- A mode converges when its absolute or relative residual satisfies the corresponding tolerance
- The returned spectral shift is always `Complex<f64>::zero()`

## Dependencies

- `neco-generalized-eigen`: generalized-problem and result types
- `neco-linear-dense`: dense problem matrices
- `neco-sparse`: CSR problem matrices
- `neco-complex`: complex spectral shifts

## Runtime configuration

The default configuration uses the standard library. Disabling default features selects `no_std` with `alloc`.

## License

MIT License.
