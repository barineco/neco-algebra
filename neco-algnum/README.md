# neco-algnum

[日本語](README-ja.md)

`neco-algnum` provides exact real algebraic numbers. A value is identified by a pair: an integer minimal polynomial in normal form (primitive, irreducible, positive leading coefficient) and a zero-based index into its ascending real roots. For example, `sqrt(2) + sqrt(3)` is the pair

```text
m(x) = x^4 - 10x^2 + 1,    k = 3
```

The four real roots of this polynomial have the form shown above with independent signs.
The largest root has index 3 and equals the sum of the two positive square roots.
Certified dyadic intervals construct and observe a value, while the polynomial and root index preserve its identity.

## Polynomials and validated types

Integer and rational coefficients are stored from degree zero upward. Construction states and validated values use separate types:

- `Polynomial`: integer coefficients with trailing zeroes removed
- `RationalPolynomial`: rational coefficients with trailing zeroes removed
- `CandidatePolynomial`: a candidate of degree at least one
- `SquareFreePolynomial`: primitive, positive-leading, and square-free
- `IrreduciblePolynomial`: irreducible over the rationals
- `MinimalPolynomial`: the irreducible polynomial that identifies a value
- `PolynomialQuotient`: remainder operations in `Q[x] / (m)`
- `GeneratorRepresentative`: `x mod m` for the same modulus
- `RootIndex`: the ascending index of a real root of one irreducible polynomial
- `RealAlgebraic`: an exact value containing a minimal polynomial and root index
- `IsolatingInterval`: a dyadic observation interval containing an exact value
- `CertifiedAlgebraic`: an exact value and interval produced by root isolation

`Polynomial` supports arithmetic, differentiation, evaluation, and composition.
`CandidatePolynomial::square_free` removes content and repeated factors.
Factorization enumerates candidates completely with the Kronecker method, and that enumeration is the sole irreducibility evidence.

`PolynomialQuotient` reduces operation results modulo the minimal polynomial and returns a `RationalPolynomial` of degree below the modulus:

- `reduce`: reduce one polynomial
- `add`: reduce an addition result
- `sub`: reduce a subtraction result
- `mul`: reduce a multiplication result

`generator` returns the reduced representative of the polynomial variable for the same modulus.
`RationalPolynomial::to_real_algebraic_coefficients` converts rational coefficients to real algebraic values and returns `RationalCoefficientConversion`.

## Certified real roots

Sturm sequences isolate every real root into intervals that each contain exactly one distinct root, and assign indices in ascending numeric order within one irreducible polynomial. Neighboring intervals may share an endpoint. Root construction uses these operations:

- `isolate_real_roots`: return all real roots with certified intervals
- `certify_root`: validate two caller-provided dyadic endpoints
- `into_value`: extract the exact value from a certified construction result

`RealAlgebraic::enclose` and `IsolatingInterval::refine` preserve the minimal polynomial and root index while returning an interval within the requested width.

Exact observations use these operations:

- `compare`: compare two real algebraic numbers
- `compare_dyadic`: compare with a dyadic value
- `sign`: determine the sign
- `is_zero`: decide zero from the minimal polynomial and root index
- `is_one`: decide the multiplicative unit from the minimal polynomial and root index
- `minimal_polynomial`: observe the identifying minimal polynomial
- `root_index`: observe the identifying root index

## Exact algebraic operations

`RealAlgebraic` provides these operations:

- `add`: add two real algebraic numbers
- `sub`: subtract two real algebraic numbers
- `mul`: multiply two real algebraic numbers
- `div`: divide two real algebraic numbers
- `pow_integer`: an integer power
- `pow_rational`: a real-valued reduced rational power
- `nth_root`: a positive-degree real root
- `from_form_sum`: promotion of an exact `FormSum`
- `equals_form_sum`: cross-representation equality by substitution and root index

Addition, subtraction, multiplication, division, powers, and roots proceed through resultant construction, square-free normalization, complete factorization, and certified root selection, and return every result in the same representation. Division rejects an exact zero divisor first. An even root of a negative value has no real result and returns a failure, while an odd root returns the unique real root.

## Failures

`AlgnumError` distinguishes input, operation, representation, storage, and lower-layer failures:

- `ZeroPolynomial`: no candidate of degree at least one
- `InvalidIsolation`: unordered, equal, or root-valued endpoints
- `NoTargetRoot`: no target root in the interval
- `MultipleTargetRoots`: multiple target roots in the interval
- `DivisionByZero`: division by an exact zero value
- `UndefinedZeroPower`: `0^0`
- `ZeroToNegativePower`: a negative power of zero
- `ZeroRootDegree`: a root of degree zero
- `EvenRootOfNegative`: an even root of a negative value
- `RepresentationLimit`: a degree, coefficient-count, or Sylvester representation limit
- `AllocationLimit`: an exact total element count beyond `usize`
- `AllocationFailure`: allocator refusal with the resource and requested element count
- `Bigint`: the preserved variant and payload from `neco-bigint`
- `FormSum`: the preserved variant and payload from `neco-formsum`

Two supporting types classify the failing subject:

- `RepresentationResource`: root degree, polynomial degree, coefficient count, Sylvester dimension, and Sylvester element count
- `AllocationResource`: stored coefficients, factors, Sturm sequences, root intervals, Sylvester elements, permutations, resultant coefficients, and related collections

Storage failures split into two cases: the exact total element count exceeds the platform limit, or the allocator refuses a request within that limit.

Public types that own variable-length values provide `try_clone` and return allocation failures through `Result`.

The `std` feature makes `AlgnumError` a standard error type.
Lower errors remain reachable through the standard error source.

## Features and dependencies

The default `std` feature enables standard error integration and the same feature in both dependencies. Disabling default features selects the `core + alloc` configuration with the same exact values and the same failures:

```console
cargo check -p neco-algnum --no-default-features
```

Runtime dependencies are:

- `neco-bigint`
- `neco-formsum`

## License

MIT License.
