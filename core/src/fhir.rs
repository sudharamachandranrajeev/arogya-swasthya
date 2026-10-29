//! The FHIR R4 resources phase 1 keeps, and the few fields the store indexes.
//! Resources stay as JSON; only the person they belong to and a sort date are
//! pulled out into columns.

use serde_json::Value;

/// Resource types the store accepts, with the field that points at the person.
const KINDS: &[(&str, &str)] = &[
    ("Patient", ""),
    ("Condition", "subject"),
    ("MedicationStatement", "subject"),
    ("AllergyIntolerance", "patient"),
    ("Procedure", "subject"),
    ("Immunization", "patient"),
    ("Observation", "subject"),
    ("DiagnosticReport", "subject"),
];

/// Where each type keeps the date that orders it on the timeline, first match wins.
const DATES: &[(&str, &[&str])] = &[
    (
        "Condition",
        &["onsetDateTime", "onsetPeriod.start", "recordedDate"],
    ),
    (
        "MedicationStatement",
        &["effectiveDateTime", "effectivePeriod.start", "dateAsserted"],
    ),
    ("AllergyIntolerance", &["onsetDateTime", "recordedDate"]),
    ("Procedure", &["performedDateTime", "performedPeriod.start"]),
    ("Immunization", &["occurrenceDateTime"]),
    (
        "Observation",
        &["effectiveDateTime", "effectivePeriod.start", "issued"],
    ),
    (
        "DiagnosticReport",
        &["effectiveDateTime", "effectivePeriod.start", "issued"],
    ),
];

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Invalid {
    #[error("not a FHIR resource: it has no resourceType")]
    NoType,
    #[error("{0} records are not kept yet")]
    Unsupported(String),
    #[error("a {0} must say whose record it is in `{1}.reference` (Patient/<id>)")]
    NoPerson(String, &'static str),
    #[error("the id must be 1-64 letters, digits, '-' or '.'")]
    BadId,
}

pub fn resource_type(r: &Value) -> Result<&str, Invalid> {
    let t = r
        .get("resourceType")
        .and_then(Value::as_str)
        .ok_or(Invalid::NoType)?;
    KINDS
        .iter()
        .any(|(k, _)| *k == t)
        .then_some(t)
        .ok_or_else(|| Invalid::Unsupported(t.into()))
}

/// The person a resource belongs to: its own id for a Patient, otherwise the
/// id in its subject/patient reference.
pub fn person_of(r: &Value) -> Result<Option<String>, Invalid> {
    let t = resource_type(r)?;
    let field = KINDS
        .iter()
        .find(|(k, _)| *k == t)
        .map(|(_, f)| *f)
        .unwrap_or("");
    if field.is_empty() {
        return Ok(r.get("id").and_then(Value::as_str).map(String::from));
    }
    let id = r
        .get(field)
        .and_then(|s| s.get("reference"))
        .and_then(Value::as_str)
        .and_then(|s| s.strip_prefix("Patient/"))
        .filter(|id| valid_id(id))
        .ok_or_else(|| Invalid::NoPerson(t.into(), field))?;
    Ok(Some(id.into()))
}

/// FHIR id rule: [A-Za-z0-9-.]{1,64}.
pub fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
}

/// The date that orders this resource on the timeline, as written (FHIR dates sort as text).
pub fn sort_date(r: &Value) -> Option<String> {
    let t = r.get("resourceType")?.as_str()?;
    let paths = DATES.iter().find(|(k, _)| *k == t)?.1;
    paths.iter().find_map(|p| {
        let v = p.split('.').try_fold(r, |v, key| v.get(key))?;
        v.as_str().map(String::from)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn finds_person_and_date() {
        let c = json!({"resourceType": "Condition", "subject": {"reference": "Patient/p1"}, "recordedDate": "2024-02-01", "onsetDateTime": "2019-05"});
        assert_eq!(person_of(&c).unwrap().as_deref(), Some("p1"));
        assert_eq!(sort_date(&c).as_deref(), Some("2019-05"));
        let i = json!({"resourceType": "Immunization", "patient": {"reference": "Patient/p1"}, "occurrenceDateTime": "2025-10-02"});
        assert_eq!(person_of(&i).unwrap().as_deref(), Some("p1"));
        let m = json!({"resourceType": "MedicationStatement", "subject": {"reference": "Patient/p1"}, "effectivePeriod": {"start": "2023-01-10"}});
        assert_eq!(sort_date(&m).as_deref(), Some("2023-01-10"));
    }

    #[test]
    fn refuses_what_it_cannot_place() {
        assert_eq!(resource_type(&json!({})), Err(Invalid::NoType));
        assert_eq!(
            resource_type(&json!({"resourceType": "Claim"})),
            Err(Invalid::Unsupported("Claim".into()))
        );
        let orphan = json!({"resourceType": "Observation", "subject": {"reference": "Group/g"}});
        assert_eq!(
            person_of(&orphan),
            Err(Invalid::NoPerson("Observation".into(), "subject"))
        );
        assert!(!valid_id("a b") && !valid_id("") && valid_id("p-1.x"));
    }
}
