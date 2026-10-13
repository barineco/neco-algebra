use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub digest: String,
    pub cases: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct InspectionFailure {
    kind: &'static str,
    operation: &'static str,
    condition: String,
    operand_provenance: serde_json::Value,
}

impl InspectionFailure {
    fn new(kind: &'static str, operation: &'static str, condition: impl Into<String>) -> Self {
        Self {
            kind,
            operation,
            condition: condition.into(),
            operand_provenance: serde_json::json!({}),
        }
    }

    pub fn invalid_invocation(condition: impl Into<String>) -> Self {
        Self::new("invalidProcessInvocation", "parseCli", condition)
    }

    pub fn process_failure(operation: &'static str, condition: impl Into<String>) -> Self {
        Self::new("processFailure", operation, condition)
    }

    pub fn kind(&self) -> &str {
        self.kind
    }

    fn with_paths(mut self, batch: &Path, checker_result: &Path) -> Self {
        self.operand_provenance = serde_json::json!({
            "batch":batch.to_string_lossy(),"checkerResult":checker_result.to_string_lossy()
        });
        self
    }

    pub fn into_report(self) -> serde_json::Value {
        let process_failure = self.kind == "processFailure";
        let failure = serde_json::json!({
            "kind":self.kind,"failingOperation":self.operation,"violatedCondition":self.condition,
            "operandProvenance":self.operand_provenance,"witness":null,"contributingFailures":[]
        });
        let errors = if process_failure {
            vec![failure.clone()]
        } else {
            Vec::new()
        };
        serde_json::json!({
            "schema":"neco.algebra.observer-failure.v1","classification":self.kind,
            "processErrorCount":errors.len(),"processErrors":errors,"failure":failure
        })
    }
}

impl std::fmt::Display for InspectionFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.condition)
    }
}

impl std::error::Error for InspectionFailure {}

pub fn verification_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("observer は verification/observer/ に置く")
        .to_path_buf()
}

pub fn canonical_json(value: &serde_json::Value) -> Result<Vec<u8>, String> {
    let mut output = String::new();
    write_canonical(value, &mut output)?;
    Ok(output.into_bytes())
}

fn write_canonical(value: &serde_json::Value, output: &mut String) -> Result<(), String> {
    match value {
        serde_json::Value::Null => output.push_str("null"),
        serde_json::Value::Bool(true) => output.push_str("true"),
        serde_json::Value::Bool(false) => output.push_str("false"),
        serde_json::Value::Number(number) => output.push_str(&number.to_string()),
        serde_json::Value::String(string) => {
            output.push_str(&serde_json::to_string(string).map_err(|error| error.to_string())?)
        }
        serde_json::Value::Array(values) => {
            output.push('[');
            for (index, item) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_canonical(item, output)?;
            }
            output.push(']');
        }
        serde_json::Value::Object(values) => {
            output.push('{');
            for (index, (key, item)) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(&serde_json::to_string(key).map_err(|error| error.to_string())?);
                output.push(':');
                write_canonical(item, output)?;
            }
            output.push('}');
        }
    }
    Ok(())
}

pub fn canonical_batch(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    canonical_batch_bytes(path, bytes)
}

pub fn canonical_batch_bytes(path: &Path, bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    canonical_batch_value(path, &bytes)?;
    Ok(bytes)
}

fn canonical_batch_value(path: &Path, bytes: &[u8]) -> Result<serde_json::Value, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| format!("{}: UTF-8 ではありません: {error}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| format!("{}: JSON として復号できません: {error}", path.display()))?;
    validate_batch(&value, path)?;
    let encoded = canonical_json(&value)?;
    if bytes != encoded.as_slice() {
        return Err(format!(
            "{}: canonical JSON bytes と一致しません",
            path.display()
        ));
    }
    Ok(value)
}

fn validate_batch(value: &serde_json::Value, path: &Path) -> Result<(), String> {
    let batch = value
        .as_object()
        .ok_or_else(|| format!("{}: batch は object でなければなりません", path.display()))?;
    if batch.len() != 2 || !batch.contains_key("schema") || !batch.contains_key("cases") {
        return Err(format!(
            "{}: batch は schema と cases だけを持たなければなりません",
            path.display()
        ));
    }
    if batch["schema"] != "neco.algebra.observation-batch.v1" {
        return Err(format!("{}: 未対応の schema です", path.display()));
    }
    let cases = batch["cases"]
        .as_array()
        .ok_or_else(|| format!("{}: cases は array でなければなりません", path.display()))?;
    let mut identifiers = std::collections::BTreeSet::new();
    for case in cases {
        let case = case
            .as_object()
            .ok_or_else(|| format!("{}: case は object でなければなりません", path.display()))?;
        if case.len() != 4
            || !case.contains_key("caseId")
            || !case.contains_key("operation")
            || !case.contains_key("input")
            || !case.contains_key("observation")
        {
            return Err(format!(
                "{}: case は caseId, operation, input, observation だけを持たなければなりません",
                path.display()
            ));
        }
        let case_id = case["caseId"]
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                format!(
                    "{}: caseId は空でない string でなければなりません",
                    path.display()
                )
            })?;
        if !identifiers.insert(case_id) {
            return Err(format!(
                "{}: caseId が重複しています: {case_id}",
                path.display()
            ));
        }
        if case["operation"]
            .as_str()
            .filter(|value| !value.is_empty())
            .is_none()
        {
            return Err(format!(
                "{}: operation は空でない string でなければなりません",
                path.display()
            ));
        }
        if !case["input"].is_object() || !case["observation"].is_object() {
            return Err(format!(
                "{}: input と observation は object でなければなりません",
                path.display()
            ));
        }
    }
    Ok(())
}

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn check_expected_digest(path: &Path, digest: &str) -> Result<(), String> {
    let sidecar = path.with_extension("sha256");
    if !sidecar.exists() {
        return Ok(());
    }
    let expected = fs::read_to_string(&sidecar)
        .map_err(|error| format!("{}: {error}", sidecar.display()))?
        .trim()
        .to_owned();
    if expected != digest {
        return Err(format!("{}: digest が一致しません", path.display()));
    }
    Ok(())
}

pub fn run_lean_check(
    executable: &Path,
    batch: &Path,
    digest: &str,
) -> Result<CheckResult, String> {
    let output = Command::new(executable)
        .arg("check")
        .arg(batch)
        .output()
        .map_err(|error| format!("{} を起動できません: {error}", executable.display()))?;
    if !output.status.success() {
        return Err(format!(
            "{} は status {} で終了しました: {}",
            executable.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let stdout = std::str::from_utf8(&output.stdout)
        .map_err(|error| format!("Lean checker の stdout は UTF-8 ではありません: {error}"))?;
    let line = stdout.strip_suffix('\n').unwrap_or(stdout);
    if line.contains('\n') {
        return Err("Lean checker の stdout は一行でなければなりません".to_owned());
    }
    decode_checker_result(line.as_bytes(), digest).map_err(|failure| failure.to_string())
}

pub fn verdict_counts(cases: &serde_json::Value) -> Result<BTreeMap<String, usize>, String> {
    let entries = cases.as_array().ok_or_else(|| {
        "Lean checker の cases は caseId と verdict を持つ object の array でなければなりません"
            .to_owned()
    })?;
    let mut counts = BTreeMap::new();
    let mut case_ids = std::collections::BTreeSet::new();
    for entry in entries {
        let object = entry
            .as_object()
            .ok_or_else(|| "Lean checker の case は object でなければなりません".to_owned())?;
        if object.len() != 2 || !object.contains_key("caseId") || !object.contains_key("verdict") {
            return Err(
                "Lean checker の case は caseId と verdict だけを持たなければなりません".to_owned(),
            );
        }
        let case_id = object["caseId"]
            .as_str()
            .ok_or_else(|| "Lean checker の caseId は string でなければなりません".to_owned())?;
        if case_id.is_empty() {
            return Err("Lean checker の caseId は空文字列にできません".to_owned());
        }
        if !case_ids.insert(case_id) {
            return Err(format!(
                "Lean checker の caseId が重複しています: {case_id}"
            ));
        }
        let verdict = object["verdict"].as_str().ok_or_else(|| {
            "Lean checker の case verdict は string でなければなりません".to_owned()
        })?;
        match verdict {
            "verified" | "refuted" | "undetermined" | "invalid" => {
                if let Some(count) = counts.get_mut(verdict) {
                    *count += 1;
                } else {
                    counts.insert(verdict.to_owned(), 1);
                }
            }
            _ => return Err("unknown verdict".to_owned()),
        }
    }
    Ok(counts)
}

fn decode_checker_result(bytes: &[u8], digest: &str) -> Result<CheckResult, InspectionFailure> {
    let invalid = |condition: String| {
        InspectionFailure::new("resultInvalid", "decodeCheckerResult", condition)
    };
    let text = std::str::from_utf8(bytes).map_err(|error| {
        invalid(format!(
            "Lean checker の stdout は UTF-8 ではありません: {error}"
        ))
    })?;
    let mut value: serde_json::Value = serde_json::from_str(text).map_err(|error| {
        invalid(format!(
            "Lean checker の stdout は JSON ではありません: {error}"
        ))
    })?;
    if canonical_json(&value).map_err(invalid)? != bytes {
        return Err(invalid(
            "Lean checker の stdout は canonical JSON ではありません".to_owned(),
        ));
    }
    let object = value.as_object_mut().ok_or_else(|| {
        invalid("Lean checker の stdout は object でなければなりません".to_owned())
    })?;
    if object.len() != 2 || !object.contains_key("digest") || !object.contains_key("cases") {
        return Err(invalid(
            "Lean checker の stdout は digest と cases だけを持たなければなりません".to_owned(),
        ));
    }
    let observed_digest = match object.remove("digest") {
        Some(serde_json::Value::String(digest)) => digest,
        _ => {
            return Err(invalid(
                "Lean checker の digest は string でなければなりません".to_owned(),
            ))
        }
    };
    if observed_digest != digest {
        return Err(InspectionFailure::new(
            "resultDigestMismatch",
            "reconcile",
            "checker digest differs from actual input bytes",
        ));
    }
    let cases = object
        .remove("cases")
        .ok_or_else(|| invalid("Lean checker の cases がありません".to_owned()))?;
    Ok(CheckResult {
        digest: observed_digest,
        cases,
    })
}

fn batch_invalid_result(cases: &serde_json::Value) -> bool {
    cases.as_array().is_some_and(|entries| {
        entries.len() == 1
            && entries[0].as_object().is_some_and(|entry| {
                entry.len() == 2
                    && entry.get("caseId").and_then(serde_json::Value::as_str) == Some("")
                    && entry.get("verdict").and_then(serde_json::Value::as_str) == Some("invalid")
            })
    })
}

pub fn inspect_bytes(
    batch: &[u8],
    checker_result: &[u8],
) -> Result<serde_json::Value, InspectionFailure> {
    let digest = sha256(batch);
    let result = decode_checker_result(checker_result, &digest)?;
    let decoded = canonical_batch_value(Path::new("batch bytes"), batch);
    let batch = match decoded {
        Ok(batch) => batch,
        Err(condition) => {
            if batch_invalid_result(&result.cases) {
                return Ok(serde_json::json!({"cases":{"invalid":1},"digest":result.digest}));
            }
            return Err(InspectionFailure::new(
                "batchInvalid",
                "decodeObservation",
                condition,
            ));
        }
    };
    let cases = batch["cases"].as_array().ok_or_else(|| {
        InspectionFailure::new(
            "batchInvalid",
            "decodeObservation",
            "batch cases must be an array",
        )
    })?;
    if cases.is_empty() {
        return Err(InspectionFailure::new(
            "batchInvalid",
            "decodeObservation",
            "inspection requires a nonempty batch",
        ));
    }
    if batch_invalid_result(&result.cases) {
        return Err(InspectionFailure::new(
            "resultContradiction",
            "reconcile",
            "batch invalid singleton cannot describe an enumerated canonical batch",
        ));
    }
    let results = result.cases.as_array().ok_or_else(|| {
        InspectionFailure::new(
            "resultInvalid",
            "decodeCheckerResult",
            "checker cases must be an array",
        )
    })?;
    if results.is_empty() {
        return Err(InspectionFailure::new(
            "resultInvalid",
            "decodeCheckerResult",
            "result cases must be nonempty",
        ));
    }
    let counts = verdict_counts(&result.cases).map_err(|condition| {
        InspectionFailure::new("resultInvalid", "decodeCheckerResult", condition)
    })?;
    if cases.len() != results.len() {
        return Err(InspectionFailure::new(
            "caseCorrespondenceMismatch",
            "reconcile",
            "case count differs",
        ));
    }
    if cases
        .iter()
        .zip(results)
        .any(|(case, row)| case["caseId"] != row["caseId"])
    {
        return Err(InspectionFailure::new(
            "caseCorrespondenceMismatch",
            "reconcile",
            "caseId order differs",
        ));
    }
    Ok(serde_json::json!({"cases":counts,"digest":result.digest}))
}

pub fn inspect_files(
    batch: &Path,
    checker_result: &Path,
) -> Result<serde_json::Value, InspectionFailure> {
    let batch_bytes = fs::read(batch).map_err(|error| {
        InspectionFailure::process_failure("readBatch", error.to_string())
            .with_paths(batch, checker_result)
    })?;
    let result_bytes = fs::read(checker_result).map_err(|error| {
        InspectionFailure::process_failure("readCheckerResult", error.to_string())
            .with_paths(batch, checker_result)
    })?;
    inspect_bytes(&batch_bytes, &result_bytes)
        .map_err(|failure| failure.with_paths(batch, checker_result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_json_orders_object_keys() {
        let value: serde_json::Value =
            serde_json::from_str(r#"{"z":0,"a":{"b":false,"a":true}}"#).unwrap();
        assert_eq!(
            canonical_json(&value).unwrap(),
            br#"{"a":{"a":true,"b":false},"z":0}"#
        );
    }

    #[test]
    fn canonical_batch_requires_the_observation_batch_shape() {
        let batch = br#"{"cases":[{"caseId":"case-001","input":{},"observation":{},"operation":"rational-reduction"}],"schema":"neco.algebra.observation-batch.v1"}"#;
        assert!(canonical_batch_bytes(Path::new("batch.json"), batch.to_vec()).is_ok());
        let array = br#"[]"#;
        assert!(canonical_batch_bytes(Path::new("array.json"), array.to_vec()).is_err());
    }

    #[test]
    fn verdict_counts_accepts_the_protocol_verdicts() {
        let cases: serde_json::Value = serde_json::from_str(
            r#"[{"caseId":"a","verdict":"verified"},{"caseId":"b","verdict":"invalid"},{"caseId":"c","verdict":"verified"},{"caseId":"d","verdict":"refuted"}]"#,
        )
        .unwrap();
        let counts = verdict_counts(&cases).unwrap();
        assert_eq!(counts["verified"], 2);
        assert_eq!(counts["invalid"], 1);
        assert_eq!(counts["refuted"], 1);
    }

    #[test]
    fn verdict_counts_rejects_invalid_case_shapes() {
        for cases in [
            r#"[{"caseId":"","verdict":"verified"}]"#,
            r#"[{"caseId":"a","verdict":"verified"},{"caseId":"a","verdict":"invalid"}]"#,
            r#"[{"caseId":"a","verdict":"verified","extra":true}]"#,
            r#"[{"caseId":"a","verdict":"other"}]"#,
        ] {
            let cases: serde_json::Value = serde_json::from_str(cases).unwrap();
            assert!(verdict_counts(&cases).is_err());
        }
    }

    fn inspection_pair(ids: &[&str]) -> (Vec<u8>, serde_json::Value) {
        let cases = ids
            .iter()
            .map(|id| {
                serde_json::json!({
                    "caseId":id,"operation":"rational-reduction",
                    "input":{"numerator":"2","denominator":"4"},
                    "observation":{"kind":"success","value":{"numerator":"1","denominator":"2"}}
                })
            })
            .collect::<Vec<_>>();
        let batch = canonical_json(&serde_json::json!({
            "schema":"neco.algebra.observation-batch.v1","cases":cases
        }))
        .unwrap();
        let result = serde_json::json!({
            "cases":ids.iter().map(|id| serde_json::json!({"caseId":id,"verdict":"verified"})).collect::<Vec<_>>(),
            "digest":sha256(&batch)
        });
        (batch, result)
    }

    #[test]
    fn inspection_accepts_correspondence_and_rejects_four_result_mutations() {
        let (batch, result) = inspection_pair(&["a", "b"]);
        let summary = inspect_bytes(&batch, &canonical_json(&result).unwrap()).unwrap();
        assert_eq!(
            summary,
            serde_json::json!({"cases":{"verified":2},"digest":sha256(&batch)})
        );
        let mut digest = result.clone();
        digest["digest"] = serde_json::json!("0".repeat(64));
        let mut reversed = result.clone();
        reversed["cases"].as_array_mut().unwrap().reverse();
        let mut unknown = result.clone();
        unknown["cases"][0]["verdict"] = serde_json::json!("not-a-verdict");
        let (one_batch, mut missing) = inspection_pair(&["a"]);
        missing["cases"] = serde_json::json!([]);
        for (input, changed, kind, operation) in [
            (&batch, digest, "resultDigestMismatch", "reconcile"),
            (&batch, reversed, "caseCorrespondenceMismatch", "reconcile"),
            (&batch, unknown, "resultInvalid", "decodeCheckerResult"),
            (&one_batch, missing, "resultInvalid", "decodeCheckerResult"),
        ] {
            let failure = inspect_bytes(input, &canonical_json(&changed).unwrap()).unwrap_err();
            assert_eq!(failure.kind(), kind);
            assert_eq!(failure.operation, operation);
            let report = failure.into_report();
            assert_eq!(report["processErrorCount"], 0);
            assert_eq!(report["processErrors"], serde_json::json!([]));
        }
    }

    #[test]
    fn inspection_distinguishes_native_raw_invalid_from_enumerated_batches() {
        let invalid_bytes = b"\xff\xfe\0";
        let singleton = serde_json::json!({
            "cases":[{"caseId":"","verdict":"invalid"}],"digest":sha256(invalid_bytes)
        });
        let inspected = inspect_bytes(invalid_bytes, &canonical_json(&singleton).unwrap()).unwrap();
        assert_eq!(
            inspected,
            serde_json::json!({"cases":{"invalid":1},"digest":sha256(invalid_bytes)})
        );
        let (batch, _) = inspection_pair(&["a"]);
        let mut contradiction = singleton.clone();
        contradiction["digest"] = serde_json::json!(sha256(&batch));
        assert_eq!(
            inspect_bytes(&batch, &canonical_json(&contradiction).unwrap())
                .unwrap_err()
                .kind(),
            "resultContradiction"
        );
        let mut wrong_digest = singleton.clone();
        wrong_digest["digest"] = serde_json::json!(sha256(&batch));
        assert_eq!(
            inspect_bytes(invalid_bytes, &canonical_json(&wrong_digest).unwrap())
                .unwrap_err()
                .kind(),
            "resultDigestMismatch"
        );
        let mut false_verdict = singleton;
        false_verdict["cases"][0]["verdict"] = serde_json::json!("verified");
        assert_eq!(
            inspect_bytes(invalid_bytes, &canonical_json(&false_verdict).unwrap())
                .unwrap_err()
                .kind(),
            "batchInvalid"
        );
    }

    #[test]
    fn inspection_rejects_noncanonical_results_and_identity_boundaries() {
        let (batch, result) = inspection_pair(&["a", "b"]);
        let encoded = canonical_json(&result).unwrap();
        let mut whitespace = encoded.clone();
        whitespace.push(b'\n');
        assert_eq!(
            inspect_bytes(&batch, &whitespace).unwrap_err().kind(),
            "resultInvalid"
        );
        let duplicate = String::from_utf8(encoded).unwrap().replace(
            "\"digest\":",
            &format!("\"digest\":\"{}\",\"digest\":", sha256(&batch)),
        );
        assert_eq!(
            inspect_bytes(&batch, duplicate.as_bytes())
                .unwrap_err()
                .kind(),
            "resultInvalid"
        );
        let mut duplicate_ids = result.clone();
        duplicate_ids["cases"][1]["caseId"] = serde_json::json!("a");
        assert_eq!(
            inspect_bytes(&batch, &canonical_json(&duplicate_ids).unwrap())
                .unwrap_err()
                .kind(),
            "resultInvalid"
        );
        let mut absent = result.clone();
        absent["cases"].as_array_mut().unwrap().pop();
        assert_eq!(
            inspect_bytes(&batch, &canonical_json(&absent).unwrap())
                .unwrap_err()
                .kind(),
            "caseCorrespondenceMismatch"
        );
        let empty = br#"{"cases":[],"schema":"neco.algebra.observation-batch.v1"}"#;
        let empty_result = serde_json::json!({"cases":[],"digest":sha256(empty)});
        assert_eq!(
            inspect_bytes(empty, &canonical_json(&empty_result).unwrap())
                .unwrap_err()
                .kind(),
            "batchInvalid"
        );
    }

    #[test]
    fn inspection_read_errors_precede_protocol_rejection() {
        struct TemporaryDirectory(PathBuf);
        impl Drop for TemporaryDirectory {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "neco-algebra-observer-inspection-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let directory = TemporaryDirectory(directory);
        let missing_batch = directory.0.join("missing-batch");
        let missing_result = directory.0.join("missing-result");
        let first = inspect_files(&missing_batch, &missing_result).unwrap_err();
        assert_eq!(first.kind(), "processFailure");
        assert_eq!(first.operation, "readBatch");
        let bad_batch = directory.0.join("non-utf8-batch");
        fs::write(&bad_batch, b"\xff\xfe\0").unwrap();
        let failure = inspect_files(&bad_batch, &missing_result).unwrap_err();
        assert_eq!(failure.kind(), "processFailure");
        assert_eq!(failure.operation, "readCheckerResult");
        let report = failure.into_report();
        assert_eq!(report["processErrorCount"], 1);
        assert_eq!(
            report["processErrors"],
            serde_json::json!([report["failure"]])
        );
    }
}
