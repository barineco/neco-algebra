use std::{
    io::Write,
    process::{Command, Stdio},
};

use neco_algebra_protocol_model::{canonical_value_bytes, validate_canonical};
use serde_json::Value;

fn producer() -> Command {
    Command::new(env!("CARGO_BIN_EXE_neco-algebra-producer"))
}

#[test]
fn generate_file_and_stdin_emit_identical_canonical_bytes() {
    let request = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vectors/requests/algebra-generated-request-batch.json"
    );
    let file = producer()
        .args(["generate", "--request", request])
        .output()
        .expect("producer starts");
    assert!(file.status.success());
    assert!(file.stderr.is_empty());
    assert!(!file.stdout.ends_with(b"\n"));
    validate_canonical(&file.stdout).expect("generated batch is canonical");

    let request_bytes = std::fs::read(request).expect("request vector");
    let mut child = producer()
        .args(["generate", "--request", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("producer starts");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(&request_bytes)
        .expect("write request");
    let stdin = child.wait_with_output().expect("producer exits");
    assert!(stdin.status.success());
    assert_eq!(stdin.stdout, file.stdout);
    assert!(stdin.stderr.is_empty());
}

#[test]
fn self_check_emits_the_canonical_parity_result() {
    let request = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vectors/requests/algebra-generated-request-batch.json"
    );
    let batch = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vectors/requests/algebra-generated-observation-batch.json"
    );
    let output = producer()
        .args(["self-check", "--request", request, "--batch", batch])
        .output()
        .expect("producer starts");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let value: Value = serde_json::from_slice(&output.stdout).expect("parity result");
    assert_eq!(canonical_value_bytes(&value), output.stdout);
    assert_eq!(value["schema"], "neco.verification.public-api-parity-result.v1");
    assert_eq!(value["verified"], true);
}

#[test]
fn generated_path_failures_have_process_contract() {
    for arguments in [vec!["fixture"], vec!["generate"], vec!["describe", "extra"]] {
        let output = producer()
            .args(arguments)
            .output()
            .expect("producer starts");
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.ends_with(b"\n"));
        let value: Value = serde_json::from_slice(&output.stderr).expect("canonical failure");
        assert_eq!(canonical_value_bytes(&value), output.stderr);
        assert_eq!(value["adapterFailure"], "invalidProcessInvocation");
    }

}

#[test]
fn describe_emits_canonical_descriptor_with_independent_claims() {
    let describe = producer()
        .arg("describe")
        .output()
        .expect("producer starts");
    assert!(describe.status.success());
    assert!(describe.stderr.is_empty());
    assert!(!describe.stdout.ends_with(b"\n"));
    let value: Value = serde_json::from_slice(&describe.stdout).expect("canonical descriptor");
    assert_eq!(canonical_value_bytes(&value), describe.stdout);
    assert_eq!(value["schema"], "neco.verification.producer-description.v1");
    assert_eq!(value["domain"], "neco.algebra");
    assert!(value["rustcIdentity"].as_str().is_some_and(|text| text.starts_with("rustc ")));
    assert!(value["targetTriple"].as_str().is_some_and(|text| !text.is_empty()));
    assert_eq!(value["projectionClaim"]["projectionDeclarations"].as_array().map(Vec::len), Some(6));
    assert_eq!(value["projectionClaim"]["projectionRevision"].as_str().map(str::len), Some(64));
    assert_eq!(value["resourceClaim"]["maxBatchCases"], 2000);
    assert_eq!(value["resourceClaim"]["resourceRevision"], "algebra-resource-v1");
    assert_eq!(value["resourceClaim"]["resourceDeclarationDigest"].as_str().map(str::len), Some(64));
    assert!(value.get("sourceOid").is_none());
    let declarations = value["projectionClaim"]["projectionDeclarations"]
        .as_array()
        .expect("projectionDeclarations");
    for declaration in declarations {
        let axes = declaration["applicableCoverageAxes"]
            .as_array()
            .expect("applicableCoverageAxes");
        let bindings = declaration["selectorBindings"]
            .as_array()
            .expect("selectorBindings");
        assert_eq!(axes.len(), bindings.len());
        for (axis, binding) in axes.iter().zip(bindings) {
            assert_eq!(binding["axis"], *axis);
            let rule = &binding["selectorRule"];
            assert!(rule.is_object());
            assert_eq!(rule["modulus"], 4);
            assert_eq!(rule["selector"], binding["selector"]);
            assert_eq!(rule["bucketsByRemainder"].as_object().map(|map| map.len()), Some(4));
        }
    }
    let projection = canonical_value_bytes(&value["projectionClaim"]["projectionDeclarations"]);
    assert!(!String::from_utf8_lossy(&projection).contains("maxBatchCases"));
}
