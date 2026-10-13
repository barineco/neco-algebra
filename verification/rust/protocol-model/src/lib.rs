#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sha2::{Digest, Sha256};

pub const BATCH_SCHEMA: &str = "neco.algebra.observation-batch.v1";
pub const REQUEST_BATCH_SCHEMA: &str = "neco.verification.generated-request-batch.v1";
pub const DESCRIPTOR_SCHEMA: &str = "neco.verification.producer-description.v1";
pub const ALGEBRA_DOMAIN: &str = "neco.algebra";
pub const ADAPTER_REVISION: &str = "algebra-generated-request-v2";
pub const GENERATOR_ALGORITHM: &str = "splitmix64-v1";
pub const DRAW_SCHEDULE_REVISION: &str = "draw-schedule-v1";
pub const SELECTOR_NAMES: [&str; 9] = [
    "value",
    "familyParameter",
    "observationClass",
    "sign",
    "multiplicity",
    "intervalWidth",
    "formulaVariant",
    "mutation",
    "reserved",
];

pub fn object(fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}
pub fn decimal(value: impl ToString) -> Value {
    Value::String(value.to_string())
}
pub fn dyadic(integer: impl ToString, exponent: u32) -> Value {
    object([
        ("exponent", Value::Number(exponent.into())),
        ("integer", decimal(integer)),
    ])
}
pub fn rational(numerator: impl ToString, denominator: impl ToString) -> Value {
    object([
        ("denominator", decimal(denominator)),
        ("numerator", decimal(numerator)),
    ])
}
pub fn observation(
    case_id: String,
    input: Value,
    observation: Value,
    operation: &'static str,
) -> Value {
    object([
        ("caseId", Value::String(case_id)),
        ("input", input),
        ("observation", observation),
        ("operation", Value::String(operation.to_owned())),
    ])
}
pub fn success(value: Value) -> Value {
    object([
        ("kind", Value::String("success".to_owned())),
        ("value", value),
    ])
}
pub fn failure(kind: &'static str) -> Value {
    object([
        ("failure", Value::String(kind.to_owned())),
        ("kind", Value::String("failure".to_owned())),
    ])
}
pub fn canonical_bytes(cases: &[Value]) -> Vec<u8> {
    canonical_value_bytes(&object([
        ("cases", Value::Array(cases.to_vec())),
        ("schema", Value::String(BATCH_SCHEMA.to_owned())),
    ]))
}
pub fn canonical_value_bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("JSON values are serializable")
}
pub fn digest_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticsRef {
    pub declaration_digest: String,
    pub revision: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceClaim {
    pub resource_revision: String,
    pub resource_declaration_digest: String,
    pub max_batch_cases: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionClaim {
    pub projection_revision: String,
    pub projection_declaration_digest: String,
    pub projection_declarations: Vec<Value>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProducerDescriptor {
    pub domain: String,
    pub semantics_ref: SemanticsRef,
    pub producer_api_revision: String,
    pub adapter_revision: String,
    pub target_triple: String,
    pub rustc_identity: String,
    pub projection_claim: ProjectionClaim,
    pub resource_claim: ResourceClaim,
}

pub fn canonical_descriptor_value(descriptor: &ProducerDescriptor) -> Value {
    object([
        (
            "adapterRevision",
            Value::String(descriptor.adapter_revision.clone()),
        ),
        ("domain", Value::String(descriptor.domain.clone())),
        (
            "producerApiRevision",
            Value::String(descriptor.producer_api_revision.clone()),
        ),
        (
            "projectionClaim",
            object([
                (
                    "projectionDeclarationDigest",
                    Value::String(
                        descriptor
                            .projection_claim
                            .projection_declaration_digest
                            .clone(),
                    ),
                ),
                (
                    "projectionDeclarations",
                    Value::Array(descriptor.projection_claim.projection_declarations.clone()),
                ),
                (
                    "projectionRevision",
                    Value::String(descriptor.projection_claim.projection_revision.clone()),
                ),
            ]),
        ),
        (
            "resourceClaim",
            object([
                (
                    "maxBatchCases",
                    Value::Number(descriptor.resource_claim.max_batch_cases.into()),
                ),
                (
                    "resourceDeclarationDigest",
                    Value::String(
                        descriptor
                            .resource_claim
                            .resource_declaration_digest
                            .clone(),
                    ),
                ),
                (
                    "resourceRevision",
                    Value::String(descriptor.resource_claim.resource_revision.clone()),
                ),
            ]),
        ),
        (
            "rustcIdentity",
            Value::String(descriptor.rustc_identity.clone()),
        ),
        ("schema", Value::String(DESCRIPTOR_SCHEMA.to_owned())),
        (
            "semanticsRef",
            object([
                (
                    "declarationDigest",
                    Value::String(descriptor.semantics_ref.declaration_digest.clone()),
                ),
                (
                    "revision",
                    Value::String(descriptor.semantics_ref.revision.clone()),
                ),
            ]),
        ),
        (
            "targetTriple",
            Value::String(descriptor.target_triple.clone()),
        ),
    ])
}
pub fn canonical_descriptor_bytes(descriptor: &ProducerDescriptor) -> Vec<u8> {
    canonical_value_bytes(&canonical_descriptor_value(descriptor))
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperandProvenance {
    pub case_id: Option<String>,
    pub family: Option<String>,
    pub requested_observation_kind: Option<String>,
    pub selectors: BTreeMap<String, String>,
    pub projection_revision: Option<String>,
    pub resource_revision: Option<String>,
    pub build_identity: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterFailure(Box<AdapterFailureRecord>);
#[derive(Clone, Debug, Eq, PartialEq)]
struct AdapterFailureRecord {
    kind: &'static str,
    failing_operation: &'static str,
    operand_provenance: OperandProvenance,
    violated_condition: String,
    witness: Option<String>,
    contributing_failures: Vec<String>,
}
impl AdapterFailure {
    pub fn new(
        kind: &'static str,
        failing_operation: &'static str,
        operand_provenance: OperandProvenance,
        violated_condition: impl Into<String>,
    ) -> Self {
        Self(Box::new(AdapterFailureRecord {
            kind,
            failing_operation,
            operand_provenance,
            violated_condition: violated_condition.into(),
            witness: None,
            contributing_failures: Vec::new(),
        }))
    }

    pub fn invalid_request(operation: &'static str, condition: impl Into<String>) -> Self {
        Self::new(
            "invalidRequest",
            operation,
            OperandProvenance {
                case_id: None,
                family: None,
                requested_observation_kind: None,
                selectors: BTreeMap::new(),
                projection_revision: None,
                resource_revision: None,
                build_identity: None,
            },
            condition,
        )
    }

    pub fn kind(&self) -> &'static str {
        self.0.kind
    }

    pub fn failing_operation(&self) -> &'static str {
        self.0.failing_operation
    }

    pub fn operand_provenance(&self) -> &OperandProvenance {
        &self.0.operand_provenance
    }

    pub fn violated_condition(&self) -> &str {
        &self.0.violated_condition
    }

    pub fn witness(&self) -> Option<&str> {
        self.0.witness.as_deref()
    }

    pub fn contributing_failures(&self) -> &[String] {
        &self.0.contributing_failures
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedRequestCase {
    pub case_id: String,
    pub global_case_index: u64,
    pub domain_case_index: u64,
    pub family: String,
    pub requested_observation_kind: String,
    pub selectors: BTreeMap<String, u64>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedRequestBatch {
    pub adapter_revision: String,
    pub domain: String,
    pub algorithm: String,
    pub draw_schedule_revision: String,
    pub seed: String,
    pub cases: Vec<GeneratedRequestCase>,
}

pub fn canonical_request_value(batch: &GeneratedRequestBatch) -> Value {
    let cases = batch
        .cases
        .iter()
        .map(|case| {
            Value::Object(
                [
                    ("caseId".to_owned(), Value::String(case.case_id.clone())),
                    (
                        "domainCaseIndex".to_owned(),
                        Value::String(format!("{:016x}", case.domain_case_index)),
                    ),
                    ("family".to_owned(), Value::String(case.family.clone())),
                    (
                        "globalCaseIndex".to_owned(),
                        Value::String(format!("{:016x}", case.global_case_index)),
                    ),
                    (
                        "requestedObservationKind".to_owned(),
                        Value::String(case.requested_observation_kind.clone()),
                    ),
                    (
                        "selectors".to_owned(),
                        Value::Object(
                            case.selectors
                                .iter()
                                .map(|(key, value)| {
                                    (key.clone(), Value::String(format!("{value:016x}")))
                                })
                                .collect(),
                        ),
                    ),
                ]
                .into_iter()
                .collect(),
            )
        })
        .collect();
    object([
        (
            "adapterRevision",
            Value::String(batch.adapter_revision.clone()),
        ),
        ("cases", Value::Array(cases)),
        ("domain", Value::String(batch.domain.clone())),
        (
            "generator",
            object([
                ("algorithm", Value::String(batch.algorithm.clone())),
                (
                    "drawScheduleRevision",
                    Value::String(batch.draw_schedule_revision.clone()),
                ),
                ("seed", Value::String(batch.seed.clone())),
            ]),
        ),
        ("schema", Value::String(REQUEST_BATCH_SCHEMA.to_owned())),
    ])
}
pub fn canonical_request_bytes(batch: &GeneratedRequestBatch) -> Vec<u8> {
    canonical_value_bytes(&canonical_request_value(batch))
}

pub fn decode_canonical_request(bytes: &[u8]) -> Result<GeneratedRequestBatch, AdapterFailure> {
    reject_duplicate_keys(bytes)
        .map_err(|error| AdapterFailure::invalid_request("decodeRequest", error))?;
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| AdapterFailure::invalid_request("decodeRequest", error.to_string()))?;
    let batch = decode_request_value(&value)?;
    if canonical_request_bytes(&batch) != bytes {
        return Err(AdapterFailure::invalid_request(
            "decodeRequest",
            "request bytes are not canonical",
        ));
    }
    Ok(batch)
}

fn decode_request_value(value: &Value) -> Result<GeneratedRequestBatch, AdapterFailure> {
    let object = required_object(value, "request batch")?;
    exact_fields(
        object,
        &["adapterRevision", "cases", "domain", "generator", "schema"],
        "request batch",
    )?;
    if required_string(object, "schema", "request batch")? != REQUEST_BATCH_SCHEMA {
        return Err(AdapterFailure::invalid_request(
            "decodeRequest",
            "request schema is unsupported",
        ));
    }
    let generator = required_object(
        required_value(object, "generator", "request batch")?,
        "generator",
    )?;
    exact_fields(
        generator,
        &["algorithm", "drawScheduleRevision", "seed"],
        "generator",
    )?;
    let cases = required_value(object, "cases", "request batch")?
        .as_array()
        .ok_or_else(|| {
            AdapterFailure::invalid_request("decodeRequest", "request cases must be an array")
        })?
        .iter()
        .map(decode_request_case)
        .collect::<Result<Vec<_>, _>>()?;
    let mut identities = BTreeSet::new();
    for case in &cases {
        if !identities.insert((case.global_case_index, case.case_id.clone())) {
            return Err(AdapterFailure::invalid_request(
                "decodeRequest",
                "request cases repeat a global index or case identity",
            ));
        }
    }
    Ok(GeneratedRequestBatch {
        adapter_revision: required_string(object, "adapterRevision", "request batch")?,
        domain: required_string(object, "domain", "request batch")?,
        algorithm: required_string(generator, "algorithm", "generator")?,
        draw_schedule_revision: required_string(generator, "drawScheduleRevision", "generator")?,
        seed: required_string(generator, "seed", "generator")?,
        cases,
    })
}

fn decode_request_case(value: &Value) -> Result<GeneratedRequestCase, AdapterFailure> {
    let object = required_object(value, "request case")?;
    exact_fields(
        object,
        &[
            "caseId",
            "domainCaseIndex",
            "family",
            "globalCaseIndex",
            "requestedObservationKind",
            "selectors",
        ],
        "request case",
    )?;
    let case_id = required_string(object, "caseId", "request case")?;
    let global_case_index =
        parse_hex_u64(required_string(object, "globalCaseIndex", "request case")?)?;
    let domain_case_index =
        parse_hex_u64(required_string(object, "domainCaseIndex", "request case")?)?;
    if case_id != format!("case-{global_case_index:016x}") {
        return Err(AdapterFailure::invalid_request(
            "decodeRequest",
            "case identity does not match global case index",
        ));
    }
    let selectors_value = required_object(
        required_value(object, "selectors", "request case")?,
        "selectors",
    )?;
    exact_fields(selectors_value, &SELECTOR_NAMES, "selectors")?;
    let selectors = SELECTOR_NAMES
        .iter()
        .map(|name| {
            required_string(selectors_value, name, "selectors")
                .and_then(parse_hex_u64)
                .map(|value| ((*name).to_owned(), value))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    Ok(GeneratedRequestCase {
        case_id,
        global_case_index,
        domain_case_index,
        family: required_string(object, "family", "request case")?,
        requested_observation_kind: required_string(
            object,
            "requestedObservationKind",
            "request case",
        )?,
        selectors,
    })
}

fn required_object<'a>(
    value: &'a Value,
    name: &str,
) -> Result<&'a serde_json::Map<String, Value>, AdapterFailure> {
    value.as_object().ok_or_else(|| {
        AdapterFailure::invalid_request("decodeRequest", format!("{name} must be an object"))
    })
}
fn required_value<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
    name: &str,
) -> Result<&'a Value, AdapterFailure> {
    object.get(field).ok_or_else(|| {
        AdapterFailure::invalid_request("decodeRequest", format!("{name} lacks {field}"))
    })
}
fn required_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
    name: &str,
) -> Result<String, AdapterFailure> {
    required_value(object, field, name)?
        .as_str()
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            AdapterFailure::invalid_request(
                "decodeRequest",
                format!("{name} {field} must be a nonempty string"),
            )
        })
}
fn exact_fields(
    object: &serde_json::Map<String, Value>,
    fields: &[&str],
    name: &str,
) -> Result<(), AdapterFailure> {
    if object.keys().map(String::as_str).collect::<BTreeSet<_>>()
        == fields.iter().copied().collect()
    {
        Ok(())
    } else {
        Err(AdapterFailure::invalid_request(
            "decodeRequest",
            format!("{name} fields differ from the canonical schema"),
        ))
    }
}
fn parse_hex_u64(value: String) -> Result<u64, AdapterFailure> {
    if value.len() != 16
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(AdapterFailure::invalid_request(
            "decodeRequest",
            "index and selector values must be sixteen lowercase hexadecimal digits",
        ));
    }
    u64::from_str_radix(&value, 16).map_err(|_| {
        AdapterFailure::invalid_request("decodeRequest", "hexadecimal value exceeds u64")
    })
}

pub fn validate_canonical(bytes: &[u8]) -> Result<Vec<Value>, String> {
    reject_duplicate_keys(bytes)?;
    let value: Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let object = value
        .as_object()
        .ok_or_else(|| "batch must be an object".to_owned())?;
    if object.get("schema") != Some(&Value::String(BATCH_SCHEMA.to_owned())) {
        return Err("unexpected batch schema".to_owned());
    }
    let cases = object
        .get("cases")
        .and_then(Value::as_array)
        .ok_or_else(|| "batch cases must be an array".to_owned())?;
    for case in cases {
        validate_case(case)?;
    }
    if canonical_bytes(cases) != bytes {
        return Err("JSON bytes are not canonical".to_owned());
    }
    Ok(cases.to_vec())
}
fn validate_case(case: &Value) -> Result<(), String> {
    let case = case
        .as_object()
        .ok_or_else(|| "case must be an object".to_owned())?;
    if case.len() != 4
        || !["caseId", "operation", "input", "observation"]
            .iter()
            .all(|field| case.contains_key(*field))
    {
        return Err("case has unexpected fields".to_owned());
    }
    if case
        .get("caseId")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
        || case
            .get("operation")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
    {
        return Err("case identity is invalid".to_owned());
    }
    Ok(())
}

fn reject_duplicate_keys(bytes: &[u8]) -> Result<(), String> {
    let mut parser = DuplicateKeyParser { bytes, offset: 0 };
    parser.value()?;
    parser.space();
    if parser.offset == bytes.len() {
        Ok(())
    } else {
        Err("trailing JSON data".to_owned())
    }
}
struct DuplicateKeyParser<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl DuplicateKeyParser<'_> {
    fn space(&mut self) {
        while self
            .bytes
            .get(self.offset)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.offset += 1;
        }
    }
    fn take(&mut self, byte: u8) -> Result<(), String> {
        self.space();
        if self.bytes.get(self.offset) == Some(&byte) {
            self.offset += 1;
            Ok(())
        } else {
            Err("invalid JSON".to_owned())
        }
    }
    fn value(&mut self) -> Result<(), String> {
        self.space();
        match self.bytes.get(self.offset) {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'\"') => {
                self.string()?;
                Ok(())
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(b't') => self.word(b"true"),
            Some(b'f') => self.word(b"false"),
            Some(b'n') => self.word(b"null"),
            _ => Err("invalid JSON".to_owned()),
        }
    }
    fn object(&mut self) -> Result<(), String> {
        self.take(b'{')?;
        self.space();
        let mut keys = BTreeSet::new();
        if self.bytes.get(self.offset) == Some(&b'}') {
            self.offset += 1;
            return Ok(());
        }
        loop {
            let key = self.string()?;
            if !keys.insert(key) {
                return Err("duplicate object key".to_owned());
            }
            self.take(b':')?;
            self.value()?;
            self.space();
            if self.bytes.get(self.offset) == Some(&b'}') {
                self.offset += 1;
                return Ok(());
            }
            self.take(b',')?;
        }
    }
    fn array(&mut self) -> Result<(), String> {
        self.take(b'[')?;
        self.space();
        if self.bytes.get(self.offset) == Some(&b']') {
            self.offset += 1;
            return Ok(());
        }
        loop {
            self.value()?;
            self.space();
            if self.bytes.get(self.offset) == Some(&b']') {
                self.offset += 1;
                return Ok(());
            }
            self.take(b',')?;
        }
    }
    fn string(&mut self) -> Result<String, String> {
        let start = self.offset;
        self.take(b'\"')?;
        let mut escaped = false;
        while let Some(byte) = self.bytes.get(self.offset) {
            self.offset += 1;
            if escaped {
                escaped = false;
                continue;
            }
            if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'\"' {
                return serde_json::from_slice(&self.bytes[start..self.offset])
                    .map_err(|error| error.to_string());
            } else if *byte < 0x20 {
                return Err("invalid JSON string".to_owned());
            }
        }
        Err("unterminated JSON string".to_owned())
    }
    fn number(&mut self) -> Result<(), String> {
        let start = self.offset;
        while self.bytes.get(self.offset).is_some_and(|byte| {
            byte.is_ascii_digit() || matches!(*byte, b'-' | b'+' | b'.' | b'e' | b'E')
        }) {
            self.offset += 1;
        }
        serde_json::from_slice::<Value>(&self.bytes[start..self.offset])
            .map(|_| ())
            .map_err(|_| "invalid JSON number".to_owned())
    }
    fn word(&mut self, word: &[u8]) -> Result<(), String> {
        if self.bytes.get(self.offset..self.offset + word.len()) == Some(word) {
            self.offset += word.len();
            Ok(())
        } else {
            Err("invalid JSON token".to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_request_round_trips() {
        let selectors = SELECTOR_NAMES
            .into_iter()
            .map(|name| (name.to_owned(), 0))
            .collect();
        let batch = GeneratedRequestBatch {
            adapter_revision: ADAPTER_REVISION.to_owned(),
            domain: ALGEBRA_DOMAIN.to_owned(),
            algorithm: GENERATOR_ALGORITHM.to_owned(),
            draw_schedule_revision: DRAW_SCHEDULE_REVISION.to_owned(),
            seed: "0000000000000000".to_owned(),
            cases: vec![GeneratedRequestCase {
                case_id: "case-0000000000000000".to_owned(),
                global_case_index: 0,
                domain_case_index: 0,
                family: "rational-reduction".to_owned(),
                requested_observation_kind: "success".to_owned(),
                selectors,
            }],
        };
        assert_eq!(
            decode_canonical_request(&canonical_request_bytes(&batch)),
            Ok(batch)
        );
    }
}
