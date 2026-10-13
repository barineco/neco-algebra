import Mathlib.Data.Real.Basic

set_option autoImplicit false

namespace NecoAlgebraVerify

def eval : List Int → Rat → Rat
  | [], _ => 0
  | coefficient :: rest, value => Rat.ofInt coefficient + value * eval rest value

def derivativeFrom : Nat → List Int → List Int
  | _, [] => []
  | index, coefficient :: rest =>
    Int.ofNat index * coefficient :: derivativeFrom (index + 1) rest

def derivative (coefficients : List Int) : List Int :=
  derivativeFrom 1 (coefficients.drop 1)

def signChange (left right : Rat) : Prop :=
  (left < 0 ∧ 0 < right) ∨ (right < 0 ∧ 0 < left)

instance (left right : Rat) : Decidable (signChange left right) := by
  unfold signChange
  infer_instance

structure Interval where
  lower : Rat
  upper : Rat
  deriving Repr, BEq, DecidableEq

namespace Interval

def Valid (interval : Interval) : Prop :=
  interval.lower ≤ interval.upper

instance (interval : Interval) : Decidable interval.Valid := by
  unfold Valid
  infer_instance

def Contains (interval : Interval) (value : Rat) : Prop :=
  interval.lower ≤ value ∧ value ≤ interval.upper

instance (interval : Interval) (value : Rat) : Decidable (interval.Contains value) := by
  unfold Contains
  infer_instance

def ContainsReal (interval : Interval) (value : ℝ) : Prop :=
  (interval.lower : ℝ) ≤ value ∧ value ≤ (interval.upper : ℝ)

def zero : Interval := { lower := 0, upper := 0 }

def constant (value : Rat) : Interval := { lower := value, upper := value }

def hull (left right : Interval) : Interval :=
  { lower := min left.lower right.lower, upper := max left.upper right.upper }

def maxAbs (interval : Interval) : Rat :=
  max (max (-interval.lower) interval.lower) (max (-interval.upper) interval.upper)

def add (left right : Interval) : Interval :=
  { lower := left.lower + right.lower
    upper := left.upper + right.upper }

def mul (left right : Interval) : Interval :=
  { lower := min (min (left.lower * right.lower) (left.lower * right.upper))
      (min (left.upper * right.lower) (left.upper * right.upper))
    upper := max (max (left.lower * right.lower) (left.lower * right.upper))
      (max (left.upper * right.lower) (left.upper * right.upper)) }

def reciprocal (interval : Interval) : Interval :=
  { lower := min (1 / interval.lower) (1 / interval.upper)
    upper := max (1 / interval.lower) (1 / interval.upper) }

def div (numerator denominator : Interval) : Interval :=
  mul numerator (reciprocal denominator)

def pow : Interval → Nat → Interval
  | _, 0 => constant 1
  | interval, exponent + 1 => mul (pow interval exponent) interval

def ExcludesZero (interval : Interval) : Prop :=
  ¬ interval.Contains 0

instance (interval : Interval) : Decidable interval.ExcludesZero := by
  unfold ExcludesZero
  infer_instance

end Interval

def intervalEval : List Int → Interval → Interval
  | [], _ => Interval.zero
  | coefficient :: rest, domain =>
    Interval.add (Interval.constant (Rat.ofInt coefficient))
      (Interval.mul domain (intervalEval rest domain))

def excludesZero (interval : Interval) : Prop :=
  interval.ExcludesZero

instance (interval : Interval) : Decidable (excludesZero interval) := by
  unfold excludesZero
  infer_instance

def realEval : List Int → ℝ → ℝ
  | [], _ => 0
  | coefficient :: rest, value => (coefficient : ℝ) + value * realEval rest value

def IsSignChangeBracket (coefficients : List Int) (left right : Rat) : Prop :=
  left < right ∧
    ((realEval coefficients (left : ℝ) < 0 ∧ 0 < realEval coefficients (right : ℝ)) ∨
      (realEval coefficients (right : ℝ) < 0 ∧ 0 < realEval coefficients (left : ℝ)))

def IsLocalAlgebraicRoot (coefficients : List Int) (domain : Interval) : Prop :=
  ∃ root : ℝ, domain.ContainsReal root ∧ realEval coefficients root = 0

def IsUniqueRealRootIn (coefficients : List Int) (domain : Interval) : Prop :=
  ∃ root : ℝ, domain.ContainsReal root ∧ realEval coefficients root = 0 ∧
    ∀ other : ℝ, domain.ContainsReal other → realEval coefficients other = 0 → other = root

end NecoAlgebraVerify
