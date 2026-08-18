# neco-formsum

[日本語](README-ja.md)

`neco-formsum` provides exact rational linear combinations (formal sums) of normalized radical monomials. A value remains symbolic through arithmetic, equality is decided on its unique sparse normal form, and sign determination uses certified dyadic enclosures.

## Input and normal form

Construction and canonical values have these roles:

- `RawTerm`: an unreduced rational coefficient and a construction monomial
- `RawFormSum::normalize`: validation and normalization of every input
- `FormSum`: lexicographically ordered distinct bases with nonzero coefficients

Normalization moves rational factors into coefficients, combines terms that share a `RadicalBasis`, and removes terms whose coefficient becomes zero. A sum with no remaining terms is zero. For example, the value

```text
1 + 2 sqrt(2) - sqrt(3)
```

is a three-term normal form with coefficients `1, 2, -1` over the bases `1, 2^(1/2), 3^(1/2)`.

Normalization is independent of input order. When the input contains semantic errors, it returns a sorted, deduplicated `NormalizationErrors<FormSumErrorKind>`. A failure that stops the computation is returned as a single error.

## Exact arithmetic

`FormSum` supports addition, subtraction, multiplication, division, and inversion, and every result comes back normalized. Division and inversion solve exact rational linear systems in a finite radical extension.

- `FormSum`: the input and result of exact arithmetic
- `FormSumErrorKind::DivisionByZero`: division by zero

```rust
use neco_formsum::FormSum;

fn main() -> Result<(), neco_formsum::FormSumErrorKind> {
    let one = FormSum::one()?;
    let two = one.add(&one)?;
    let quotient = two.div(&one)?;

    assert_eq!(quotient, two);
    assert!(FormSum::zero().is_zero());
    Ok(())
}
```

## Finite radical extensions

`extension_with` constructs the smallest shared extension containing two values.
For example, adjoining the square roots of 2 and 3 produces a basis of dimension 4.
`RadicalExtension` exposes its ascending prime sequence, exponent denominators, and basis count.
The basis uses mixed-radix order with the last coordinate changing fastest.

Coordinate operations have these roles:

- `coordinates_with`: represent a value in an extension containing every required prime and denominator
- `RadicalCoordinates`: reconstruct the formal sum and produce the column-major multiplication matrix

The matrix uses column-major indexing. The entry at row i and column j is the coefficient of the i-th basis element in the product of the value with the j-th basis element.

`annihilating_coefficients` returns a primitive integer annihilating polynomial. Coefficients are ordered from degree zero upward, and the highest-degree coefficient is positive.

## Enclosures and sign

- `enclose(bits)`: an interval containing the exact value with width at most `2^-bits`
- `sign`: a structural zero test, then the sign proved by an interval excluding zero

Radical interval certificates use only integer power inequalities.

## Failures and allocation

Failure types have these roles:

- `FormSumErrorKind`: lower-layer failures, division by zero, dimension overflow, and allocation refusal
- `DimensionResource`: exponent denominators, basis counts, and matrix element counts

Types that own variable-length storage provide `try_clone`, so an allocation failure during cloning remains visible through `Result`.

## Features

The default `std` feature provides standard error integration and enables the same feature in both dependencies. Disabling default features selects the `core + alloc` configuration with the same values and the same failures:

```console
cargo check -p neco-formsum --no-default-features
```

Runtime dependencies:

- `neco-bigint`
- `neco-monomial`

## License

MIT License.
