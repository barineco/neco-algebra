import NecoLean.Json
import NecoLean.Digest
import NecoAlgebraVerify.Canonical
import NecoAlgebraVerify.Semantics

set_option autoImplicit false

namespace NecoAlgebraVerify

open NecoLean.Json

inductive Verdict where
  | verified | refuted | undetermined | invalid
  deriving Repr, BEq, DecidableEq

structure CaseResult where
  caseId : String
  verdict : Verdict
  deriving Repr, BEq

structure CheckResult where
  digest : String
  cases : List CaseResult
  deriving Repr, BEq

private def decodeNormalizedRationals (v : Value) : Option (List Rat) :=
  match v with
  | .array xs => xs.mapM decodeNormalizedRational
  | _ => none

private def decodeNormalizedMatrix (v : Value) : Option (List (List Rat)) :=
  match v with
  | .array xs => xs.mapM decodeNormalizedRationals
  | _ => none

private def hexNat (s : String) : Option Nat :=
  s.toList.foldl (fun acc c => do
    let x ← acc
    let d := if c >= '0' && c <= '9' then some (c.toNat - '0'.toNat)
      else if c >= 'a' && c <= 'f' then some (10 + c.toNat - 'a'.toNat) else none
    pure (x * 16 + (← d))) (some 0)

private def f64Dyadic (s : String) : Option Rat := do
  if s.length != 16 || !(s.toList.all fun c => ('0' <= c && c <= '9') || ('a' <= c && c <= 'f')) then none else pure ()
  let w ← hexNat s
  let sign : Int := if w / 2^63 == 0 then 1 else -1
  let exponent := (w / 2^52) % 2048
  let fraction := w % 2^52
  if exponent == 2047 then none
  else if exponent == 0 then pure (Rat.ofInt (sign * Int.ofNat fraction) / (2^1074 : Rat))
  else
    let significand := Rat.ofInt (sign * Int.ofNat (2^52 + fraction))
    let scale : Int := Int.ofNat exponent - 1023 - 52
    if scale < 0 then pure (significand / (2 ^ scale.natAbs : Rat))
    else pure (significand * (2 ^ scale.natAbs : Rat))

def dot (x y : List Rat) : Rat := (List.zipWith (· * ·) x y).sum
def mulVec (a : List (List Rat)) (x : List Rat) : List Rat := a.map fun row => dot row x
def isRectangular (a : List (List Rat)) : Bool :=
  match a with | [] => true | r :: rs => rs.all fun s => s.length == r.length
def transpose (a : List (List Rat)) : List (List Rat) :=
  match a with | [] => [] | r :: _ => (List.range r.length).map fun j => a.map fun row => row.getD j 0

def hasWidth (a : List (List Rat)) (width : Nat) : Bool :=
  a.all fun row => row.length == width

private def nonzeroPivot? (rows : List (List Rat)) (col : Nat) : Option (List Rat × List (List Rat)) :=
  match rows.find? (fun r => r.getD col 0 != 0) with
  | none => none
  | some p => some (p, rows.erase p)

private partial def rankAux (rows : List (List Rat)) (col width : Nat) : Nat :=
  if col >= width then 0 else
  match nonzeroPivot? rows col with
  | none => rankAux rows (col + 1) width
  | some (p, rest) =>
    let q := p.getD col 0
    let next := rest.map fun r => r.zipWith (fun x y => x - r.getD col 0 / q * y) p
    rankAux next (col + 1) width + 1

def rank (a : List (List Rat)) : Nat :=
  if !isRectangular a then 0 else rankAux a 0 (a.head?.map List.length |>.getD 0)

def reductionPredicate (numerator : Int) (denominator : Nat) (reducedNumerator : Int) (reducedDenominator : Nat) : Bool :=
  denominator != 0 && Rat.ofInt numerator / Rat.ofInt (Int.ofNat denominator) == Rat.ofInt reducedNumerator / Rat.ofInt (Int.ofNat reducedDenominator) && Int.gcd reducedNumerator reducedDenominator == 1

def bezoutPredicate (left right gcd coefficientLeft coefficientRight : Int) : Bool :=
  gcd >= 0 && gcd == Int.gcd left right && left * coefficientLeft + right * coefficientRight == gcd

def rootPredicate (coefficients : List Int) (left right : Rat) : Bool :=
  left < right && eval coefficients left != 0 && eval coefficients right != 0 &&
    decide (signChange (eval coefficients left) (eval coefficients right)) &&
    decide (excludesZero (intervalEval (derivative coefficients) { lower := left, upper := right }))

def ratAbs (x : Rat) : Rat := if x < 0 then -x else x

def projectionPredicate (precision : Nat) (input lower upper error projected : Rat) : Bool :=
  lower <= projected && projected <= upper && upper - lower <= 1 / (2 ^ precision : Rat) && error == max (ratAbs (projected - lower)) (ratAbs (upper - projected)) && lower <= input && input <= upper

def uniquePredicate (matrix : List (List Rat)) (rhs solution : List Rat) : Bool :=
  let width := matrix.head?.map List.length |>.getD 0
  isRectangular matrix && matrix.length == rhs.length && hasWidth matrix width && solution.length == width && mulVec matrix solution == rhs && rank matrix == width

def affinePredicate (matrix : List (List Rat)) (rhs particular : List Rat) (basis : List (List Rat)) : Bool :=
  let width := matrix.head?.map List.length |>.getD 0
  let columns := transpose basis
  isRectangular matrix && matrix.length == rhs.length && hasWidth matrix width && particular.length == width && basis.all (fun vector => vector.length == width) && mulVec matrix particular == rhs && basis.all (fun vector => mulVec matrix vector == List.replicate matrix.length 0) && rank columns == basis.length

private def checkSuccess (family : String) (input value : Value) : Option Bool :=
  match family with
  | "rational-reduction" => do
    let i ← jsonObjectWithExactFields? input ["denominator", "numerator"]
    let o ← jsonObjectWithExactFields? value ["denominator", "numerator"]
    let n ← decodeDecimalInteger (← jsonStringField? i "numerator"); let d ← decodeUnsignedNat (← jsonStringField? i "denominator")
    let on ← decodeDecimalInteger (← jsonStringField? o "numerator"); let od ← decodePositiveNat (← jsonStringField? o "denominator")
    pure (reductionPredicate n d on od)
  | "extended-gcd-bezout" => do
    let i ← jsonObjectWithExactFields? input ["left", "right"]; let o ← jsonObjectWithExactFields? value ["bezoutLeft", "bezoutRight", "gcd"]
    let a ← decodeDecimalInteger (← jsonStringField? i "left"); let b ← decodeDecimalInteger (← jsonStringField? i "right")
    let g ← decodeDecimalInteger (← jsonStringField? o "gcd"); let x ← decodeDecimalInteger (← jsonStringField? o "bezoutLeft"); let y ← decodeDecimalInteger (← jsonStringField? o "bezoutRight")
    pure (bezoutPredicate a b g x y)
  | "local-algebraic-root" => do
    let i ← jsonObjectWithExactFields? input ["polynomial"]; let o ← jsonObjectWithExactFields? value ["left", "right"]
    let p ← (← jsonField? i "polynomial") |> (fun v => match v with | .array xs => xs.mapM (fun z => jsonString? z >>= decodeDecimalInteger) | _ => none)
    rootPredicate p (← decodeDyadicRat (← jsonField? o "left")) (← decodeDyadicRat (← jsonField? o "right")) |> some
  | "scalar-f64-projection" => do
    let i ← jsonObjectWithExactFields? input ["bits", "value"]; let o ← jsonObjectWithExactFields? value ["absoluteError", "bits", "lower", "upper"]
    let bits ← jsonNatField? i "bits"; let x ← decodeDyadicRat (← jsonField? i "value"); let lo ← decodeDyadicRat (← jsonField? o "lower"); let hi ← decodeDyadicRat (← jsonField? o "upper"); let err ← decodeDyadicRat (← jsonField? o "absoluteError")
    let f ← f64Dyadic (← jsonStringField? o "bits")
    pure (projectionPredicate bits x lo hi err f)
  | "rational-linear-unique" => do
    let i ← jsonObjectWithExactFields? input ["matrix", "rhs"]; let o ← jsonObjectWithExactFields? value ["solution"]
    match decodeNormalizedMatrix (← jsonField? i "matrix"), decodeNormalizedRationals (← jsonField? i "rhs"), decodeNormalizedRationals (← jsonField? o "solution") with
    | some a, some b, some x =>
      pure (uniquePredicate a b x)
    | _, _, _ => pure false
  | "rational-linear-affine" => do
    let i ← jsonObjectWithExactFields? input ["matrix", "rhs"]; let o ← jsonObjectWithExactFields? value ["basis", "particular"]
    let basis := match ← jsonField? o "basis" with | .array xs => xs.mapM decodeNormalizedRationals | _ => none
    match decodeNormalizedMatrix (← jsonField? i "matrix"), decodeNormalizedRationals (← jsonField? i "rhs"), decodeNormalizedRationals (← jsonField? o "particular"), basis with
    | some a, some b, some p, some ks =>
      pure (affinePredicate a b p ks)
    | _, _, _, _ => pure false
  | _ => none

private def constantNonzero (coeffs : List Int) : Bool :=
  coeffs.head?.getD 0 != 0 && (coeffs.drop 1).all (· == 0)

private def linearFailureMatches (input : Value) (failure : String) : Option Bool := do
  let xs ← jsonObjectWithExactFields? input ["matrix", "rhs"]
  let a ← decodeNormalizedMatrix (← jsonField? xs "matrix")
  let b ← decodeNormalizedRationals (← jsonField? xs "rhs")
  let width := a.head?.map List.length |>.getD 0
  let dimensionMismatch := !isRectangular a || a.length != b.length
  let singular := !dimensionMismatch && a.length == width && rank a < width
  pure ((failure == "dimension-mismatch" && dimensionMismatch) || (failure == "singular-system" && singular))

private def failureMatches (family : String) (input failure : Value) : Option Bool := do
  let f ← jsonString? failure
  match family with
  | "rational-reduction" => do
    let xs ← jsonObjectWithExactFields? input ["denominator", "numerator"]
    pure (f == "zero-denominator" && decodeUnsignedNat (← jsonStringField? xs "denominator") == some 0)
  | "local-algebraic-root" => do
    let xs ← jsonObjectWithExactFields? input ["polynomial"]
    let coeffs ← (← jsonField? xs "polynomial") |> (fun v => match v with | .array ys => ys.mapM (fun z => jsonString? z >>= decodeDecimalInteger) | _ => none)
    if f != "no-local-root" then some false
    else if constantNonzero coeffs then some true else none
  | "scalar-f64-projection" => do
    let xs ← jsonObjectWithExactFields? input ["bits", "value"]
    let value ← decodeDyadicRat (← jsonField? xs "value")
    let maximumFinite ← f64Dyadic "7fefffffffffffff"
    pure (f == "non-finite-f64" && ratAbs value > maximumFinite)
  | "rational-linear-unique" | "rational-linear-affine" => linearFailureMatches input f
  | _ => pure false

private def declaredVerdict? (v : Value) : Option Verdict := do
  match ← jsonString? v with
  | "verified" => some .verified | "refuted" => some .refuted
  | "undetermined" => some .undetermined | "invalid" => some .invalid | _ => none

private def checkCase (v : Value) : CaseResult :=
  let invalid := { caseId := "", verdict := .invalid }
  match jsonObjectWithExactFields? v ["caseId", "input", "observation", "operation"] with
  | none => invalid
  | some e =>
    match jsonStringField? e "caseId", jsonStringField? e "operation", jsonField? e "input", jsonField? e "observation" with
    | some id, some family, some input, some observation =>
      if id.isEmpty then invalid else
          match family with
          | "rational-reduction" | "extended-gcd-bezout" | "local-algebraic-root" | "scalar-f64-projection" | "rational-linear-unique" | "rational-linear-affine" =>
            match jsonObjectWithExactFields? observation ["kind", "value"], jsonObjectWithExactFields? observation ["failure", "kind"] with
              | some so, _ =>
                if jsonStringField? so "kind" != some "success" then { caseId := id, verdict := .invalid }
                else match (jsonField? so "value").bind (checkSuccess family input) with
                | none => { caseId := id, verdict := .invalid }
                  | some true => { caseId := id, verdict := .verified }
                  | some false => { caseId := id, verdict := .refuted }
              | _, some fo =>
                if jsonStringField? fo "kind" != some "failure" then { caseId := id, verdict := .invalid }
                else match (jsonField? fo "failure").bind (failureMatches family input) with
                  | some true => { caseId := id, verdict := .verified }
                  | some false => { caseId := id, verdict := .refuted }
                  | none => { caseId := id, verdict := .undetermined }
              | _, _ => { caseId := id, verdict := .invalid }
          | _ => { caseId := id, verdict := .invalid }
    | _, _, _, _ => invalid

private def jsonWhitespace (character : Char) : Bool :=
  character == ' ' || character == '\n' || character == '\r' || character == '\t'

private partial def scanBatchCases (remaining : List Char) (current : List Char) (cases : List (List Char)) (depth : Nat) (inString escaped : Bool) : Option (List (List Char)) :=
  match remaining with
  | [] =>
    if depth == 0 && !inString && !current.isEmpty then some (cases.reverse ++ [current.reverse]) else none
  | character :: rest =>
    if inString then
      if escaped then scanBatchCases rest (character :: current) cases depth true false
      else if character == '\\' then scanBatchCases rest (character :: current) cases depth true true
      else if character == '"' then scanBatchCases rest (character :: current) cases depth false false
      else if character.toNat < 32 then none
      else scanBatchCases rest (character :: current) cases depth true false
    else if jsonWhitespace character then none
    else if character == '"' then scanBatchCases rest (character :: current) cases depth true false
    else if character == '{' || character == '[' then scanBatchCases rest (character :: current) cases (depth + 1) false false
    else if character == '}' || character == ']' then
      if depth == 0 then none else scanBatchCases rest (character :: current) cases (depth - 1) false false
    else if character == ',' && depth == 0 then
      if current.isEmpty then none else scanBatchCases rest [] (current.reverse :: cases) 0 false false
    else scanBatchCases rest (character :: current) cases depth false false

private def arrayCaseBytes? (bytes : ByteArray) : Option (List ByteArray) := do
  let text ← String.fromUTF8? bytes
  match text.toList with
  | '[' :: rest =>
    match rest.reverse with
    | ']' :: reversedBody =>
      let body := reversedBody.reverse
      if body.isEmpty then some []
      else (← scanBatchCases body [] [] 0 false false).map (fun chars => String.ofList chars |>.toUTF8)
    | _ => none
  | _ => none

private def batchCaseBytes? (bytes : ByteArray) : Option (List ByteArray) := do
  let text ← String.fromUTF8? bytes
  let startText := "{\"cases\":"
  let endText := ",\"schema\":\"neco.algebra.observation-batch.v1\"}"
  if !text.startsWith startText || !text.endsWith endText then none
  else
    let body := ((text.toList.drop startText.toList.length).reverse.drop endText.toList.length).reverse
    arrayCaseBytes? (String.ofList body).toUTF8

private def checkCaseBytes (bytes : ByteArray) : CaseResult :=
  match NecoLean.Json.decode bytes with
  | .ok value =>
    match NecoLean.Json.encode value with
    | .ok encoded => if encoded == bytes then checkCase value else { caseId := "", verdict := .invalid }
    | .error _ => { caseId := "", verdict := .invalid }
  | .error _ => { caseId := "", verdict := .invalid }

private def uniqueCaseIds : List CaseResult → Bool
  | [] => true
  | item :: rest => !item.caseId.isEmpty && !(rest.any fun candidate => candidate.caseId == item.caseId) && uniqueCaseIds rest

private def checkedBatch (digest : String) (cases : List CaseResult) : CheckResult :=
  if uniqueCaseIds cases then { digest, cases }
  else { digest, cases := [{ caseId := "", verdict := .invalid }] }

/-- Checks a canonical protocol batch without any file or process effects. -/
def checkBytes (bytes : ByteArray) : CheckResult :=
  let digest := NecoLean.Digest.toHex (NecoLean.Digest.sha256 bytes)
  match NecoLean.Json.decode bytes with
  | .ok (.object fields) =>
    match NecoLean.Json.encode (.object fields), jsonObjectWithExactFields? (.object fields) ["cases", "schema"], jsonField? fields "cases", jsonStringField? fields "schema" with
    | .ok encoded, some _, some (.array cases), some "neco.algebra.observation-batch.v1" =>
      if encoded == bytes then checkedBatch digest (cases.map checkCase) else { digest, cases := [{ caseId := "", verdict := .invalid }] }
    | _, _, _, _ => { digest, cases := [{ caseId := "", verdict := .invalid }] }
  | _ =>
    match batchCaseBytes? bytes with
    | some cases => checkedBatch digest (cases.map checkCaseBytes)
    | none => { digest, cases := [{ caseId := "", verdict := .invalid }] }

private def verdictText : Verdict → String
  | .verified => "verified"
  | .refuted => "refuted"
  | .undetermined => "undetermined"
  | .invalid => "invalid"

def resultJson (result : CheckResult) : Value :=
  .object [
    ("cases", .array (result.cases.map fun item =>
      .object [("caseId", .string item.caseId), ("verdict", .string (verdictText item.verdict))])),
    ("digest", .string result.digest)
  ]

def resultBytes (result : CheckResult) : ByteArray :=
  match NecoLean.Json.encode (resultJson result) with
  | .ok bytes => bytes
  | .error _ => "{\"cases\":[],\"digest\":\"\"}".toUTF8

end NecoAlgebraVerify
