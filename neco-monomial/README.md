# neco-monomial

[日本語](README-ja.md)

`neco-monomial` provides monomials that represent radicals exactly. A monomial carries a reduced rational exponent for each prime base, which makes values such as `sqrt(12) = 2 sqrt(3)` exact rather than approximate.

Dependencies:

- `neco-bigint`

## Features

- Construction inputs that allow composite and repeated bases
- Normalization to canonical prime bases through finite trial division
- Multiplication, division by nonzero values, and reduced rational powers
- Unique decomposition into a rational coefficient and a `RadicalBasis`
- Ordered error sets that report every invalid zero power at once

The design extends positional exponents from integers to rationals: integer exponents express rationals, and noninteger exponents express radicals. The normal form is a sign with ascending prime exponents, and it holds values like these:

```text
sqrt(12) = 2^1 * 3^(1/2)
sqrt(2) * sqrt(8) = 2^(1/2) * 2^(3/2) = 2^2 = 4
```

`split_radical` separates a monomial into a rational coefficient and a radical basis.

## Usage

This example normalizes the square root of 12 and splits it into the coefficient `2` and the basis `sqrt(3)`:

```rust
use neco_bigint::{BigInt, BigUint, RawRational};
use neco_monomial::{RawMonomial, RawPower};

fn main() {
    let exponent = RawRational::new(
        BigInt::try_from(1_i32).unwrap(),
        BigUint::try_from(2_u32).unwrap(),
    );
    let raw = RawMonomial::positive(vec![RawPower::new(
        BigUint::try_from(12_u32).unwrap(),
        exponent,
    )]);
    let value = raw.normalize().unwrap();
    let (coefficient, basis) = value.split_radical().unwrap();
    let expected = BigInt::try_from(2_i32).unwrap();
    assert_eq!(coefficient.numerator(), &expected);
    let prime = &basis.factors()[0].0;
    assert_eq!(prime.value().to_u32(), Some(3));
}
```

## Public types

- `RawPower`: a nonnegative construction base and an unreduced exponent
- `RawMonomial`: an explicit zero, or a sign and construction powers
- `NormalizationErrors`: an ordered set of distinct normalization failures
- `MonomialErrorKind`: failures from normalization and monomial operations
- `ProvenPrime`: a value proven prime by trial division
- `Monomial`: the normal form, either zero or a sign with ascending prime exponents
- `RadicalBasis`: prime exponents restricted to `0 < exponent < 1`

Canonical types have private fields, and owning values are copied through `try_clone`.

The supporting operations:

- `NormalizationErrors::from_one`: construct from one failure
- `NormalizationErrors::from_errors`: sort a failure sequence and construct
- `NormalizationErrors::errors`: visit the first and additional failures in order
- `NormalizationErrors::into_parts`: transfer the owned first and additional failures
- `RadicalBasis::try_from_sorted_factors`: validate strictly ascending distinct primes with exponents strictly between zero and one

## Failures

`MonomialErrorKind` distinguishes these conditions:

- `DivisionByZero`: the divisor is zero
- `ZeroToNegativePower`: zero raised to a negative power
- `UndefinedZeroPower`: `0^0`
- `EvenRootOfNegative`: an even root of a negative value
- `InvalidRadicalBasis`: the basis sequence fails validation
- `CapacityOverflow`: a checked capacity is exceeded
- `AllocationFailure { requested_elements }`: allocation fails; the payload is the total required element count
- `Bigint(BigintError)`: a failure from the underlying crate

Normalization collects the semantic failures found in the input and returns them as `NormalizationErrors`. Capacity overflow and allocation refusal stop the operation immediately and return a single error.

## Runtime configuration

The default configuration uses the standard library:

```text
std
```

Disabling default features selects a minimal configuration that assumes only dynamic allocation:

```text
core + alloc
```

Values and failures are identical in both configurations.

## License

MIT License.
