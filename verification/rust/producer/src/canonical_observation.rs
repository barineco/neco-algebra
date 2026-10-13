use neco_algebra_protocol_model::{canonical_bytes, validate_canonical, AdapterFailure};
use serde_json::Value;

pub(crate) fn encode(cases: &[Value]) -> Vec<u8> {
    canonical_bytes(cases)
}
pub(crate) fn verify_supplied(supplied: &[u8], replayed: &[Value]) -> Result<(), AdapterFailure> {
    validate_canonical(supplied)
        .map_err(|error| AdapterFailure::invalid_request("selfCheck", error))?;
    if encode(replayed) == supplied {
        Ok(())
    } else {
        Err(AdapterFailure::invalid_request(
            "selfCheck",
            "supplied observation bytes differ from public API replay",
        ))
    }
}
