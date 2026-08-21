# neco-linear-types

[日本語](README-ja.md)

`neco-linear-types` provides checked shapes, validated vector storage, and linear operator interfaces for numerical linear algebra.

## Public API

- `Shape`: row and column dimensions with checked element counts
- `RowIndex`, `ColumnIndex`: indices created through shape validation
- `Vector<T>`: vector storage with checked construction and value access
- `LinearOperator<T>`: interface for input length, output length, and application
- `LinearError`: dimension, index, storage-length, capacity, allocation, and storage-state failures

The linear-operator interface has these signatures:

```text
domain(&self) -> usize
codomain(&self) -> usize
apply(&self, input: &Vector<T>) -> Result<Vector<T>, LinearError>
```

Each implementation validates its input and output lengths.

## Runtime configuration

The default configuration uses the standard library. The no-default-features configuration uses dynamic allocation with `no_std`.

## License

MIT License.
