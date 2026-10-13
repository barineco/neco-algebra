import NecoLean.Json

set_option autoImplicit false

namespace NecoAlgebraVerify

open NecoLean.Json

abbrev JsonFields := List (String × Value)

def jsonFields? : Value → Option JsonFields
  | .object fields => some fields
  | _ => none

def jsonString? : Value → Option String
  | .string text => some text
  | _ => none

def jsonNat? : Value → Option Nat
  | .number number => number.toString.toNat?
  | _ => none

def jsonField? (fields : JsonFields) (name : String) : Option Value :=
  (fields.find? fun field => field.1 == name).map Prod.snd

private def distinctNames : List String → Bool
  | [] => true
  | name :: names => !names.contains name && distinctNames names

def hasExactFields (fields : JsonFields) (names : List String) : Bool :=
  distinctNames names &&
    fields.length == names.length &&
    fields.all (fun field => names.contains field.1) &&
    names.all (fun name => fields.any fun field => field.1 == name)

def jsonObjectWithExactFields? (value : Value) (names : List String) : Option JsonFields := do
  let fields ← jsonFields? value
  if hasExactFields fields names then some fields else none

def jsonStringField? (fields : JsonFields) (name : String) : Option String := do
  jsonString? (← jsonField? fields name)

def jsonNatField? (fields : JsonFields) (name : String) : Option Nat := do
  jsonNat? (← jsonField? fields name)

def decodeDecimalInteger (text : String) : Option Int :=
  let characters := text.toList
  let digits (remaining : List Char) : Option Nat :=
    if remaining.isEmpty || !(remaining.all fun character => character >= '0' && character <= '9') then none
    else if remaining.length > 1 && remaining.head? == some '0' then none
    else String.ofList remaining |>.toNat?
  match characters with
  | '-' :: remaining =>
    if remaining == ['0'] then none else (digits remaining).map fun value => -Int.ofNat value
  | remaining => (digits remaining).map Int.ofNat

def decodeUnsignedNat (text : String) : Option Nat :=
  match decodeDecimalInteger text with
  | some (.ofNat value) => some value
  | _ => none

def decodePositiveNat (text : String) : Option Nat :=
  match decodeUnsignedNat text with
  | some (value + 1) => some (value + 1)
  | _ => none

def decodeDyadicRat (value : Value) : Option Rat := do
  let fields ← jsonObjectWithExactFields? value ["exponent", "integer"]
  let integer ← decodeDecimalInteger (← jsonStringField? fields "integer")
  let exponent ← jsonNatField? fields "exponent"
  pure (Rat.ofInt integer / (2 ^ exponent : Rat))

def decodeNormalizedRational (value : Value) : Option Rat := do
  let fields ← jsonObjectWithExactFields? value ["denominator", "numerator"]
  let numerator ← decodeDecimalInteger (← jsonStringField? fields "numerator")
  let denominator ← decodePositiveNat (← jsonStringField? fields "denominator")
  if Int.gcd numerator denominator == 1 then
    pure (Rat.ofInt numerator / Rat.ofInt (Int.ofNat denominator))
  else
    none

end NecoAlgebraVerify
