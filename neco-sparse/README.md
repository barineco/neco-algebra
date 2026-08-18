# neco-sparse

[日本語](README-ja.md)

`neco-sparse` provides sparse matrices for numerical linear algebra in `no_std + alloc` environments. COO storage accepts triplets in any order, including duplicates, as long as the indices lie within the shape, and CSR storage validates compact row-oriented invariants for efficient matrix-vector multiplication.

## Public API

- `CooMatrix<T>`: sparse triplet storage with checked indices
- `CooMatrix::to_csr`: stable coordinate ordering and duplicate summation
- `CsrMatrix<T>`: validated compressed sparse row storage
- `CsrMatrix::from_parts`: constructs CSR storage after invariant checks
- `CsrMatrix::row`: borrows one row as `CsrRow`
- `CsrRow`: observes column indices, values, or paired entries
- `LinearOperator<f64>`: applies CSR matrix-vector multiplication

`neco-sparse` depends directly on `neco-linear-types`. Dense conversion is provided by higher-level integration crates.

## Runtime configuration

The default configuration uses the standard library. Disabling default features selects `no_std` with `alloc`.

## License

MIT License.
