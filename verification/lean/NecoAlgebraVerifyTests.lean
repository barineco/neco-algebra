import NecoAlgebraVerify
import Mathlib.Tactic.NormNum

open NecoAlgebraVerify

private def verified (result : CheckResult) : Bool :=
  !result.cases.isEmpty && result.cases.all (fun item => item.verdict == .verified)

private def expect (name : String) (actual expected : Verdict) : IO Unit :=
  if actual == expected then pure () else throw (IO.userError s!"{name}: unexpected verdict")

private def expectBool (name : String) (condition : Bool) : IO Unit :=
  if condition then pure () else throw (IO.userError s!"{name}: condition failed")

private def one (json : String) : Verdict :=
  (checkBytes json.toUTF8).cases.head?.map CaseResult.verdict |>.getD .invalid

private def testCanonical : IO Unit := do
  expectBool "decimal zero" (decodeDecimalInteger "0" == some (0 : Int))
  expectBool "decimal negative" (decodeDecimalInteger "-12" == some (-12 : Int))
  expectBool "negative zero rejected" (decodeDecimalInteger "-0" == none)
  expectBool "leading zero rejected" (decodeDecimalInteger "01" == none)
  expectBool "empty rejected" (decodeDecimalInteger "" == none)
  expectBool "positive zero rejected" (decodePositiveNat "0" == none)
  let zeroDenominator : NecoLean.Json.Value :=
    .object [("denominator", .string "0"), ("numerator", .string "1")]
  let unreduced : NecoLean.Json.Value :=
    .object [("denominator", .string "4"), ("numerator", .string "2")]
  let reduced : NecoLean.Json.Value :=
    .object [("denominator", .string "2"), ("numerator", .string "1")]
  expectBool "zero denominator rejected" (decodeNormalizedRational zeroDenominator == none)
  expectBool "unreduced rational rejected" (decodeNormalizedRational unreduced == none)
  expectBool "reduced rational accepted"
    (decodeNormalizedRational reduced == some (1 / 2 : Rat))
  IO.println "canonical tests passed"

private def testInterval : IO Unit := do
  let left : Interval := { lower := -2, upper := 3 }
  let right : Interval := { lower := 1, upper := 4 }
  let denominator : Interval := { lower := 2, upper := 4 }
  let x : Rat := 1
  let y : Rat := 2
  expectBool "interval validity" (decide (left.Valid ∧ right.Valid ∧ denominator.Valid))
  expectBool "addition containment" (decide ((Interval.add left right).Contains (x + y)))
  expectBool "multiplication containment" (decide ((Interval.mul left right).Contains (x * y)))
  expectBool "division containment"
    (decide ((Interval.div left denominator).Contains (x / y)))
  expectBool "power containment" (decide ((Interval.pow right 3).Contains (y ^ 3)))
  expectBool "division excludes zero" (decide (denominator.ExcludesZero))
  IO.println "interval tests passed"

private def testPolynomial : IO Unit := do
  let coefficients : List Int := [-1, 2, 1]
  let domain : Interval := { lower := -1, upper := 2 }
  let value : Rat := 1
  expectBool "rational evaluation" (eval coefficients value == 2)
  expectBool "interval evaluation containment"
    (decide ((intervalEval coefficients domain).Contains (eval coefficients value)))
  expectBool "interval evaluation valid" (decide (intervalEval coefficients domain).Valid)
  IO.println "polynomial tests passed"

private def testRoots : IO Unit := do
  let coefficients : List Int := [-1, 1]
  let left : Rat := 0
  let right : Rat := 2
  let domain : Interval := { lower := left, upper := right }
  expectBool "sign change" (decide (signChange (eval coefficients left) (eval coefficients right)))
  expectBool "derivative excludes zero"
    (decide (excludesZero (intervalEval (derivative coefficients) domain)))
  expectBool "root bracket predicate" (rootPredicate coefficients left right)
  IO.println "root tests passed"

private def testKnown : IO Unit := do
  let bytes ← IO.FS.readBinFile "../protocol/v1/algebra-observation-batch.json"
  if verified (checkBytes bytes) then IO.println "known tests passed" else throw (IO.userError "known batch")

private def testAdversarial : IO Unit := do
  expect "tampered" (one "{\"cases\":[{\"caseId\":\"bad\",\"input\":{\"denominator\":\"6\",\"numerator\":\"8\"},\"observation\":{\"kind\":\"success\",\"value\":{\"denominator\":\"3\",\"numerator\":\"5\"}},\"operation\":\"rational-reduction\"}],\"schema\":\"neco.algebra.observation-batch.v1\"}") .refuted
  expect "empty case id" (one "{\"cases\":[{\"caseId\":\"\",\"input\":{\"denominator\":\"6\",\"numerator\":\"8\"},\"observation\":{\"kind\":\"success\",\"value\":{\"denominator\":\"3\",\"numerator\":\"4\"}},\"operation\":\"rational-reduction\"}],\"schema\":\"neco.algebra.observation-batch.v1\"}") .invalid
  expect "invalid" (one "{ \"cases\":[],\"schema\":\"neco.algebra.observation-batch.v1\"}") .invalid
  IO.println "adversarial tests passed"

private theorem linearRootSoundnessWitness :
    IsUniqueRealRootIn [-1, 1] { lower := 0, upper := 2 } := by
  exact signChangeBracket_uniqueRoot
    (signChangeBracket_of_eval (by norm_num) (by norm_num [signChange, eval]))
    (by
      norm_num [excludesZero, Interval.ExcludesZero, Interval.Contains, intervalEval,
        derivative, derivativeFrom, Interval.add, Interval.mul, Interval.zero,
        Interval.constant])

private theorem cubicRootSoundnessWitness :
    IsUniqueRealRootIn [-2, 0, 0, 1] { lower := 1, upper := 2 } := by
  exact signChangeBracket_uniqueRoot
    (signChangeBracket_of_eval (by norm_num) (by norm_num [signChange, eval]))
    (by
      norm_num [excludesZero, Interval.ExcludesZero, Interval.Contains, intervalEval,
        derivative, derivativeFrom, Interval.add, Interval.mul, Interval.zero,
        Interval.constant])

private def testSoundness : IO Unit := IO.println "soundness theorems compiled"

private def checkPath (path : String) : IO Unit := do
  let bytes ← if path == "-" then
    (← IO.getStdin).readBinToEnd
  else IO.FS.readBinFile path
  IO.println (String.fromUTF8! (resultBytes (checkBytes bytes)))

private def testAll : IO Unit := do
  testCanonical
  testInterval
  testPolynomial
  testRoots
  testKnown
  testAdversarial
  testSoundness

def main (arguments : List String) : IO Unit :=
  let command := if arguments.head? == some "--" then arguments.drop 1 else arguments
  match command with
  | ["canonical"] => testCanonical
  | ["interval"] => testInterval
  | ["polynomial"] => testPolynomial
  | ["roots"] => testRoots
  | ["known"] => testKnown
  | ["soundness"] => testSoundness
  | ["all"] => testAll
  | ["check", path] => checkPath path
  | _ => throw (IO.userError "usage: neco-algebra-checker <canonical|interval|polynomial|roots|known|soundness|all|check PATH>")
