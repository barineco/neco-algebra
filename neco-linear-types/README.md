# neco-linear-types

[日本語](README-ja.md)

`neco-linear-types` provides checked shapes, validated vectors, and linear operator interfaces for numerical linear algebra as public types.

## Public API

- `Shape`: holds the row and column counts and checks that rows × columns fits in `usize`
- `RowIndex`, `ColumnIndex`: indices created through shape validation
- `Vector<T>`: a vector that owns its values and gives access to its length, individual elements, and all elements
- `LinearOperator<T>`: a linear operator interface that defines the input length, output length, and application
- `LinearError`: dimension, index, storage-length, capacity-overflow, allocation, and invalid-storage failures

In `LinearOperator<T>`, `domain` returns the input length.
`codomain` returns the output length, and `apply` applies the operator to an input vector.

```text
domain(&self) -> usize
codomain(&self) -> usize
apply(&self, input: &Vector<T>) -> Result<Vector<T>, LinearError>
```

Each implementation validates its input and output lengths.

The capacity-overflow failure (`CapacityOverflow`) of `LinearError` reports a count, index, or allocation byte-size computation that exceeds its limit.

The invalid-storage failure is returned when a storage length exceeds `isize::MAX` and when a CSR invariant is violated.
It is also returned when reading an element of a constructed matrix finds the storage shorter than its declared shape.
The elimination in `neco-linear-exact` returns it when the storage of the working matrix, right-hand side, solution, or kernel basis vector is shorter than its declared dimension, and when a pivot position or pivot count exceeds the matrix dimension.

The storage-length failure (`StorageLengthMismatch`) reports an element sequence or vector whose length differs from the expected length.
The expected length is the element count of the shape (rows × columns) for dense matrix construction, the operator's input length (`domain`) for operator application, and the row count of the coefficient matrix for solving a linear system.

## Runtime configuration

The standard library is used by default.
Disabling default features selects the `core + alloc` configuration, which assumes only dynamic memory allocation.

## License

MIT License.
