# neco-expr

[日本語](README-ja.md)

`neco-expr` is a crate that stores exact values as expression graphs and resolves them into certified floating-point values for each place that receives a resolved value.
Below, such a place is called a consumer.
A resolved result carries a finite `f64` value, a dyadic interval that meets the requested precision, and an exact absolute error bound.
The approximation to floating point happens only at the final step of resolution, and every intermediate value stays exact until the end.

## Usage

This example stores the exact value 1 as an atom (an exact value placed at a leaf of an expression) and resolves it at 20 bits of absolute precision.

```rust
use neco_expr::{
    AbsoluteBits, Assignments, AtomId, AtomStore, ConsumerId, ExactValue, ExprGraph,
    ExprNode, PrecisionRequirements, Resolver,
};
use neco_monomial::Monomial;

let mut graph = ExprGraph::new();
let expression = graph
    .push(ExprNode::Atom(AtomId::new(0)))
    .expect("valid expression node");

let mut atoms = AtomStore::new();
let atom_value = ExactValue::Monomial(Monomial::one());
atoms
    .insert(AtomId::new(0), atom_value)
    .expect("unique atom ID");

let consumer = ConsumerId::new(0);
let mut requirements = PrecisionRequirements::new();
requirements
    .insert(consumer, AbsoluteBits::new(20))
    .expect("unique consumer ID");
let mut assignments = Assignments::new();
assignments
    .insert(consumer, expression)
    .expect("unique consumer ID");

let (_, _, resolved) = Resolver::new()
    .resolve_all(&graph, &atoms, &requirements, &assignments)
    .expect("sufficient storage");
let certified = resolved
    .get(consumer)
    .expect("requested consumer")
    .as_ref()
    .expect("successful resolution");
assert_eq!(certified.value(), 1.0);
```

## Expressions and exact values

`ExactValue` holds an exact value in one of the three layers distinguished by `ExactLayer`.

- `Monomial` (layer 1): a normalized monomial
- `FormSum` (layer 2): a normalized formal sum
- `Algebraic` (layer 3): a real algebraic number with a minimal polynomial and a real-root index

An expression graph is assembled by pushing `ExprNode` nodes into an `ExprGraph`.

- `ExprGraph`: an arena that keeps nodes in insertion order
- `ExprNode`: atoms, negation, the four arithmetic operations, and reduced rational powers

Operation nodes can reference only nodes that were already inserted, so the graph is always acyclic.

For example, an expression for the golden ratio is built from the atom $ 5^{1/2} $ (a `Monomial`) and exact constants.
The addition produces a `FormSum`.
Resolving the expression returns an interval within the requested width that contains $ \varphi $, together with a finite `f64` selected from it.

$$ \varphi = \frac{1 + 5^{1/2}}{2} $$

The result layer is determined uniquely by the input layers and the kind of operation.

| Operation | Inputs | Result |
|---|---|---|
| Negation | Any layer | Same layer as the input |
| Addition, subtraction | Includes `Algebraic` | `Algebraic` |
| Addition, subtraction | Any other combination | `FormSum` |
| Multiplication, division | Two `Monomial` values | `Monomial` |
| Multiplication, division | Includes `Algebraic` | `Algebraic` |
| Multiplication, division | Any other combination | `FormSum` |
| Integer power | Any layer | Same layer as the input |
| Non-integer rational power | `Monomial` | `Monomial` |
| Non-integer rational power | `FormSum` | `Algebraic` |
| Non-integer rational power | `Algebraic` | `Algebraic` |

## Inputs and resolution

`Resolver::resolve_all` takes the resolution input as four arguments of different types.

- `ExprGraph`: expression nodes
- `AtomStore`: the mapping from atom IDs to exact values
- `PrecisionRequirements`: the mapping from consumer IDs to absolute precision
- `Assignments`: the mapping from consumer IDs to expression IDs

`resolve_all` resolves every consumer in ID order in a single call.

The call returns a tuple of the evaluation cache, the isolation cache, and the resolved values, in this order.

- `EvaluationCache`: the exact value, or the evaluation failure, for each reached expression ID
- `IsolationCache`: isolating intervals of algebraic numbers, reusable for the same expression ID and precision
- `ResolvedValues`: a `CertifiedF64`, or a resolution failure, for each consumer ID

Unknown expression IDs and atom IDs are kept out of the evaluation cache and are recorded as resolution failures of the requesting consumer.
When one consumer fails, its failure is stored in the result map and resolution continues with the remaining consumers.
Therefore, the resolution as a whole returns failure only when a cache or the result map itself cannot be stored.

## Certified floating-point values

`CertifiedF64` holds three values together.

- `value()`: a finite `f64` selected with ties-to-even rounding
- `enclosure()`: a dyadic interval that contains the exact value
- `absolute_error()`: the larger of the distances from the selected value to the two interval endpoints

The absolute precision given by `AbsoluteBits(bits)` is the following upper limit on the interval width.

```text
upper - lower <= 2^(-bits)
```

Every `u32` value, including zero, can be given as the precision.
The floating-point value is selected independently of the requested precision.
Selection normalizes negative zero to positive zero and then compares the exact value with every finite `f64` value, including subnormals, positive zero, and the largest finite value.

Here, projection means computing, from an exact value, a finite `f64` value and an interval that contains the exact value.
Exact values that the caller owns separately from an expression graph are projected with `project_exact_value_f64`.
Its arguments are the exact input value and the precision policy for the interval, and its return value is the projection result.

- `project_exact_value_f64`: projects one exact value
- `ExactValue`: the exact input value
- `ProjectionPolicy`: specifies the interval precision
- `CertifiedScalarProjection`: holds the policy, the selected value, the interval, and the absolute error

`project_exact_value_f64` returns `ScalarProjectionError::FloatOutOfRange` when the exact value is beyond the finite floating-point range.

An already computed interval is projected with `project_enclosure_f64`, which rounds the midpoint of the interval to select an `f64` value.
The rounding and the absolute-error computation are the same as in expression resolution.

However, when the interval is wider than the maximum width that the requested `AbsoluteBits` allows, this function returns `EnclosureProjectionError::InsufficientWidth`.

## Failure types

- `GraphError`: exhausted identifiers, references to nodes not yet inserted, node cloning, and graph storage failures
- `InsertError`: duplicate identifiers, atom value cloning, and input map storage failures
- `EvalError`: division by zero, powers of zero, even roots, and lower-crate failures
- `ResolveError`: missing assignments (a consumer with a precision requirement has no assigned expression), unknown identifiers, exceeding the finite range, evaluation, lower arithmetic, and result storage failures
- `ScalarProjectionError`: range, lower-crate, and storage failures in standalone projection
- `EnclosureProjectionError`: insufficient width and lower-crate failures in interval projection
- `NecoFailure::ProvenanceMismatch`: a consumer, owner, expression, precision, implementation source (`NecoImplementationSource`), or computation result identifier differs from the requirement
- `NecoFailure::MatrixInputMismatch`: a matrix shape, expression mapping, or floating-point bit pattern differs from the certified projection
- `NecoFailure::MissingMatrixInput`: the `MassNormalizationReference`, which holds the mass matrix and how to treat it, was absent when the generalized eigenvalue input was built
- `NecoFailure::PrecisionExhaustion`: the requested interval width cannot be achieved; holds the requested precision and the lower cause
- `StorageError`: capacity overflow, or an allocation failure with the required total element count

In a `NecoFailure` variant that carries a value, the value is stored in a `Box` allocation.

- Construction: allocates the `Box` that stores the value; a failed allocation follows the default allocation error handling
- Value accessors: the same as when the value is stored directly
- `Debug` output: the same as when the value is stored directly

Lower failures from interval computation propagate as two `ResolveError` variants.

- `ResolveError::Bigint`: a failure from dyadic rational and interval processing
- `ResolveError::Algnum`: a failure from interval processing of algebraic numbers

A stored lower failure can be referenced through `std::error::Error::source`. Owned data can be cloned with `try_clone`, which checks for allocation failure.
Types that own sequences are cloned only through `try_clone`.

## High-level exact computation

An owner is a unit on the caller side that brings a set of expressions and requirements, distinguished by an `OwnerId` that the caller supplies.
The high-level pipeline takes one expression set per owner and assigns exact and numerical computation to each requirement.
Each expression set is built from an expression graph and atoms created with the lower-level expression graph API.

- `OwnerId`: an owner identifier; the caller supplies a `u32` value
- `OwnerExpressionSet`: one owner with its expression graph, atoms, and requirements
- `ExactAllocationInput`: the sequence of per-owner expression sets, the observed capabilities, and the implementation source
- `ExactExpressionRequirement`: one consumer, the selected expressions, an exact decision request, and an absolute precision; the request is an `ExactDecisionRequest` value that holds the decision kind (zero, equality, sign, or degeneracy) and the expressions to decide; the degeneracy decision classifies the expression as degenerate when its value is zero
- `NecoObservedCapability`: the list of operations observed to be available in the implementation source
- `NecoImplementationSource`: the implementation source, the identifier of the version and public API referenced by results; the version is the package version of `neco-expr` and corresponds to the same version on crates.io

Owners are matched by identifier value alone.

- A lineage whose owner differs from the owner of its requirement: returns a `ProvenanceMismatchField::Owner` failure
- An owner that appears twice: returns `NecoFailure::StorageFailure` carrying that owner's identifier as the duplicate identifier

When different owners use the same `ConsumerId`, each result is still built from the requirement of its own owner.
The result is the same in whatever order the owner sequence is given.

The public functions that make up the high-level processing are called in the following order.
From the third function on, each takes ownership of the value returned by the previous one, so the second through sixth functions can be called only in this order.

1. `read_exact_numeric_inputs`
2. `allocate_exact_numeric`
3. `normalize_exact_expressions`
4. `decide_exact_properties`
5. `resolve_certified_f64`
6. `assemble_exact_computation_product`

`ExactNumericAllocation` is a value that holds the set of owner-consumer pairs to resolve.

`ExactComputationProduct` returns a `CertifiedF64Bundle` for each owner-consumer pair, and results are looked up by the pair of an owner identifier and a consumer identifier.
The lineage of a result can be read from the following values.

- `CertifiedF64Bundle`: the owner, consumer, requested precision, implementation source, and computation result identifier
- `ComputationProductId`: an opaque identifier for the result of one computation
- `ConsumerId`: the identifier of a consumer
- `AbsoluteBits`: the requested width of the dyadic interval
- `NecoImplementationSource`: the version and public API referenced by the result
- `CertifiedF64Expression`: the certified result for each expression
- `ExprId`: the identifier of an expression
- `CertifiedF64`: a finite floating-point value, an enclosure, and an absolute error bound
- `CertifiedF64Bundle::try_clone`: clones while preserving the lineage and the computation result identifier

The assembly step checks that the requirement set and the result set correspond completely one to one, and returns mismatched or missing lineage as `NecoFailure`.

`AbsoluteBits` constrains only the width of the exact dyadic interval.
By contrast, the finite floating-point value is selected independently of the requested width. The value nearest to the exact value is selected, and ties are resolved with ties-to-even rounding.
A high-level result combines an interval that satisfies the requested width with a finite rounded value.

`direct_inspection` reads the allocations, the requirements with owner-consumer pairs, the decision results, the certified values, the lineage, and the resolution count.
The resolution count is the number of resolutions actually performed after merging, per owner, the requirements for the same expression at the same precision.
`CertifiedF64::absolute_error()` computes the absolute error bound of the projection from the interval endpoints.

Each consumer handles its own numerical error budget and physical measurement uncertainty.
A consumer is identified by the pair of `OwnerId` and `ConsumerId`.
The caller uses this pair to match its own values with the certified result.

## Configuration and dependencies

At runtime, the crate depends only on crates in this repository.

- `neco-bigint`
- `neco-monomial`
- `neco-formsum`
- `neco-algnum`

With the default `std` feature enabled, the failure types other than `NecoFailure` can be used as standard error types.
This feature also enables the feature of the same name in the dependencies.
Disabling default features selects the `core + alloc` configuration.

```bash
cargo check -p neco-expr --no-default-features
```

## License

MIT License.
