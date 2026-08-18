# neco-linear-types

[日本語](README-ja.md)

`neco-linear-types` provides checked shapes, validated vector storage, and linear operator interfaces for numerical linear algebra.

## Public API

- `Shape`: row and column dimensions with checked element counts
- `RowIndex`, `ColumnIndex`: indices created through shape validation
- `Vector<T>`: vector storage with checked construction and value access
- `LinearOperator<T>`: a trait declaring the input and output vector lengths and the `apply` signature; length validation belongs to each implementation
- `LinearError`: dimension, index, storage-length, capacity, allocation, and storage-state failures

## Runtime configuration

The default configuration uses the standard library. The no-default-features configuration uses dynamic allocation with `no_std`.

## License

MIT License.
