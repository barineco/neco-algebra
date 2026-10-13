import NecoAlgebraVerify.Semantics
import Mathlib.Analysis.Calculus.Deriv.Mul
import Mathlib.Analysis.Calculus.LocalExtr.Rolle
import Mathlib.Data.Rat.Cast.Order
import Mathlib.Topology.Algebra.Ring.Real
import Mathlib.Topology.Order.IntermediateValue

set_option autoImplicit false

namespace NecoAlgebraVerify

theorem foldl_max_ge_init {α : Type _} (f : α → Nat) (l : List α) :
    ∀ init, init ≤ l.foldl (fun d a => max d (f a)) init := by
  induction l with
  | nil => intro init; exact le_rfl
  | cons a rest ih =>
      intro init
      simp only [List.foldl_cons]
      exact le_trans (Nat.le_max_left _ _) (ih _)

theorem foldl_max_ge_mem {α : Type _} (f : α → Nat) (l : List α) (a : α)
    (h : a ∈ l) : ∀ init, f a ≤ l.foldl (fun d a => max d (f a)) init := by
  induction l with
  | nil => simp at h
  | cons b rest ih =>
      intro init
      simp only [List.mem_cons] at h
      cases h with
      | inl heq =>
          subst heq
          exact le_trans (Nat.le_max_right _ _) (foldl_max_ge_init f rest _)
      | inr hmem => exact ih hmem _

namespace Interval

theorem hull_contains_left {left right : Interval} {value : Rat}
    (contained : left.Contains value) : (hull left right).Contains value :=
  ⟨min_le_left _ _ |>.trans contained.1, contained.2.trans (le_max_left _ _)⟩

theorem hull_contains_right {left right : Interval} {value : Rat}
    (contained : right.Contains value) : (hull left right).Contains value :=
  ⟨min_le_right _ _ |>.trans contained.1, contained.2.trans (le_max_right _ _)⟩

theorem abs_le_maxAbs (interval : Interval) (value : Rat)
    (contained : interval.Contains value) :
    |value| ≤ maxAbs interval := by
  unfold maxAbs
  by_cases h : 0 ≤ value
  · rw [abs_of_nonneg h]
    exact contained.2.trans
      (le_max_of_le_right (le_max_of_le_right (le_refl _)))
  · rw [abs_of_neg (lt_of_not_ge h)]
    have hneg : -value ≤ -interval.lower := neg_le_neg contained.1
    exact hneg.trans (le_max_of_le_left (le_max_left _ _))

private theorem endpointMulContains
    {α : Type*} [Ring α] [LinearOrder α] [IsStrictOrderedRing α]
    {a b c d x y : α}
    (hax : a ≤ x) (hxb : x ≤ b) (hcy : c ≤ y) (hyd : y ≤ d) :
    min (min (a * c) (a * d)) (min (b * c) (b * d)) ≤ x * y ∧
      x * y ≤ max (max (a * c) (a * d)) (max (b * c) (b * d)) := by
  have hMinAC : min (min (a * c) (a * d)) (min (b * c) (b * d)) ≤ a * c :=
    (min_le_left _ _).trans (min_le_left _ _)
  have hMinAD : min (min (a * c) (a * d)) (min (b * c) (b * d)) ≤ a * d :=
    (min_le_left _ _).trans (min_le_right _ _)
  have hMinBC : min (min (a * c) (a * d)) (min (b * c) (b * d)) ≤ b * c :=
    (min_le_right _ _).trans (min_le_left _ _)
  have hMinBD : min (min (a * c) (a * d)) (min (b * c) (b * d)) ≤ b * d :=
    (min_le_right _ _).trans (min_le_right _ _)
  have hMaxAC : a * c ≤ max (max (a * c) (a * d)) (max (b * c) (b * d)) :=
    (le_max_left _ _).trans (le_max_left _ _)
  have hMaxAD : a * d ≤ max (max (a * c) (a * d)) (max (b * c) (b * d)) :=
    (le_max_right _ _).trans (le_max_left _ _)
  have hMaxBC : b * c ≤ max (max (a * c) (a * d)) (max (b * c) (b * d)) :=
    (le_max_left _ _).trans (le_max_right _ _)
  have hMaxBD : b * d ≤ max (max (a * c) (a * d)) (max (b * c) (b * d)) :=
    (le_max_right _ _).trans (le_max_right _ _)
  by_cases hx : 0 ≤ x
  · have hb : 0 ≤ b := hx.trans hxb
    by_cases hy : 0 ≤ y
    · have hd : 0 ≤ d := hy.trans hyd
      constructor
      · by_cases hc : 0 ≤ c
        · exact hMinAC.trans ((mul_le_mul_of_nonneg_right hax hc).trans
            (mul_le_mul_of_nonneg_left hcy hx))
        · have hc' : c ≤ 0 := (lt_of_not_ge hc).le
          exact hMinBC.trans ((mul_le_mul_of_nonpos_right hxb hc').trans
            (mul_le_mul_of_nonneg_left hcy hx))
      · exact ((mul_le_mul_of_nonneg_left hyd hx).trans
          (mul_le_mul_of_nonneg_right hxb hd)).trans hMaxBD
    · have hy' : y ≤ 0 := (lt_of_not_ge hy).le
      have hc : c ≤ 0 := hcy.trans hy'
      constructor
      · exact hMinBC.trans ((mul_le_mul_of_nonpos_right hxb hc).trans
          (mul_le_mul_of_nonneg_left hcy hx))
      · by_cases hd : d ≤ 0
        · exact ((mul_le_mul_of_nonneg_left hyd hx).trans
            (mul_le_mul_of_nonpos_right hax hd)).trans hMaxAD
        · have hd' : 0 ≤ d := (lt_of_not_ge hd).le
          exact (mul_nonpos_of_nonneg_of_nonpos hx hy').trans
            ((mul_nonneg hb hd').trans hMaxBD)
  · have hx' : x ≤ 0 := (lt_of_not_ge hx).le
    have ha : a ≤ 0 := hax.trans hx'
    by_cases hy : 0 ≤ y
    · have hd : 0 ≤ d := hy.trans hyd
      constructor
      · exact hMinAD.trans ((mul_le_mul_of_nonneg_right hax hd).trans
          (mul_le_mul_of_nonpos_left hyd hx'))
      · by_cases hb : b ≤ 0
        · exact ((mul_le_mul_of_nonneg_right hxb hy).trans
            (mul_le_mul_of_nonpos_left hcy hb)).trans hMaxBC
        · have hb' : 0 ≤ b := (lt_of_not_ge hb).le
          exact (mul_nonpos_of_nonpos_of_nonneg hx' hy).trans
            ((mul_nonneg hb' hd).trans hMaxBD)
    · have hy' : y ≤ 0 := (lt_of_not_ge hy).le
      have hc : c ≤ 0 := hcy.trans hy'
      constructor
      · by_cases hb : b ≤ 0
        · by_cases hd : d ≤ 0
          · exact hMinBD.trans ((mul_le_mul_of_nonpos_right hxb hd).trans
              (mul_le_mul_of_nonpos_left hyd hx'))
          · have hd' : 0 ≤ d := (lt_of_not_ge hd).le
            exact hMinAD.trans ((mul_nonpos_of_nonpos_of_nonneg ha hd').trans
              (mul_nonneg_of_nonpos_of_nonpos hx' hy'))
        · have hb' : 0 ≤ b := (lt_of_not_ge hb).le
          exact hMinBC.trans ((mul_nonpos_of_nonneg_of_nonpos hb' hc).trans
            (mul_nonneg_of_nonpos_of_nonpos hx' hy'))
      · exact ((mul_le_mul_of_nonpos_right hax hy').trans
          (mul_le_mul_of_nonpos_left hcy ha)).trans hMaxAC

theorem zero_valid : zero.Valid := by
  simp [zero, Valid]

theorem constant_valid (value : Rat) : (constant value).Valid := by
  simp [constant, Valid]

theorem add_valid {left right : Interval} (hleft : left.Valid) (hright : right.Valid) :
    (add left right).Valid := by
  dsimp [Valid, add] at hleft hright ⊢
  exact add_le_add hleft hright

theorem mul_valid (left right : Interval) : (mul left right).Valid := by
  dsimp [Valid, mul]
  exact ((min_le_left _ _).trans (min_le_left _ _)).trans
    ((le_max_left _ _).trans (le_max_left _ _))

theorem reciprocal_valid (interval : Interval) : (reciprocal interval).Valid := by
  dsimp [Valid, reciprocal]
  exact (min_le_left _ _).trans (le_max_left _ _)

theorem div_valid (numerator denominator : Interval) : (div numerator denominator).Valid := by
  exact mul_valid numerator (reciprocal denominator)

theorem pow_valid (interval : Interval) (exponent : Nat) : (pow interval exponent).Valid := by
  induction exponent with
  | zero => exact constant_valid 1
  | succ exponent _ => exact mul_valid (pow interval exponent) interval

theorem constant_contains (value : Rat) : (constant value).Contains value := by
  simp [constant, Contains]

theorem constant_contains_real (value : Rat) : (constant value).ContainsReal (value : ℝ) := by
  simp [constant, ContainsReal]

theorem add_contains {left right : Interval} {x y : Rat}
    (hleft : left.Contains x) (hright : right.Contains y) :
    (add left right).Contains (x + y) := by
  exact ⟨add_le_add hleft.1 hright.1, add_le_add hleft.2 hright.2⟩

theorem add_contains_real {left right : Interval} {x y : ℝ}
    (hleft : left.ContainsReal x) (hright : right.ContainsReal y) :
    (add left right).ContainsReal (x + y) := by
  constructor
  · simpa [add, ContainsReal] using add_le_add hleft.1 hright.1
  · simpa [add, ContainsReal] using add_le_add hleft.2 hright.2

theorem mul_contains {left right : Interval} {x y : Rat}
    (hleft : left.Contains x) (hright : right.Contains y) :
    (mul left right).Contains (x * y) := by
  simpa [mul, Contains] using endpointMulContains hleft.1 hleft.2 hright.1 hright.2

theorem mul_contains_real {left right : Interval} {x y : ℝ}
    (hleft : left.ContainsReal x) (hright : right.ContainsReal y) :
    (mul left right).ContainsReal (x * y) := by
  simpa [mul, ContainsReal] using endpointMulContains hleft.1 hleft.2 hright.1 hright.2

theorem reciprocal_contains {interval : Interval} {value : Rat}
    (_hvalid : interval.Valid) (hexcludesZero : interval.ExcludesZero)
    (hcontains : interval.Contains value) :
    (reciprocal interval).Contains (1 / value) := by
  have hsign : interval.upper < 0 ∨ 0 < interval.lower := by
    by_cases hupperNegative : interval.upper < 0
    · exact Or.inl hupperNegative
    · right
      have hupperNonnegative : 0 ≤ interval.upper := le_of_not_gt hupperNegative
      by_contra hlowerNonpositive
      exact hexcludesZero ⟨le_of_not_gt hlowerNonpositive, hupperNonnegative⟩
  rcases hcontains with ⟨hlower, hupper⟩
  rcases hsign with hnegative | hpositive
  · have hvalueNegative : value < 0 := lt_of_le_of_lt hupper hnegative
    have hnegativeUpper : 0 < -interval.upper := neg_pos.mpr hnegative
    have hnegativeValue : 0 < -value := neg_pos.mpr hvalueNegative
    have hleft : 1 / (-value) ≤ 1 / (-interval.upper) :=
      one_div_le_one_div_of_le hnegativeUpper (neg_le_neg hupper)
    have hright : 1 / (-interval.lower) ≤ 1 / (-value) :=
      one_div_le_one_div_of_le hnegativeValue (neg_le_neg hlower)
    have hreciprocalLower : 1 / interval.upper ≤ 1 / value := by
      simpa using neg_le_neg hleft
    have hreciprocalUpper : 1 / value ≤ 1 / interval.lower := by
      simpa using neg_le_neg hright
    exact ⟨(min_le_right _ _).trans hreciprocalLower,
      hreciprocalUpper.trans (le_max_left _ _)⟩
  · have hvaluePositive : 0 < value := lt_of_lt_of_le hpositive hlower
    have hleft : 1 / interval.upper ≤ 1 / value :=
      one_div_le_one_div_of_le hvaluePositive hupper
    have hright : 1 / value ≤ 1 / interval.lower :=
      one_div_le_one_div_of_le hpositive hlower
    exact ⟨(min_le_right _ _).trans hleft, hright.trans (le_max_left _ _)⟩

theorem div_contains {numerator denominator : Interval} {x y : Rat}
    (_hnumeratorValid : numerator.Valid) (hdenominatorValid : denominator.Valid)
    (hdenominatorExcludesZero : denominator.ExcludesZero)
    (hnumerator : numerator.Contains x) (hdenominator : denominator.Contains y) :
    (div numerator denominator).Contains (x / y) := by
  have hreciprocal := reciprocal_contains hdenominatorValid hdenominatorExcludesZero hdenominator
  simpa [div, div_eq_mul_inv] using mul_contains hnumerator hreciprocal

theorem pow_contains {interval : Interval} {value : Rat} (exponent : Nat)
    (_hvalid : interval.Valid) (hcontains : interval.Contains value) :
    (pow interval exponent).Contains (value ^ exponent) := by
  induction exponent with
  | zero => simp [pow, constant, Contains]
  | succ exponent inductionHypothesis =>
    simpa [pow, pow_succ] using mul_contains inductionHypothesis hcontains

end Interval

theorem intervalEval_valid (coefficients : List Int) (domain : Interval) :
    (intervalEval coefficients domain).Valid := by
  induction coefficients with
  | nil => exact Interval.zero_valid
  | cons coefficient rest _ =>
    exact Interval.add_valid (Interval.constant_valid (Rat.ofInt coefficient))
      (Interval.mul_valid domain (intervalEval rest domain))

theorem intervalEval_sound {coefficients : List Int} {domain : Interval} {value : Rat}
    (_hdomainValid : domain.Valid) (hcontains : domain.Contains value) :
    (intervalEval coefficients domain).Contains (eval coefficients value) := by
  induction coefficients with
  | nil => simp [intervalEval, eval, Interval.zero, Interval.Contains]
  | cons coefficient rest inductionHypothesis =>
    simpa [intervalEval, eval] using
      Interval.add_contains (Interval.constant_contains (Rat.ofInt coefficient))
        (Interval.mul_contains hcontains inductionHypothesis)

theorem intervalEval_sound_real {coefficients : List Int} {domain : Interval} {value : ℝ}
    (hcontains : domain.ContainsReal value) :
    (intervalEval coefficients domain).ContainsReal (realEval coefficients value) := by
  induction coefficients with
  | nil => simp [intervalEval, realEval, Interval.zero, Interval.ContainsReal]
  | cons coefficient rest inductionHypothesis =>
    simpa [intervalEval, realEval] using
      Interval.add_contains_real (Interval.constant_contains_real (Rat.ofInt coefficient))
        (Interval.mul_contains_real hcontains inductionHypothesis)

theorem excludesZero_sound {interval : Interval} {value : Rat}
    (hexcludesZero : excludesZero interval) (hcontains : interval.Contains value) : value ≠ 0 := by
  intro hzero
  subst value
  exact hexcludesZero hcontains

theorem excludesZero_sound_real {interval : Interval} {value : ℝ}
    (hexcludesZero : excludesZero interval) (hcontains : interval.ContainsReal value) : value ≠ 0 := by
  intro hzero
  subst value
  exact hexcludesZero ⟨Rat.cast_nonpos.mp hcontains.1, Rat.cast_nonneg.mp hcontains.2⟩

theorem intervalEval_excludesZero_sound {coefficients : List Int} {domain : Interval} {value : Rat}
    (hdomainValid : domain.Valid) (hcontains : domain.Contains value)
    (hexcludesZero : excludesZero (intervalEval coefficients domain)) :
    eval coefficients value ≠ 0 := by
  exact excludesZero_sound hexcludesZero (intervalEval_sound hdomainValid hcontains)

theorem intervalEval_excludesZero_sound_real
    {coefficients : List Int} {domain : Interval} {value : ℝ}
    (hcontains : domain.ContainsReal value)
    (hexcludesZero : excludesZero (intervalEval coefficients domain)) :
    realEval coefficients value ≠ 0 := by
  exact excludesZero_sound_real hexcludesZero (intervalEval_sound_real hcontains)

theorem eval_cast_real (coefficients : List Int) (value : Rat) :
    (eval coefficients value : ℝ) = realEval coefficients (value : ℝ) := by
  induction coefficients with
  | nil => simp [eval, realEval]
  | cons coefficient rest inductionHypothesis =>
    simp [eval, realEval, inductionHypothesis]

theorem realEval_continuous (coefficients : List Int) : Continuous (realEval coefficients) := by
  induction coefficients with
  | nil => simpa [realEval] using (continuous_const : Continuous fun _ : ℝ => (0 : ℝ))
  | cons coefficient rest inductionHypothesis =>
    exact
      (continuous_const : Continuous fun _ : ℝ => (coefficient : ℝ)).add
        (continuous_id.mul inductionHypothesis)

private theorem derivativeAlgebra (index coefficient value polynomial derivativeValue : ℝ) :
    index * coefficient + value * ((index + 1) * polynomial + value * derivativeValue) =
      index * (coefficient + value * polynomial) +
        value * (polynomial + value * derivativeValue) := by
  have hscaled :
      value * ((index + 1) * polynomial) =
        index * (value * polynomial) + value * polynomial := by
    calc
      value * ((index + 1) * polynomial) =
          value * (index * polynomial + 1 * polynomial) := by rw [add_mul]
      _ = value * (index * polynomial + polynomial) := by rw [one_mul]
      _ = value * (index * polynomial) + value * polynomial := by rw [mul_add]
      _ = index * (value * polynomial) + value * polynomial := by
        rw [← mul_assoc value index polynomial, mul_comm value index,
          mul_assoc index value polynomial]
  calc
    index * coefficient + value * ((index + 1) * polynomial + value * derivativeValue) =
        index * coefficient +
          ((index * (value * polynomial) + value * polynomial) +
            value * (value * derivativeValue)) := by rw [mul_add, hscaled]
    _ = (index * coefficient + index * (value * polynomial)) +
        (value * polynomial + value * (value * derivativeValue)) := by ac_rfl
    _ = index * (coefficient + value * polynomial) +
        value * (polynomial + value * derivativeValue) := by rw [mul_add, mul_add]

private theorem realEval_derivativeFrom
    (coefficients : List Int) (value : ℝ) (index : Nat) :
    realEval (derivativeFrom index coefficients) value =
      (index : ℝ) * realEval coefficients value +
        value * realEval (derivative coefficients) value := by
  induction coefficients generalizing index with
  | nil => simp [derivativeFrom, derivative, realEval]
  | cons coefficient rest inductionHypothesis =>
    have hDerivativeCons :
        realEval (derivative (coefficient :: rest)) value =
          realEval rest value + value * realEval (derivative rest) value := by
      change realEval (derivativeFrom 1 rest) value = _
      simpa using inductionHypothesis 1
    simp only [derivativeFrom, realEval, Int.cast_mul, Int.ofNat_eq_natCast,
      Int.cast_natCast]
    rw [inductionHypothesis (index + 1), hDerivativeCons]
    simpa only [Nat.cast_add, Nat.cast_one] using
      derivativeAlgebra (index : ℝ) (coefficient : ℝ) value
        (realEval rest value) (realEval (derivative rest) value)

theorem realEval_hasDerivAt (coefficients : List Int) (value : ℝ) :
    HasDerivAt (realEval coefficients) (realEval (derivative coefficients) value) value := by
  induction coefficients with
  | nil =>
    simpa [realEval, derivative, derivativeFrom] using
      (hasDerivAt_const value (0 : ℝ))
  | cons coefficient rest inductionHypothesis =>
    have hDerivativeCons :
        realEval (derivative (coefficient :: rest)) value =
          realEval rest value + value * realEval (derivative rest) value := by
      change realEval (derivativeFrom 1 rest) value = _
      simpa using realEval_derivativeFrom rest value 1
    rw [hDerivativeCons]
    simpa [realEval] using
      ((hasDerivAt_id' value).mul inductionHypothesis).const_add (coefficient : ℝ)

theorem signChangeBracket_of_eval {coefficients : List Int} {left right : Rat}
    (hordered : left < right) (hsignChange : signChange (eval coefficients left) (eval coefficients right)) :
    IsSignChangeBracket coefficients left right := by
  refine ⟨hordered, ?_⟩
  rcases hsignChange with hnegativeToPositive | hpositiveToNegative
  · left
    constructor
    · rw [← eval_cast_real]
      exact Rat.cast_lt_zero.mpr hnegativeToPositive.1
    · rw [← eval_cast_real]
      exact Rat.cast_pos.mpr hnegativeToPositive.2
  · right
    constructor
    · rw [← eval_cast_real]
      exact Rat.cast_lt_zero.mpr hpositiveToNegative.1
    · rw [← eval_cast_real]
      exact Rat.cast_pos.mpr hpositiveToNegative.2

theorem signChangeBracket_root_exists {coefficients : List Int} {left right : Rat}
    (hbracket : IsSignChangeBracket coefficients left right) :
    IsLocalAlgebraicRoot coefficients { lower := left, upper := right } := by
  rcases hbracket with ⟨hordered, hsignChange⟩
  have horderedReal : (left : ℝ) ≤ (right : ℝ) := Rat.cast_le.mpr hordered.le
  have hcontinuous : ContinuousOn (realEval coefficients)
      (Set.Icc (left : ℝ) (right : ℝ)) :=
    (realEval_continuous coefficients).continuousOn
  rcases hsignChange with hnegativeToPositive | hpositiveToNegative
  · have hzeroInImage : (0 : ℝ) ∈ (realEval coefficients) ''
        Set.Icc (left : ℝ) (right : ℝ) :=
      intermediate_value_Icc horderedReal hcontinuous
        ⟨hnegativeToPositive.1.le, hnegativeToPositive.2.le⟩
    rcases hzeroInImage with ⟨root, hrootDomain, hrootValue⟩
    exact ⟨root, hrootDomain, hrootValue⟩
  · have hzeroInImage : (0 : ℝ) ∈ (realEval coefficients) ''
        Set.Icc (left : ℝ) (right : ℝ) :=
      intermediate_value_Icc' horderedReal hcontinuous
        ⟨hpositiveToNegative.1.le, hpositiveToNegative.2.le⟩
    rcases hzeroInImage with ⟨root, hrootDomain, hrootValue⟩
    exact ⟨root, hrootDomain, hrootValue⟩

theorem signChangeBracket_uniqueRoot
    {coefficients : List Int} {left right : Rat}
    (hbracket : IsSignChangeBracket coefficients left right)
    (hderivativeExcludesZero :
      excludesZero (intervalEval (derivative coefficients) { lower := left, upper := right })) :
    IsUniqueRealRootIn coefficients { lower := left, upper := right } := by
  rcases signChangeBracket_root_exists hbracket with ⟨root, hrootDomain, hrootValue⟩
  refine ⟨root, hrootDomain, hrootValue, ?_⟩
  intro other hotherDomain hotherValue
  by_contra hne
  rcases lt_or_gt_of_ne hne with hotherRoot | hrootOther
  · rcases exists_hasDerivAt_eq_zero hotherRoot
        (realEval_continuous coefficients).continuousOn
        (hotherValue.trans hrootValue.symm)
        (fun value _ => realEval_hasDerivAt coefficients value) with
      ⟨critical, hcritical, hcriticalZero⟩
    have hcriticalDomain :
        ({ lower := left, upper := right } : Interval).ContainsReal critical :=
      ⟨hotherDomain.1.trans hcritical.1.le, hcritical.2.le.trans hrootDomain.2⟩
    exact intervalEval_excludesZero_sound_real hcriticalDomain hderivativeExcludesZero
      hcriticalZero
  · rcases exists_hasDerivAt_eq_zero hrootOther
        (realEval_continuous coefficients).continuousOn
        (hrootValue.trans hotherValue.symm)
        (fun value _ => realEval_hasDerivAt coefficients value) with
      ⟨critical, hcritical, hcriticalZero⟩
    have hcriticalDomain :
        ({ lower := left, upper := right } : Interval).ContainsReal critical :=
      ⟨hrootDomain.1.trans hcritical.1.le, hcritical.2.le.trans hotherDomain.2⟩
    exact intervalEval_excludesZero_sound_real hcriticalDomain hderivativeExcludesZero
      hcriticalZero

end NecoAlgebraVerify
