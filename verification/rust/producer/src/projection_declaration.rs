use neco_algebra_protocol_model::{object, AdapterFailure, GeneratedRequestCase, OperandProvenance};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProjectionDeclaration {
    pub family: &'static str,
    pub requested_kinds: &'static [&'static str],
    pub applicable_axes: &'static [&'static str],
    pub operation: &'static str,
    pub checker_predicate: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PublicApiInvocation {
    pub operation: &'static str,
    pub arguments: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ProjectedRequest {
    pub case_id: String,
    pub family: &'static str,
    pub requested_observation_kind: String,
    pub invocation: PublicApiInvocation,
    pub checker_predicate: &'static str,
}

const DECLARATIONS: [ProjectionDeclaration; 6] = [
    ProjectionDeclaration {
        family: "rational-reduction",
        requested_kinds: &["success", "zero-denominator"],
        applicable_axes: &["value", "familyParameter", "sign", "formulaVariant"],
        operation: "rational-reduction",
        checker_predicate: "reduced-rational-or-zero-denominator",
    },
    ProjectionDeclaration {
        family: "extended-gcd-bezout",
        requested_kinds: &["success"],
        applicable_axes: &["value", "familyParameter", "sign", "formulaVariant"],
        operation: "extended-gcd-bezout",
        checker_predicate: "bezout-identity",
    },
    ProjectionDeclaration {
        family: "local-algebraic-root",
        requested_kinds: &["success"],
        applicable_axes: &["value", "sign", "intervalWidth", "formulaVariant"],
        operation: "local-algebraic-root",
        checker_predicate: "isolating-interval-sign-change",
    },
    ProjectionDeclaration {
        family: "scalar-f64-projection",
        requested_kinds: &["success", "non-finite-f64"],
        applicable_axes: &["value", "familyParameter", "sign", "formulaVariant"],
        operation: "scalar-f64-projection",
        checker_predicate: "f64-enclosure-or-range-failure",
    },
    ProjectionDeclaration {
        family: "rational-linear-unique",
        requested_kinds: &["success", "dimension-mismatch", "singular-system"],
        applicable_axes: &["value", "familyParameter", "sign", "formulaVariant"],
        operation: "rational-linear-unique",
        checker_predicate: "unique-linear-solution-or-typed-failure",
    },
    ProjectionDeclaration {
        family: "rational-linear-affine",
        requested_kinds: &["success", "dimension-mismatch", "singular-system"],
        applicable_axes: &["value", "familyParameter", "sign", "formulaVariant"],
        operation: "rational-linear-affine",
        checker_predicate: "affine-linear-solution-or-typed-failure",
    },
];

pub(crate) fn declarations() -> &'static [ProjectionDeclaration] {
    &DECLARATIONS
}

pub(crate) fn declaration(family: &str) -> Result<&'static ProjectionDeclaration, AdapterFailure> {
    DECLARATIONS
        .iter()
        .find(|declaration| declaration.family == family)
        .ok_or_else(|| unsupported(family, None, "family is outside the six-family declaration"))
}

pub(crate) fn project(case: &GeneratedRequestCase) -> Result<ProjectedRequest, AdapterFailure> {
    let declaration = declaration(&case.family)?;
    if !declaration
        .requested_kinds
        .contains(&case.requested_observation_kind.as_str())
    {
        return Err(unsupported(
            &case.family,
            Some(case),
            "requested observation kind is outside the declaration",
        ));
    }
    let invocation = match declaration.family {
        "rational-reduction" => reduction(case),
        "extended-gcd-bezout" => gcd(case),
        "local-algebraic-root" => root(case),
        "scalar-f64-projection" => scalar(case),
        "rational-linear-unique" => linear(case, false),
        "rational-linear-affine" => linear(case, true),
        _ => {
            return Err(unsupported(
                &case.family,
                Some(case),
                "family declaration cannot be projected",
            ))
        }
    }?;
    Ok(ProjectedRequest {
        case_id: case.case_id.clone(),
        family: declaration.family,
        requested_observation_kind: case.requested_observation_kind.clone(),
        invocation,
        checker_predicate: declaration.checker_predicate,
    })
}

fn reduction(case: &GeneratedRequestCase) -> Result<PublicApiInvocation, AdapterFailure> {
    let factor = common_factor(case)?;
    let numerator = signed(nonzero(selector(case, "value")?)?, selector(case, "sign")?) * factor;
    let denominator = if case.requested_observation_kind == "zero-denominator" {
        0
    } else {
        nonzero(selector(case, "familyParameter")?)? as i64 * factor
    };
    Ok(PublicApiInvocation {
        operation: "rational-reduction",
        arguments: object([
            ("denominator", Value::String(denominator.to_string())),
            ("numerator", Value::String(numerator.to_string())),
        ]),
    })
}
fn gcd(case: &GeneratedRequestCase) -> Result<PublicApiInvocation, AdapterFailure> {
    let left = nonzero(selector(case, "value")?)?;
    let right = nonzero(selector(case, "familyParameter")?)?;
    let (left, right) = match selector(case, "formulaVariant")? % 4 {
        3 => (left, left * right + 1),
        variant => (left * (variant + 1), right * (variant + 1)),
    };
    let left = signed(left, selector(case, "sign")?);
    let right = signed(right, selector(case, "sign")? >> 1);
    Ok(PublicApiInvocation {
        operation: "extended-gcd-bezout",
        arguments: object([
            ("left", Value::String(left.to_string())),
            ("right", Value::String(right.to_string())),
        ]),
    })
}
fn root(case: &GeneratedRequestCase) -> Result<PublicApiInvocation, AdapterFailure> {
    if selector(case, "value")? == 0
        && selector(case, "sign")? == 0
        && selector(case, "intervalWidth")? == 0
        && selector(case, "formulaVariant")? == 0
    {
        return Ok(PublicApiInvocation {
            operation: "local-algebraic-root",
            arguments: object([(
                "polynomial",
                Value::Array(vec![
                    Value::String("-2".to_owned()),
                    Value::String("0".to_owned()),
                    Value::String("1".to_owned()),
                ]),
            )]),
        });
    }
    let lower = signed(selector(case, "value")? % 17 + 1, selector(case, "sign")?);
    let width = selector(case, "intervalWidth")? % 17 + 1;
    let upper = lower + width as i64;
    let factor = common_factor(case)?;
    Ok(PublicApiInvocation {
        operation: "local-algebraic-root",
        arguments: object([(
            "polynomial",
            Value::Array(vec![
                Value::String((factor * lower * upper).to_string()),
                Value::String((-factor * (lower + upper)).to_string()),
                Value::String(factor.to_string()),
            ]),
        )]),
    })
}
fn scalar(case: &GeneratedRequestCase) -> Result<PublicApiInvocation, AdapterFailure> {
    let integer = signed(nonzero(selector(case, "value")?)?, selector(case, "sign")?);
    let variant = (selector(case, "formulaVariant")? % 16) as u32;
    let (exponent, shift) = if case.requested_observation_kind == "non-finite-f64" {
        (0, F64_RANGE_BITS + variant)
    } else {
        (variant, 0)
    };
    Ok(PublicApiInvocation {
        operation: "scalar-f64-projection",
        arguments: object([
            (
                "bits",
                Value::Number(((selector(case, "familyParameter")? % 32) as u32).into()),
            ),
            ("shift", Value::Number(shift.into())),
            (
                "value",
                object([
                    ("exponent", Value::Number(exponent.into())),
                    ("integer", Value::String(integer.to_string())),
                ]),
            ),
        ]),
    })
}
fn linear(
    case: &GeneratedRequestCase,
    affine: bool,
) -> Result<PublicApiInvocation, AdapterFailure> {
    let coefficient = signed(nonzero(selector(case, "value")?)?, selector(case, "sign")?);
    let rhs = signed(
        nonzero(selector(case, "familyParameter")?)?,
        selector(case, "sign")? >> 1,
    );
    let c = coefficient;
    let rows: Vec<Vec<i64>> = match (case.requested_observation_kind.as_str(), affine) {
        ("singular-system", _) => match selector(case, "formulaVariant")? % 4 {
            0 => vec![vec![0]],
            1 => vec![vec![c, c], vec![c, c]],
            2 => vec![vec![c, 0], vec![0, 0]],
            _ => vec![vec![0, c], vec![0, c]],
        },
        (_, true) => match selector(case, "formulaVariant")? % 4 {
            0 => vec![vec![c, 0]],
            1 => vec![vec![0, c]],
            2 => vec![vec![c, c]],
            _ => vec![vec![c, 0, 0]],
        },
        (_, false) => match selector(case, "formulaVariant")? % 4 {
            0 => vec![vec![c]],
            1 => vec![vec![c, 0], vec![0, c]],
            2 => vec![vec![0, c], vec![c, 0]],
            _ => vec![vec![c, c], vec![0, c]],
        },
    };
    let rhs_length = if case.requested_observation_kind == "dimension-mismatch" {
        rows.len() + 1
    } else {
        rows.len()
    };
    let matrix = rows
        .into_iter()
        .map(|row| Value::Array(row.into_iter().map(rational_value).collect()))
        .collect();
    let rhs = vec![rational_value(rhs); rhs_length];
    let operation = if affine {
        "rational-linear-affine"
    } else {
        "rational-linear-unique"
    };
    Ok(PublicApiInvocation {
        operation,
        arguments: object([("matrix", Value::Array(matrix)), ("rhs", Value::Array(rhs))]),
    })
}
const F64_RANGE_BITS: u32 = 1024;

fn common_factor(case: &GeneratedRequestCase) -> Result<i64, AdapterFailure> {
    Ok((selector(case, "formulaVariant")? % 4 + 1) as i64)
}
fn rational_value(numerator: i64) -> Value {
    object([
        ("denominator", Value::String("1".to_owned())),
        ("numerator", Value::String(numerator.to_string())),
    ])
}
fn selector(case: &GeneratedRequestCase, name: &str) -> Result<u64, AdapterFailure> {
    case.selectors.get(name).copied().ok_or_else(|| {
        unsupported(
            &case.family,
            Some(case),
            "selector is absent from canonical request",
        )
    })
}
fn nonzero(value: u64) -> Result<u64, AdapterFailure> {
    value
        .checked_rem(31)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| unsupported("", None, "selector cannot be represented"))
}
fn signed(value: u64, sign_selector: u64) -> i64 {
    let value = value as i64;
    if sign_selector % 2 == 0 {
        value
    } else {
        -value
    }
}
fn unsupported(
    family: &str,
    case: Option<&GeneratedRequestCase>,
    condition: &str,
) -> AdapterFailure {
    AdapterFailure::new(
        "unsupportedProjection",
        "projectRequest",
        OperandProvenance {
            case_id: case.map(|case| case.case_id.clone()),
            family: (!family.is_empty()).then(|| family.to_owned()),
            requested_observation_kind: case.map(|case| case.requested_observation_kind.clone()),
            selectors: case
                .map(|case| {
                    case.selectors
                        .iter()
                        .map(|(name, value)| (name.clone(), format!("{value:016x}")))
                        .collect()
                })
                .unwrap_or_default(),
            projection_revision: None,
            resource_revision: None,
            build_identity: None,
        },
        condition,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use neco_algebra_protocol_model::{GeneratedRequestCase, SELECTOR_NAMES};
    use serde_json::Value;
    #[test]
    fn non_applicable_axes_do_not_change_projection() {
        let selectors = SELECTOR_NAMES
            .into_iter()
            .map(|name| (name.to_owned(), 0))
            .collect();
        let case = GeneratedRequestCase {
            case_id: "case-0000000000000000".to_owned(),
            global_case_index: 0,
            domain_case_index: 0,
            family: "rational-reduction".to_owned(),
            requested_observation_kind: "success".to_owned(),
            selectors,
        };
        let mut changed = case.clone();
        changed.selectors.insert("observationClass".to_owned(), 3);
        changed.selectors.insert("mutation".to_owned(), 2);
        changed.selectors.insert("reserved".to_owned(), 1);
        assert_eq!(
            project(&case).unwrap().invocation,
            project(&changed).unwrap().invocation
        );
    }

    #[test]
    fn requested_observation_kind_is_independent_of_formula_variant_remainder() {
        let mut selectors: std::collections::BTreeMap<String, u64> = SELECTOR_NAMES
            .into_iter()
            .map(|name| (name.to_owned(), 0))
            .collect();
        selectors.insert("formulaVariant".to_owned(), 3);
        let case = GeneratedRequestCase {
            case_id: "case-0000000000000000".to_owned(),
            global_case_index: 0,
            domain_case_index: 0,
            family: "rational-reduction".to_owned(),
            requested_observation_kind: "success".to_owned(),
            selectors,
        };
        let projected = project(&case).expect("success remains projectable when formulaVariant remainder is 3");
        assert_eq!(projected.requested_observation_kind, "success");
        assert_eq!(
            projected.invocation.arguments["denominator"],
            Value::String("4".to_owned())
        );
    }

    #[test]
    fn declared_requests_produce_the_requested_observation() {
        for declaration in declarations() {
            for requested in declaration.requested_kinds {
                for (formula_variant, sign, value) in
                    (0..4).flat_map(|f| (0..4).flat_map(move |s| (0..2).map(move |v| (f, s, v))))
                {
                    let mut selectors: std::collections::BTreeMap<String, u64> = SELECTOR_NAMES
                        .into_iter()
                        .map(|name| (name.to_owned(), value))
                        .collect();
                    selectors.insert("formulaVariant".to_owned(), formula_variant);
                    selectors.insert("sign".to_owned(), sign);
                    let case = GeneratedRequestCase {
                        case_id: "case-0000000000000000".to_owned(),
                        global_case_index: 0,
                        domain_case_index: 0,
                        family: declaration.family.to_owned(),
                        requested_observation_kind: (*requested).to_owned(),
                        selectors,
                    };
                    let projected = project(&case).expect("declared request is projectable");
                    let observed = crate::product_api_port::invoke(&projected)
                        .and_then(|observed| {
                            crate::observation_authority::classify(&projected, observed)
                        })
                        .and_then(|classified| {
                            crate::observation_authority::require_requested_kind_match(
                                &projected, classified,
                            )
                        });
                    assert!(
                        observed.is_ok(),
                        "{} {} {:?}",
                        declaration.family,
                        requested,
                        projected.invocation.arguments
                    );
                }
            }
        }
    }

    #[test]
    fn applicable_axes_change_projection() {
        let mut inert = Vec::new();
        for declaration in declarations() {
            for requested in declaration.requested_kinds {
                for axis in declaration.applicable_axes {
                    let invocations: std::collections::BTreeSet<String> = (0..4)
                        .map(|remainder| {
                            let mut selectors: std::collections::BTreeMap<String, u64> =
                                SELECTOR_NAMES
                                    .into_iter()
                                    .map(|name| (name.to_owned(), 1))
                                    .collect();
                            selectors.insert((*axis).to_owned(), remainder);
                            let case = GeneratedRequestCase {
                                case_id: "case-0000000000000000".to_owned(),
                                global_case_index: 0,
                                domain_case_index: 0,
                                family: declaration.family.to_owned(),
                                requested_observation_kind: (*requested).to_owned(),
                                selectors,
                            };
                            project(&case)
                                .expect("declared request is projectable")
                                .invocation
                                .arguments
                                .to_string()
                        })
                        .collect();
                    if invocations.len() == 1 {
                        inert.push(format!("{} {} {}", declaration.family, requested, axis));
                    }
                }
            }
        }
        assert_eq!(
            inert,
            ["rational-reduction zero-denominator familyParameter"]
        );
    }

    #[test]
    fn affine_success_requests_project_to_distinct_full_row_rank_rectangular_systems() {
        let mut shapes = std::collections::BTreeSet::new();
        for formula_variant in 0..4 {
            let mut selectors: std::collections::BTreeMap<String, u64> = SELECTOR_NAMES
                .into_iter()
                .map(|name| (name.to_owned(), 0))
                .collect();
            selectors.insert("formulaVariant".to_owned(), formula_variant);
            let case = GeneratedRequestCase {
                case_id: "case-0000000000000000".to_owned(),
                global_case_index: 0,
                domain_case_index: 0,
                family: "rational-linear-affine".to_owned(),
                requested_observation_kind: "success".to_owned(),
                selectors,
            };
            let projected = project(&case).expect("affine success is projectable");
            let matrix = projected.invocation.arguments["matrix"]
                .as_array()
                .expect("matrix rows");
            assert_eq!(matrix.len(), 1);
            assert!(matrix[0].as_array().expect("matrix row").len() > 1);
            assert!(shapes.insert(projected.invocation.arguments["matrix"].to_string()));
            let observed = crate::product_api_port::invoke(&projected).expect("classified");
            assert_eq!(observed["observation"]["kind"], "success");
        }
    }
}
