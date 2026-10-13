# neco-formsum

[日本語](README-ja.md)

`neco-formsum` is a crate for exact rational linear combinations (formal sums) of normalized radical monomials.
Values stay symbolic during computation, and equality of values is decided by comparing the structure of sparse normal forms.
Sign decisions use dyadic intervals proved to contain the exact value.

## Input and normal form

The roles of construction inputs and the normal form are split across types and operations:

- `RawTerm`: an unreduced rational coefficient and a construction monomial
- `RawFormSum::normalize`: validates and normalizes every input
- `FormSum`: a normal form with distinct bases in lexicographic order and nonzero coefficients

Normalization moves rational factors of monomials into coefficients, combines terms that share a `RadicalBasis`, and removes terms whose coefficient becomes zero.
A formal sum with no terms left after normalization represents zero.
For example, the following value is a normal form of three terms with coefficients `1, 2, -1` over the bases `1, 2^(1/2), 3^(1/2)`.

$$ 1 + 2\sqrt{2} - \sqrt{3} $$

The result of normalization is the same regardless of input order.
When the input contains a coefficient or exponent with a zero denominator, `0^0`, or a negative power of zero, these failures are returned as a sorted and deduplicated `NormalizationErrors<FormSumErrorKind>`.
When a failure prevents the computation from continuing, processing stops there and the failure is returned as a single error.

## Exact arithmetic

Formal sums provide addition, subtraction, multiplication, division, and inversion, and return every result in normal form.
Division and inversion are computed by solving exact linear systems with rational coefficients over a finite radical extension.
Division by zero returns `FormSumErrorKind::DivisionByZero`.

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

`extension_with` constructs the smallest common extension that contains both values.
For example, the smallest extension containing $ \sqrt{2} $ and $ \sqrt{3} $ is $ \mathbb{Q}(\sqrt{2}, \sqrt{3}) $, which has a basis of dimension $ D = 4 $.
`RadicalExtension` exposes its ascending prime sequence, its exponent denominators, and its basis count.
The basis is ordered in mixed-radix order, with the last coordinate changing fastest.
The coordinate representation treats a formal sum as a sequence of rational coefficients with respect to this basis.

- `coordinates_with`: represents a value in coordinates over an extension that contains the required primes and denominators
- `RadicalCoordinates`: reconstructs the formal sum and produces the column-major multiplication matrix

A matrix index is computed by the following expression.

```text
row + D * column
```

That entry is the coefficient of the basis element selected by the row in the product of the value with the basis element selected by the column.

`annihilating_coefficients` returns a primitive annihilating polynomial with integer coefficients.
The coefficients are ordered from the lowest degree upward, and the highest-degree coefficient is positive.

## Enclosures and sign

- `enclose(bits)`: returns an interval containing the exact value with width at most `2^-bits`
- `sign`: first checks whether the term list of the normal form is empty (the value is zero), and if the value is nonzero, obtains the sign from an interval that excludes zero

Radical intervals are constructed using only integer power inequalities as evidence.

## Failures and allocation

The public failure types represent the kinds of failure and the subject of a dimension overflow:

- `FormSumErrorKind`: lower-crate failures, division by zero, dimension overflow, and allocation refusal
- `DimensionResource`: distinguishes the subject of a dimension overflow (exponent denominators, basis counts, and matrix element counts)

Types that own variable-length storage provide `try_clone`, so an allocation failure during cloning is received through `Result`.

## Features and dependencies

With the default `std` feature enabled, `FormSumErrorKind` can be used as a standard error type.
This feature also enables the feature of the same name in both dependencies.
Disabling default features selects the `core + alloc` configuration, which handles the same values and the same failures.

```console
cargo check -p neco-formsum --no-default-features
```

At runtime, the crate depends on `neco-bigint` and `neco-monomial`.

## License

MIT License.
