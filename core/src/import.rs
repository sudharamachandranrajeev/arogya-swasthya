//! Reading records from files people get elsewhere: FHIR R4 JSON (a Bundle
//! or one resource), which includes an International Patient Summary (IPS),
//! and C-CDA documents (see [`ccda`](crate::ccda)).
//! Nothing is saved here. The result is a list for the user to review; what
//! they accept is saved with [`Store::save_all`](crate::Store::save_all).
//!
//! Imported resources are made to fit this store: they get new ids, point at
//! the person chosen for the import, lose links to things not kept here
//! (practitioners, encounters, ...; a link's display text stays), and get
//! plain `text` on their codes so the app can show them.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::store::{Result, Store};
use crate::{ccda, fhir};

/// Largest file read, and most records taken from it.
pub const MAX_BYTES: usize = 20 * 1024 * 1024;
const MAX_RECORDS: usize = 5000;

/// Types taken from a file, besides MedicationRequest (made a MedicationStatement).
const TAKEN: &[&str] = &[
    "Condition",
    "MedicationStatement",
    "AllergyIntolerance",
    "Procedure",
    "Immunization",
    "Observation",
    "DiagnosticReport",
];

/// Codeable fields the app shows by their `text`.
const CODES: &[&str] = &[
    "code",
    "vaccineCode",
    "medicationCodeableConcept",
    "clinicalStatus",
];

#[derive(Debug, thiserror::Error)]
pub enum Problem {
    #[error("this file is larger than 20 MB")]
    TooLarge,
    #[error("this is not a FHIR JSON or C-CDA file ({0})")]
    Unreadable(String),
    #[error("nothing in this file can be kept here yet (it holds: {0})")]
    Nothing(String),
    #[error(transparent)]
    Store(#[from] crate::Error),
}

/// One record found in a file, ready for review.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub resource: Value,
    /// A record already kept that looks the same (type, name and date).
    pub duplicate_of: Option<String>,
}

/// What a file held, for the review screen.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub candidates: Vec<Candidate>,
    /// Types in the file that are not kept, with how many of each.
    pub skipped: Vec<(String, usize)>,
}

/// Read a FHIR JSON or C-CDA file into records for `person`, flagging ones
/// the store already has.
pub fn read(store: &Store, bytes: &[u8], person: &str) -> std::result::Result<Found, Problem> {
    if bytes.len() > MAX_BYTES {
        return Err(Problem::TooLarge);
    }
    let resources = if ccda::is_xml(bytes) {
        ccda::resources(bytes).map_err(Problem::Unreadable)?
    } else {
        let root: Value =
            serde_json::from_slice(bytes).map_err(|e| Problem::Unreadable(e.to_string()))?;
        entries(&root)?
    };
    review(store, resources, person)
}

/// Fit resources read elsewhere (a SMART Health Link, say) to `person` and
/// flag ones the store already has.
pub fn review(
    store: &Store,
    resources: Vec<Value>,
    person: &str,
) -> std::result::Result<Found, Problem> {
    let mut skipped: Vec<(String, usize)> = Vec::new();
    let mut taken = Vec::new();
    for r in &resources {
        let t = r["resourceType"].as_str().unwrap_or("?");
        match fit(r, &resources, person) {
            Some(v) if taken.len() < MAX_RECORDS => taken.push(v),
            _ => match skipped.iter_mut().find(|(k, _)| k == t) {
                Some((_, n)) => *n += 1,
                None => skipped.push((t.to_string(), 1)),
            },
        }
    }
    if taken.is_empty() {
        let held = skipped
            .iter()
            .map(|(t, n)| format!("{n} {t}"))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(Problem::Nothing(held));
    }
    let kept = kept_keys(store, person, &taken)?;
    Ok(Found {
        candidates: taken
            .into_iter()
            .map(|resource| Candidate {
                duplicate_of: kept.get(&same_key(&resource)).cloned(),
                resource,
            })
            .collect(),
        skipped,
    })
}

/// [`same_key`] of each record `person` already has, of the types in `taken`.
fn kept_keys(store: &Store, person: &str, taken: &[Value]) -> Result<HashMap<String, String>> {
    let mut types: Vec<&str> = taken
        .iter()
        .filter_map(|r| r["resourceType"].as_str())
        .collect();
    types.sort_unstable();
    types.dedup();
    let mut kept = HashMap::new();
    for t in types {
        for r in store.records(person, t)? {
            if let Some(id) = r["id"].as_str() {
                kept.entry(same_key(&r)).or_insert_with(|| id.to_string());
            }
        }
    }
    Ok(kept)
}

/// The resources in a Bundle (any type, IPS included), or the one resource.
pub(crate) fn entries(root: &Value) -> std::result::Result<Vec<Value>, Problem> {
    match root["resourceType"].as_str() {
        Some("Bundle") => Ok(root["entry"]
            .as_array()
            .map(|es| {
                es.iter()
                    .filter_map(|e| e.get("resource").cloned())
                    .collect()
            })
            .unwrap_or_default()),
        Some(_) => Ok(vec![root.clone()]),
        None => Err(Problem::Unreadable("it has no resourceType".into())),
    }
}

/// Make one resource fit this store, or `None` if it is not a kind kept here.
fn fit(r: &Value, all: &[Value], person: &str) -> Option<Value> {
    let t = r["resourceType"].as_str()?;
    let mut r = if t == "MedicationRequest" {
        as_statement(r)
    } else if TAKEN.contains(&t) {
        r.clone()
    } else {
        return None;
    };
    resolve_medication(&mut r, all);
    let obj = r.as_object_mut()?;
    for k in [
        "id",
        "meta",
        "text",
        "contained",
        "extension",
        "modifierExtension",
    ] {
        obj.remove(k);
    }
    let person_field = if matches!(
        obj["resourceType"].as_str(),
        Some("AllergyIntolerance" | "Immunization")
    ) {
        "patient"
    } else {
        "subject"
    };
    obj.remove("subject");
    obj.remove("patient");
    dates(obj);
    unlink(&mut r);
    for c in CODES {
        give_text(&mut r[*c]);
    }
    if let Some(ms) = r.get_mut("reaction").and_then(|x| x.as_array_mut()) {
        for m in ms
            .iter_mut()
            .filter_map(|x| x.get_mut("manifestation")?.as_array_mut())
        {
            m.iter_mut().for_each(give_text);
        }
    }
    r[person_field] = json!({ "reference": format!("Patient/{person}") });
    Some(r)
}

/// Dates in the shape the screens read: a medicine's as a period (started,
/// ended), everything else as one date (a period's start).
fn dates(obj: &mut Map<String, Value>) {
    if obj["resourceType"] == "MedicationStatement" {
        if let Some(d) = obj.remove("effectiveDateTime") {
            obj.entry("effectivePeriod")
                .or_insert_with(|| json!({ "start": d }));
        }
        return;
    }
    for base in ["onset", "performed", "effective", "occurrence"] {
        let (at, period) = (format!("{base}DateTime"), format!("{base}Period"));
        if obj.contains_key(&at) {
            continue;
        }
        if let Some(start) = obj.get(&period).and_then(|p| p.get("start")).cloned() {
            obj.remove(&period);
            obj.insert(at, start);
        }
    }
}

/// MedicationRequest to the MedicationStatement this app keeps.
fn as_statement(r: &Value) -> Value {
    let mut m = Map::new();
    m.insert("resourceType".into(), json!("MedicationStatement"));
    let status = match r["status"].as_str() {
        Some("active" | "on-hold") => "active",
        Some("completed") => "completed",
        Some("stopped" | "cancelled") => "stopped",
        _ => "unknown",
    };
    m.insert("status".into(), json!(status));
    for k in [
        "medicationCodeableConcept",
        "medicationReference",
        "contained",
        "reasonCode",
        "note",
    ] {
        if let Some(v) = r.get(k) {
            m.insert(k.into(), v.clone());
        }
    }
    if let Some(d) = r["authoredOn"].as_str() {
        m.insert("effectiveDateTime".into(), json!(d));
    }
    if let Some(d) = r.get("dosageInstruction") {
        m.insert("dosage".into(), d.clone());
    }
    Value::Object(m)
}

/// Replace `medicationReference` with the code of the Medication it names,
/// found in the same file or in `contained`.
fn resolve_medication(r: &mut Value, all: &[Value]) {
    let Some(reference) = r["medicationReference"]["reference"]
        .as_str()
        .map(String::from)
    else {
        return;
    };
    let id = reference
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim_start_matches('#');
    let contained = r["contained"].as_array().cloned().unwrap_or_default();
    let found = contained
        .iter()
        .chain(all.iter())
        .find(|m| m["resourceType"] == "Medication" && m["id"] == id)
        .and_then(|m| m.get("code").cloned());
    let code = found.unwrap_or_else(|| match r["medicationReference"]["display"].as_str() {
        Some(d) => json!({ "text": d }),
        None => json!({ "text": "Medicine" }),
    });
    r["medicationCodeableConcept"] = code;
    r.as_object_mut().map(|o| o.remove("medicationReference"));
}

/// Links to resources not kept here become their display text, or go.
fn unlink(v: &mut Value) {
    match v {
        Value::Object(o) => {
            let keys: Vec<String> = o.keys().cloned().collect();
            for k in keys {
                let is_link = o[&k].get("reference").is_some_and(Value::is_string);
                if is_link {
                    match o[&k]["display"].as_str() {
                        Some(d) => o[&k] = json!({ "display": d }),
                        None => {
                            o.remove(&k);
                        }
                    }
                } else if let Some(child) = o.get_mut(&k) {
                    unlink(child);
                }
            }
        }
        Value::Array(a) => {
            a.retain(|x| {
                !(x.get("reference").is_some_and(Value::is_string) && x.get("display").is_none())
            });
            for x in a.iter_mut() {
                if x.get("reference").is_some_and(Value::is_string) {
                    *x = json!({ "display": x["display"].clone() });
                } else {
                    unlink(x);
                }
            }
        }
        _ => {}
    }
}

/// Give a CodeableConcept plain text from its first coding's display (or code).
fn give_text(c: &mut Value) {
    if !c.is_object() || c.get("text").is_some_and(Value::is_string) {
        return;
    }
    let first = &c["coding"][0];
    if let Some(t) = first["display"].as_str().or(first["code"].as_str()) {
        c["text"] = json!(t);
    }
}

/// What makes two records "the same" for the duplicate warning: type, the
/// name shown, and the date that orders it (both lower case, trimmed).
pub fn same_key(r: &Value) -> String {
    let name = ["code", "vaccineCode", "medicationCodeableConcept"]
        .iter()
        .find_map(|f| r[*f]["text"].as_str())
        .unwrap_or("");
    format!(
        "{}|{}|{}",
        r["resourceType"].as_str().unwrap_or(""),
        name.trim().to_lowercase(),
        fhir::sort_date(r)
            .unwrap_or_default()
            .get(..10)
            .unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::Timestamp;

    // A small synthetic IPS-shaped bundle: no real person.
    fn ips(person_ref: &str) -> Value {
        json!({"resourceType": "Bundle", "type": "document", "entry": [
            {"resource": {"resourceType": "Composition", "id": "c1", "title": "Patient summary"}},
            {"resource": {"resourceType": "Patient", "id": "x1", "name": [{"text": "Sample Person"}]}},
            {"resource": {"resourceType": "Practitioner", "id": "d1"}},
            {"resource": {"resourceType": "Condition", "id": "k1", "subject": {"reference": person_ref},
                "asserter": {"reference": "Practitioner/d1"}, "recorder": {"reference": "Practitioner/d1", "display": "Dr Example"},
                "text": {"div": "<div>narrative</div>"},
                "code": {"coding": [{"system": "http://snomed.info/sct", "code": "195967001", "display": "Asthma"}]},
                "onsetDateTime": "2015-03-01"}},
            {"resource": {"resourceType": "MedicationRequest", "id": "m1", "status": "active", "subject": {"reference": person_ref},
                "medicationReference": {"reference": "Medication/med1"}, "authoredOn": "2021-03-15",
                "dosageInstruction": [{"text": "One tablet twice a day"}]}},
            {"resource": {"resourceType": "Medication", "id": "med1", "code": {"coding": [{"display": "Metformin 500 mg"}]}}},
            {"resource": {"resourceType": "Immunization", "id": "i1", "patient": {"reference": person_ref}, "status": "completed",
                "vaccineCode": {"text": "Influenza"}, "occurrenceDateTime": "2025-10-04"}}
        ]})
    }

    #[test]
    fn reads_an_ips_bundle_into_records_for_the_chosen_person() {
        let mut s = Store::memory().unwrap();
        let me = s
            .save(
                json!({"resourceType": "Patient", "name": [{"text": "Test Person"}]}),
                Timestamp::UNIX_EPOCH,
            )
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let bytes = serde_json::to_vec(&ips("Patient/x1")).unwrap();
        let found = read(&s, &bytes, &me).unwrap();
        assert_eq!(found.candidates.len(), 3);
        let skipped: Vec<&str> = found.skipped.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            skipped,
            ["Composition", "Patient", "Practitioner", "Medication"]
        );

        let c = &found.candidates[0].resource;
        assert_eq!(c["subject"]["reference"], format!("Patient/{me}"));
        assert_eq!(c["code"]["text"], "Asthma");
        assert_eq!(c["recorder"], json!({"display": "Dr Example"}));
        assert!(c.get("asserter").is_none() && c.get("id").is_none() && c.get("text").is_none());
        let m = &found.candidates[1].resource;
        assert_eq!(m["resourceType"], "MedicationStatement");
        assert_eq!(m["medicationCodeableConcept"]["text"], "Metformin 500 mg");
        assert_eq!(m["effectivePeriod"]["start"], "2021-03-15");
        assert_eq!(m["dosage"][0]["text"], "One tablet twice a day");
        assert_eq!(
            found.candidates[2].resource["patient"]["reference"],
            format!("Patient/{me}")
        );
        assert!(found.candidates.iter().all(|c| c.duplicate_of.is_none()));

        // Accepted as one change; importing again flags every record; one Undo removes them.
        let saved = s
            .save_all(
                found.candidates.into_iter().map(|c| c.resource).collect(),
                "import",
                Timestamp::UNIX_EPOCH,
            )
            .unwrap();
        assert_eq!(saved.len(), 3);
        let again = read(&s, &bytes, &me).unwrap();
        assert!(again.candidates.iter().all(|c| c.duplicate_of.is_some()));
        s.undo().unwrap();
        assert!(s.timeline(&me).unwrap().is_empty());
    }

    #[test]
    fn finds_contained_medicines_and_puts_dates_where_the_screens_read_them() {
        let mut s = Store::memory().unwrap();
        let me = s
            .save(
                json!({"resourceType": "Patient", "name": [{"text": "Test Person"}]}),
                Timestamp::UNIX_EPOCH,
            )
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let bundle = json!({"resourceType": "Bundle", "type": "collection", "entry": [
            {"resource": {"resourceType": "MedicationRequest", "status": "stopped",
                "contained": [{"resourceType": "Medication", "id": "m", "code": {"text": "Sample tablet"}}],
                "medicationReference": {"reference": "#m"}}},
            {"resource": {"resourceType": "MedicationStatement", "status": "active",
                "medicationCodeableConcept": {"text": "Sample syrup"}, "effectiveDateTime": "2024-01-02"}},
            {"resource": {"resourceType": "Procedure", "status": "completed", "code": {"text": "Sample procedure"},
                "performedPeriod": {"start": "2019-05-06", "end": "2019-05-07"}}}
        ]});
        let found = read(&s, &serde_json::to_vec(&bundle).unwrap(), &me).unwrap();
        let [a, b, c] = &found.candidates[..] else {
            panic!("three records")
        };
        assert_eq!(
            a.resource["medicationCodeableConcept"]["text"],
            "Sample tablet"
        );
        assert_eq!(a.resource["status"], "stopped");
        assert!(a.resource.get("contained").is_none());
        assert_eq!(b.resource["effectivePeriod"]["start"], "2024-01-02");
        assert_eq!(c.resource["performedDateTime"], "2019-05-06");
        assert!(c.resource.get("performedPeriod").is_none());
    }

    #[test]
    fn reads_a_ccda_document_the_same_way() {
        let mut s = Store::memory().unwrap();
        let me = s
            .save(
                json!({"resourceType": "Patient", "name": [{"text": "Test Person"}]}),
                Timestamp::UNIX_EPOCH,
            )
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let found = read(&s, crate::ccda::tests::SAMPLE.as_bytes(), &me).unwrap();
        assert_eq!(found.candidates.len(), 7);
        let person = format!("Patient/{me}");
        for c in &found.candidates {
            let r = &c.resource;
            assert!(r["subject"]["reference"] == person || r["patient"]["reference"] == person);
        }
        let saved = s
            .save_all(
                found.candidates.into_iter().map(|c| c.resource).collect(),
                "import",
                Timestamp::UNIX_EPOCH,
            )
            .unwrap();
        assert_eq!(saved.len(), 7);
        assert_eq!(s.search(Some(&me), "penicillin", 5).unwrap().len(), 1);
    }

    #[test]
    fn says_plainly_what_it_cannot_read() {
        let s = Store::memory().unwrap();
        assert!(matches!(
            read(&s, b"<xml/>", "p"),
            Err(Problem::Unreadable(_))
        ));
        assert!(matches!(read(&s, b"{}", "p"), Err(Problem::Unreadable(_))));
        let only = serde_json::to_vec(
            &json!({"resourceType": "Bundle", "entry": [{"resource": {"resourceType": "Claim"}}]}),
        )
        .unwrap();
        assert_eq!(
            read(&s, &only, "p").unwrap_err().to_string(),
            "nothing in this file can be kept here yet (it holds: 1 Claim)"
        );
        assert!(matches!(
            read(&s, &vec![b' '; MAX_BYTES + 1], "p"),
            Err(Problem::TooLarge)
        ));
    }
}
