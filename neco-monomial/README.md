# neco-monomial

[日本語](README-ja.md)

`neco-monomial` is a crate that provides monomials for representing radicals exactly.
A monomial carries a reduced rational exponent for each prime base, so values such as $ \sqrt{12} = 2\sqrt{3} $ are handled without any approximation.

The only direct dependency is `neco-bigint`.

## Provided operations

- Construction inputs that allow composite and repeated bases
- Normalization to a normal form with prime bases through finite trial division
- Multiplication, division by nonzero values, and powers with reduced rational exponents
- Unique decomposition into a rational coefficient and a radical basis (`RadicalBasis`)
- A check that reports, at once and as an ordered set, every power whose exponent has a zero denominator and every invalid zero power (`0^0` or a negative power of zero)

This monomial can be regarded as an extension of the exponent attached to each prime in the prime factorization of an integer from integers to rationals.
Integer exponents represent rationals, and non-integer exponents represent radicals.
The normal form of a monomial consists of a sign and a sequence of exponents ordered by ascending prime, and it holds values as follows.

$$ \sqrt{12} = 2^{1} \cdot 3^{1/2}, \qquad \sqrt{2} \cdot \sqrt{8} = 2^{1/2} \cdot 2^{3/2} = 2^{2} = 4 $$

`split_radical` decomposes $ 2^{1} \cdot 3^{1/2} $ into the rational coefficient $ 2 $ and the radical basis $ 3^{1/2} $.

## Usage

This example normalizes the square root of 12 and decomposes it into the coefficient 2 and the basis $ \sqrt{3} $.

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
- `RawMonomial`: an explicit zero, or a sign and a sequence of construction factors
- `NormalizationErrors`: a set of normalization failures, deduplicated and ordered
- `MonomialErrorKind`: failures returned by monomial operations and normalization
- `ProvenPrime`: a value confirmed prime by trial division
- `Monomial`: the normal form, either zero or a sign with a sequence of exponents over ascending primes
- `RadicalBasis`: a sequence of prime exponents whose exponents satisfy `0 < exponent < 1`

The fields of the normal forms are private, and owned values are cloned with `try_clone`.
`NormalizationErrors` has operations that construct a value from failures, an operation that visits the owned failures, and an operation that takes them out.
A `RadicalBasis` value is constructed after its sequence of factors is validated.

- `NormalizationErrors::from_one`: constructs from one failure
- `NormalizationErrors::from_errors`: sorts a sequence of failures and constructs
- `NormalizationErrors::errors`: visits the first failure and the additional failures in order
- `NormalizationErrors::into_parts`: takes out the owned failures, split into the first and the additional ones
- `RadicalBasis::try_from_sorted_factors`: constructs after validating strictly ascending distinct primes with exponents strictly between 0 and 1

## Failures

`MonomialErrorKind` distinguishes failure conditions by kind.

- `DivisionByZero`: the divisor is zero
- `ZeroToNegativePower`: zero was raised to a negative power
- `UndefinedZeroPower`: `0^0` was requested
- `EvenRootOfNegative`: an even root of a negative value was requested
- `InvalidRadicalBasis`: the basis sequence failed validation
- `CapacityOverflow`: the capacity limit checked before the operation was exceeded
- `AllocationFailure { requested_elements }`: allocation failed; holds the required total element count
- `Bigint(BigintError)`: a failure from the lower crate

When the input contains powers whose exponent has a zero denominator, `0^0`, or negative powers of zero, normalization collects each of these failures and returns them deduplicated and ordered as `NormalizationErrors`.
Capacity overflow and allocation refusal stop the operation immediately and are returned as a single failure.

## Runtime configuration

The standard library is used by default.
Disabling default features selects the `core + alloc` configuration, which assumes only dynamic memory allocation.

Value behavior and failure kinds are the same in both configurations.

## License

MIT License.
