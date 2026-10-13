use neco_algebra_protocol_model::{
    canonical_value_bytes, digest_hex, AdapterFailure, OperandProvenance, ProducerDescriptor,
    ProjectionClaim, ResourceClaim, SemanticsRef, ADAPTER_REVISION, ALGEBRA_DOMAIN,
};
use serde_json::Value;

use crate::projection_declaration::declarations;
use crate::PRODUCER_REVISION;

const RUSTC_IDENTITY: &str = include_str!(concat!(env!("OUT_DIR"), "/rustc-identity.txt"));
const TARGET_TRIPLE: &str = include_str!(concat!(env!("OUT_DIR"), "/target-triple.txt"));
const SEMANTICS_DECLARATION: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/algebra-semantics.json"));
const RESOURCE_DECLARATION: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/algebra-resource.json"));
const PROJECTION_DECLARATION: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/algebra-projection-declaration.json"));

pub(crate) fn describe() -> Result<ProducerDescriptor, AdapterFailure> {
    Ok(ProducerDescriptor {
        domain: ALGEBRA_DOMAIN.to_owned(),
        semantics_ref: semantics_ref()?,
        producer_api_revision: PRODUCER_REVISION.to_owned(),
        adapter_revision: ADAPTER_REVISION.to_owned(),
        target_triple: TARGET_TRIPLE.trim_end().to_owned(),
        rustc_identity: RUSTC_IDENTITY.trim_end().to_owned(),
        projection_claim: projection_claim()?,
        resource_claim: resource_claim()?,
    })
}

pub(crate) fn max_batch_cases() -> Result<u64, AdapterFailure> {
    Ok(resource_claim()?.max_batch_cases)
}

pub(crate) fn projection_claim() -> Result<ProjectionClaim, AdapterFailure> {
    let value = canonical_declaration(PROJECTION_DECLARATION, "projection")?;
    let families = value
        .get("families")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| declaration_failure("projection", "families is not an array"))?;
    if families.len() != declarations().len() {
        return Err(declaration_failure(
            "projection",
            "family catalog length differs",
        ));
    }
    for (declaration, family) in declarations().iter().zip(families.iter()) {
        match family.get("family").and_then(Value::as_str) {
            Some(name) if name == declaration.family => {}
            _ => {
                return Err(declaration_failure(
                    "projection",
                    "family catalog names differ",
                ))
            }
        }
    }
    let digest = digest_hex(&canonical_value_bytes(&Value::Array(families.clone())));
    Ok(ProjectionClaim {
        projection_revision: digest.clone(),
        projection_declaration_digest: digest,
        projection_declarations: families,
    })
}

fn semantics_ref() -> Result<SemanticsRef, AdapterFailure> {
    let value = canonical_declaration(SEMANTICS_DECLARATION, "semantics")?;
    if !semantics_declaration_valid(&value) {
        return Err(declaration_failure("semantics", "semantics declaration fields are invalid"));
    }
    Ok(SemanticsRef {
        declaration_digest: digest_hex(SEMANTICS_DECLARATION),
        revision: required_string(&value, "revision", "semantics")?,
    })
}

fn resource_claim() -> Result<ResourceClaim, AdapterFailure> {
    let value = canonical_declaration(RESOURCE_DECLARATION, "resource")?;
    let max_batch_cases = value
        .get("maxBatchCases")
        .and_then(Value::as_u64)
        .ok_or_else(|| declaration_failure("resource", "maxBatchCases is not a u64"))?;
    Ok(ResourceClaim {
        resource_revision: required_string(&value, "resourceRevision", "resource")?,
        resource_declaration_digest: digest_hex(RESOURCE_DECLARATION),
        max_batch_cases,
    })
}

fn canonical_declaration(bytes: &[u8], name: &str) -> Result<Value, AdapterFailure> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| declaration_failure(name, error.to_string()))?;
    if canonical_value_bytes(&value) != bytes {
        return Err(declaration_failure(name, "declaration bytes are not canonical"));
    }
    Ok(value)
}

fn required_string(value: &Value, field: &str, name: &str) -> Result<String, AdapterFailure> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| declaration_failure(name, format!("{field} is not a string")))
}

fn declaration_failure(name: &str, condition: impl Into<String>) -> AdapterFailure {
    AdapterFailure::new(
        "unboundBuildIdentity",
        "describe",
        OperandProvenance {
            case_id: None,
            family: None,
            requested_observation_kind: None,
            selectors: Default::default(),
            projection_revision: None,
            resource_revision: None,
            build_identity: Some(name.to_owned()),
        },
        condition,
    )
}

fn semantics_declaration_valid(value: &Value) -> bool {
    fn fields(value: &Value, names: &[&str]) -> bool {
        value.as_object().is_some_and(|v| v.len() == names.len() && names.iter().all(|n| v.contains_key(*n)))
    }
    fn modules(value: &Value, nonempty: bool) -> bool {
        value.as_array().is_some_and(|a| (!nonempty || !a.is_empty()) && a.iter().enumerate().all(|(i,m)| {
            fields(m, &["module", "sourcePath"]) &&
                m["module"].as_str().zip(m["sourcePath"].as_str()).is_some_and(|(name,path)|
                    name.starts_with("NecoAlgebraVerify.") &&
                    path.strip_prefix("verification/lean/").and_then(|p| p.strip_suffix(".lean"))
                        .is_some_and(|p| name.chars().map(|c| if c == '.' {'/'} else {c}).eq(p.chars()))) &&
                !a[..i].iter().any(|previous| previous["module"] == m["module"])
        }))
    }
    fields(value, &["schema", "domain", "revision", "algebraCheckerApiRevision", "algebraModuleRevision", "ownedModules", "upstreamSemantics"]) &&
        value["schema"] == "neco.verification.semantics-declaration.v2" && value["domain"] == ALGEBRA_DOMAIN &&
        ["revision", "algebraCheckerApiRevision", "algebraModuleRevision"].iter().all(|f| value[*f].as_str().is_some_and(|s| !s.is_empty())) &&
        modules(&value["ownedModules"], true) &&
        value["upstreamSemantics"].as_array().is_some_and(Vec::is_empty)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_revision_excludes_resource_values() {
        let claim = projection_claim().unwrap();
        let encoded = canonical_value_bytes(&Value::Array(claim.projection_declarations));
        assert!(!String::from_utf8_lossy(&encoded).contains("maxBatchCases"));
        assert_eq!(claim.projection_revision, claim.projection_declaration_digest);
    }
}
