//! One person's records as an International Patient Summary: a FHIR R4
//! document Bundle with a Composition first, then the Patient and the
//! records each section lists. Every entry gets a `urn:uuid:` address and
//! references between them are rewritten to match, as documents require.
//!
//! Sections follow the IPS Implementation Guide (HL7, 2.0): medicines,
//! allergies and problems are always present (with an empty reason when
//! nothing is recorded); immunizations, procedures, results and vital signs
//! only when there is something to list.

use std::collections::HashMap;

use jiff::Timestamp;
use serde_json::{Value, json};

use crate::store::{Result, Store};

const LOINC: &str = "http://loinc.org";
const OBS_CATEGORY: &str = "http://terminology.hl7.org/CodeSystem/observation-category";
const EMPTY_REASON: &str = "http://terminology.hl7.org/CodeSystem/list-empty-reason";

struct Section {
    title: &'static str,
    code: &'static str,
    display: &'static str,
    kind: &'static str,
    /// For Observations: the category code that puts one in this section.
    category: Option<&'static str>,
    required: bool,
}

const SECTIONS: &[Section] = &[
    Section {
        title: "Medication Summary",
        code: "10160-0",
        display: "History of Medication use Narrative",
        kind: "MedicationStatement",
        category: None,
        required: true,
    },
    Section {
        title: "Allergies and Intolerances",
        code: "48765-2",
        display: "Allergies and adverse reactions Document",
        kind: "AllergyIntolerance",
        category: None,
        required: true,
    },
    Section {
        title: "Problem List",
        code: "11450-4",
        display: "Problem list - Reported",
        kind: "Condition",
        category: None,
        required: true,
    },
    Section {
        title: "History of Immunizations",
        code: "11369-6",
        display: "History of Immunization Narrative",
        kind: "Immunization",
        category: None,
        required: false,
    },
    Section {
        title: "History of Procedures",
        code: "47519-4",
        display: "History of Procedures Document",
        kind: "Procedure",
        category: None,
        required: false,
    },
    Section {
        title: "Results",
        code: "30954-2",
        display: "Relevant diagnostic tests/laboratory data Narrative",
        kind: "Observation",
        category: Some("laboratory"),
        required: false,
    },
    Section {
        title: "Vital Signs",
        code: "8716-3",
        display: "Vital signs",
        kind: "Observation",
        category: Some("vital-signs"),
        required: false,
    },
];

fn has_category(r: &Value, code: &str) -> bool {
    r["category"].as_array().into_iter().flatten().any(|c| {
        c["coding"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|x| x["system"] == OBS_CATEGORY && x["code"] == code)
    })
}

/// The name a person would recognise, for the section's narrative.
fn name(r: &Value) -> String {
    [
        "/code/text",
        "/medicationCodeableConcept/text",
        "/vaccineCode/text",
        "/code/coding/0/display",
    ]
    .iter()
    .find_map(|p| r.pointer(p).and_then(Value::as_str))
    .unwrap_or("Unnamed")
    .to_string()
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Point every `reference` that names a kept resource at its new address.
fn relink(v: &mut Value, urls: &HashMap<String, String>) {
    match v {
        Value::Object(o) => {
            if let Some(Value::String(r)) = o.get_mut("reference")
                && let Some(u) = urls.get(r.as_str())
            {
                *r = u.clone();
            }
            for x in o.values_mut() {
                relink(x, urls);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| relink(x, urls)),
        _ => {}
    }
}

impl Store {
    /// `person`'s records as an IPS document Bundle.
    pub fn export_ips(&self, person: &str, now: Timestamp) -> Result<Value> {
        let patient = self.get(person)?;
        let records = self.timeline(person)?;
        let mut picked: Vec<(usize, Vec<Value>)> = Vec::new();
        for (i, s) in SECTIONS.iter().enumerate() {
            let rows: Vec<Value> = records
                .iter()
                .filter(|r| {
                    r["resourceType"] == s.kind && s.category.is_none_or(|c| has_category(r, c))
                })
                .cloned()
                .collect();
            if s.required || !rows.is_empty() {
                picked.push((i, rows));
            }
        }

        // New addresses for everything in the document.
        let new_url = || format!("urn:uuid:{}", uuid::Uuid::now_v7());
        let mut urls = HashMap::new();
        let key = |r: &Value| {
            format!(
                "{}/{}",
                r["resourceType"].as_str().unwrap_or(""),
                r["id"].as_str().unwrap_or("")
            )
        };
        urls.insert(key(&patient), new_url());
        for (_, rows) in &picked {
            for r in rows {
                urls.insert(key(r), new_url());
            }
        }
        let patient_url = urls[&key(&patient)].clone();

        let sections: Vec<Value> = picked
            .iter()
            .map(|(i, rows)| {
                let s = &SECTIONS[*i];
                let mut sec = json!({
                    "title": s.title,
                    "code": { "coding": [{ "system": LOINC, "code": s.code, "display": s.display }] },
                });
                if rows.is_empty() {
                    sec["text"] = json!({ "status": "generated", "div": "<div xmlns=\"http://www.w3.org/1999/xhtml\">No information recorded.</div>" });
                    sec["emptyReason"] = json!({ "coding": [{ "system": EMPTY_REASON, "code": "unavailable", "display": "Unavailable" }] });
                } else {
                    let items: String = rows.iter().map(|r| format!("<li>{}</li>", escape(&name(r)))).collect();
                    sec["text"] = json!({ "status": "generated", "div": format!("<div xmlns=\"http://www.w3.org/1999/xhtml\"><ul>{items}</ul></div>") });
                    sec["entry"] = rows.iter().map(|r| json!({ "reference": urls[&key(r)] })).collect();
                }
                sec
            })
            .collect();

        let composition_url = new_url();
        let composition = json!({
            "resourceType": "Composition",
            "id": composition_url.trim_start_matches("urn:uuid:"),
            "status": "final",
            "type": { "coding": [{ "system": LOINC, "code": "60591-5", "display": "Patient summary Document" }] },
            "subject": { "reference": patient_url },
            "date": now.to_string(),
            // The patient keeps these records and is their author here.
            "author": [{ "reference": patient_url }],
            "title": "Patient Summary",
            "section": sections,
        });

        let mut entry = vec![json!({ "fullUrl": composition_url, "resource": composition })];
        let mut add = |mut r: Value| {
            let url = urls[&key(&r)].clone();
            relink(&mut r, &urls);
            r["id"] = json!(url.trim_start_matches("urn:uuid:"));
            entry.push(json!({ "fullUrl": url, "resource": r }));
        };
        add(patient);
        for (_, rows) in picked {
            for r in rows {
                add(r);
            }
        }
        Ok(json!({
            "resourceType": "Bundle",
            "identifier": { "system": "urn:ietf:rfc:3986", "value": new_url() },
            "type": "document",
            "timestamp": now.to_string(),
            "entry": entry,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_summary_is_a_document_with_linked_sections() {
        let mut s = Store::memory().unwrap();
        let now: Timestamp = "2026-10-30T10:00:00Z".parse().unwrap();
        let p = s
            .save(
                json!({"resourceType": "Patient", "name": [{"text": "Alex Demo"}]}),
                now,
            )
            .unwrap();
        let pid = p["id"].as_str().unwrap().to_string();
        let subject = json!({"reference": format!("Patient/{pid}")});
        s.save(json!({"resourceType": "Condition", "subject": subject, "code": {"text": "Asthma <mild>"}}), now).unwrap();
        s.save(json!({"resourceType": "Observation", "status": "final", "subject": subject, "code": {"text": "HbA1c"},
            "category": [{"coding": [{"system": OBS_CATEGORY, "code": "laboratory"}]}], "effectiveDateTime": "2026-01-01"}), now).unwrap();
        s.save(json!({"resourceType": "Observation", "status": "final", "subject": subject, "code": {"text": "Headache"},
            "category": [{"coding": [{"system": OBS_CATEGORY, "code": "survey"}]}], "effectiveDateTime": "2026-01-02"}), now).unwrap();

        let b = s.export_ips(&pid, now).unwrap();
        assert_eq!(b["type"], "document");
        let entries = b["entry"].as_array().unwrap();
        let comp = &entries[0]["resource"];
        assert_eq!(comp["resourceType"], "Composition");
        assert_eq!(entries[1]["resource"]["resourceType"], "Patient");
        // Medicines, allergies, problems always; results because there is one.
        let titles: Vec<&str> = comp["section"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["title"].as_str().unwrap())
            .collect();
        assert_eq!(
            titles,
            [
                "Medication Summary",
                "Allergies and Intolerances",
                "Problem List",
                "Results"
            ]
        );
        assert_eq!(
            comp["section"][0]["emptyReason"]["coding"][0]["code"],
            "unavailable"
        );
        // The symptom is not in any IPS section, so it is left out.
        assert_eq!(entries.len(), 4);

        // Every reference resolves to an entry in the document.
        let urls: Vec<&str> = entries
            .iter()
            .map(|e| e["fullUrl"].as_str().unwrap())
            .collect();
        assert!(urls.iter().all(|u| u.starts_with("urn:uuid:")));
        assert_eq!(comp["subject"]["reference"], urls[1]);
        let problem = &comp["section"][2]["entry"][0]["reference"];
        let cond = entries.iter().find(|e| e["fullUrl"] == *problem).unwrap();
        assert_eq!(cond["resource"]["subject"]["reference"], urls[1]);
        assert!(
            comp["section"][2]["text"]["div"]
                .as_str()
                .unwrap()
                .contains("Asthma &lt;mild&gt;")
        );
    }
}
