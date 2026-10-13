use neco_algebra_protocol_model::{canonical_value_bytes, object, AdapterFailure};
use serde_json::Value;

pub(crate) fn render_failure(failure: AdapterFailure) -> Vec<u8> {
    canonical_value_bytes(&object([
        ("adapterFailure", Value::String(failure.kind().to_owned())),
        (
            "operation",
            Value::String(failure.failing_operation().to_owned()),
        ),
        (
            "reason",
            Value::String(failure.violated_condition().to_owned()),
        ),
    ]))
}
