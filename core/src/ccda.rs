//! Reading the common sections of a C-CDA document (the XML summaries many
//! US hospitals and patient portals hand out) into FHIR resources: problems,
//! allergies, medicines, vaccines, results and procedures. Other sections
//! are left alone. [`import`](crate::import) then fits the resources to this
//! store, the same as ones read from FHIR JSON.
//!
//! The XML is read with DTDs refused, so a file cannot pull in other files.

use std::collections::HashMap;

use roxmltree::{Document, Node};
use serde_json::{Value, json};

const V3: &str = "urn:hl7-org:v3";
const XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";

#[derive(Clone, Copy)]
enum Section {
    Problems,
    Allergies,
    Medicines,
    Vaccines,
    Results,
    Procedures,
}

/// Section template ids (with and without "entries required") and LOINC codes.
const SECTIONS: &[(&str, &str, Section)] = &[
    (
        "2.16.840.1.113883.10.20.22.2.5",
        "11450-4",
        Section::Problems,
    ),
    (
        "2.16.840.1.113883.10.20.22.2.6",
        "48765-2",
        Section::Allergies,
    ),
    (
        "2.16.840.1.113883.10.20.22.2.1",
        "10160-0",
        Section::Medicines,
    ),
    (
        "2.16.840.1.113883.10.20.22.2.2",
        "11369-6",
        Section::Vaccines,
    ),
    (
        "2.16.840.1.113883.10.20.22.2.3",
        "30954-2",
        Section::Results,
    ),
    (
        "2.16.840.1.113883.10.20.22.2.7",
        "47519-4",
        Section::Procedures,
    ),
];

/// Code systems by OID, as FHIR names them.
const SYSTEMS: &[(&str, &str)] = &[
    ("2.16.840.1.113883.6.96", "http://snomed.info/sct"),
    ("2.16.840.1.113883.6.1", "http://loinc.org"),
    (
        "2.16.840.1.113883.6.88",
        "http://www.nlm.nih.gov/research/umls/rxnorm",
    ),
    ("2.16.840.1.113883.12.292", "http://hl7.org/fhir/sid/cvx"),
    (
        "2.16.840.1.113883.6.90",
        "http://hl7.org/fhir/sid/icd-10-cm",
    ),
];

const REACTION: &str = "2.16.840.1.113883.10.20.22.4.9";

/// Does this look like XML (after any byte-order mark and blank space)?
pub(crate) fn is_xml(bytes: &[u8]) -> bool {
    let b = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    b.iter().find(|c| !c.is_ascii_whitespace()) == Some(&b'<')
}

/// The records in a C-CDA document, as FHIR resources without a subject.
pub(crate) fn resources(bytes: &[u8]) -> Result<Vec<Value>, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "it is not UTF-8 text".to_string())?;
    let doc = Document::parse(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
    let root = doc.root_element();
    if !is(root, "ClinicalDocument") {
        return Err("it is XML, but not a C-CDA clinical document".into());
    }
    let cx = Cx {
        narrative: doc
            .descendants()
            .filter_map(|n| Some((n.attribute("ID")?, n)))
            .collect(),
    };
    let mut out = Vec::new();
    for section in root.descendants().filter(|n| is(*n, "section")) {
        let Some(kind) = section_kind(section) else {
            continue;
        };
        for entry in kids(section, "entry") {
            out.extend(cx.entry(kind, entry));
        }
    }
    Ok(out)
}

fn section_kind(s: Node) -> Option<Section> {
    let roots: Vec<&str> = kids(s, "templateId")
        .filter_map(|t| t.attribute("root"))
        .collect();
    let code = kid(s, "code").and_then(|c| c.attribute("code"));
    SECTIONS.iter().find_map(|(root, loinc, kind)| {
        let by_template = roots
            .iter()
            .any(|r| r == root || r.strip_prefix(root) == Some(".1"));
        (by_template || code == Some(loinc)).then_some(*kind)
    })
}

/// The narrative text blocks, by their `ID`, that codes may point at.
struct Cx<'a, 'i> {
    narrative: HashMap<&'a str, Node<'a, 'i>>,
}

impl Cx<'_, '_> {
    fn entry(&self, kind: Section, entry: Node) -> Vec<Value> {
        match kind {
            Section::Problems => self.problem(entry).into_iter().collect(),
            Section::Allergies => self.allergy(entry).into_iter().collect(),
            Section::Medicines => self.medicine(entry).into_iter().collect(),
            Section::Vaccines => self.vaccine(entry).into_iter().collect(),
            Section::Results => entry
                .descendants()
                .filter(|n| is(*n, "observation") && kid(*n, "value").is_some())
                .filter_map(|o| self.result(o, entry))
                .collect(),
            Section::Procedures => self.procedure(entry).into_iter().collect(),
        }
    }

    fn problem(&self, entry: Node) -> Option<Value> {
        let obs = first(entry, "observation").filter(|o| !negated(*o))?;
        let code = self.concept(kid(obs, "value")?)?;
        let ended = kid(obs, "effectiveTime")
            .and_then(|t| kid(t, "high"))
            .and_then(|h| h.attribute("value"));
        let concern_done =
            first(entry, "statusCode").and_then(|s| s.attribute("code")) == Some("completed");
        let status = if ended.is_some() || concern_done {
            "resolved"
        } else {
            "active"
        };
        let mut r = json!({
            "resourceType": "Condition",
            "code": code,
            "clinicalStatus": {"coding": [{
                "system": "http://terminology.hl7.org/CodeSystem/condition-clinical", "code": status
            }]},
        });
        set(
            &mut r,
            "onsetDateTime",
            when(obs).or_else(|| when(first(entry, "act")?)),
        );
        Some(r)
    }

    fn allergy(&self, entry: Node) -> Option<Value> {
        let obs = first(entry, "observation").filter(|o| !negated(*o))?;
        let substance = first(obs, "playingEntity").and_then(|p| kid(p, "code"));
        let code = substance
            .and_then(|c| self.concept(c))
            .or_else(|| self.concept(kid(obs, "value")?))?;
        let mut r = json!({"resourceType": "AllergyIntolerance", "code": code});
        let reaction = obs
            .descendants()
            .filter(|n| is(*n, "observation") && has_template(*n, REACTION))
            .find_map(|n| self.concept(kid(n, "value")?));
        if let Some(m) = reaction {
            r["reaction"] = json!([{ "manifestation": [m] }]);
        }
        set(&mut r, "onsetDateTime", when(obs));
        Some(r)
    }

    fn medicine(&self, entry: Node) -> Option<Value> {
        let sa = first(entry, "substanceAdministration").filter(|n| !negated(*n))?;
        let code = self.concept(product(sa)?)?;
        let period = kids(sa, "effectiveTime")
            .find(|t| kid(*t, "low").is_some() || kid(*t, "high").is_some());
        let at = |end: &str| {
            period
                .and_then(|t| kid(t, end))
                .and_then(|n| n.attribute("value"))
                .and_then(date)
        };
        let (start, end) = (at("low"), at("high"));
        let status = match kid(sa, "statusCode").and_then(|s| s.attribute("code")) {
            Some("active") => "active",
            Some("completed") => "completed",
            Some("aborted" | "cancelled") => "stopped",
            Some("suspended") => "on-hold",
            _ if end.is_some() => "completed",
            _ => "active",
        };
        let mut r = json!({"resourceType": "MedicationStatement", "status": status, "medicationCodeableConcept": code});
        if start.is_some() || end.is_some() {
            let mut p = json!({});
            set(&mut p, "start", start);
            set(&mut p, "end", end);
            r["effectivePeriod"] = p;
        }
        if let Some(t) = kid(sa, "text").and_then(|t| self.words(t)) {
            r["dosage"] = json!([{ "text": t }]);
        }
        Some(r)
    }

    fn vaccine(&self, entry: Node) -> Option<Value> {
        let sa = first(entry, "substanceAdministration").filter(|n| !negated(*n))?;
        let mut r = json!({
            "resourceType": "Immunization",
            "status": "completed",
            "vaccineCode": self.concept(product(sa)?)?,
        });
        set(&mut r, "occurrenceDateTime", when(sa));
        Some(r)
    }

    fn result(&self, obs: Node, entry: Node) -> Option<Value> {
        let mut r = json!({
            "resourceType": "Observation",
            "status": "final",
            "category": [{"coding": [{
                "system": "http://terminology.hl7.org/CodeSystem/observation-category", "code": "laboratory"
            }]}],
            "code": self.concept(kid(obs, "code")?)?,
        });
        let v = kid(obs, "value")?;
        match v
            .attribute((XSI, "type"))
            .map(|t| t.rsplit(':').next().unwrap_or(t))
        {
            Some("PQ") => {
                let mut q = json!({ "value": number(v.attribute("value")?)? });
                set(
                    &mut q,
                    "unit",
                    v.attribute("unit").filter(|u| *u != "1").map(String::from),
                );
                r["valueQuantity"] = q;
            }
            Some("CD" | "CE" | "CO") => r["valueCodeableConcept"] = self.concept(v)?,
            _ => r["valueString"] = json!(self.words(v)?),
        }
        let range = first(obs, "referenceRange").and_then(|n| first(n, "value"));
        if let Some(range) = range {
            let mut rr = json!({});
            for end in ["low", "high"] {
                if let Some(x) = kid(range, end).and_then(|n| number(n.attribute("value")?)) {
                    rr[end] = json!({ "value": x });
                }
            }
            if rr.as_object().is_some_and(|o| !o.is_empty()) {
                r["referenceRange"] = json!([rr]);
            }
        }
        set(
            &mut r,
            "effectiveDateTime",
            when(obs).or_else(|| when(first(entry, "organizer")?)),
        );
        Some(r)
    }

    fn procedure(&self, entry: Node) -> Option<Value> {
        let p = ["procedure", "act", "observation"]
            .iter()
            .find_map(|n| first(entry, n))
            .filter(|n| !negated(*n))?;
        let status = match kid(p, "statusCode").and_then(|s| s.attribute("code")) {
            Some("active") => "in-progress",
            Some("aborted") => "stopped",
            _ => "completed",
        };
        let mut r = json!({"resourceType": "Procedure", "status": status, "code": self.concept(kid(p, "code")?)?});
        set(&mut r, "performedDateTime", when(p));
        Some(r)
    }

    /// A CodeableConcept from a coded element: its words (original text,
    /// else display name), and its code. `None` when it has neither.
    fn concept(&self, c: Node) -> Option<Value> {
        let text = kid(c, "originalText")
            .and_then(|t| self.words(t))
            .or_else(|| c.attribute("displayName").map(String::from))
            .or_else(|| {
                kids(c, "translation").find_map(|t| t.attribute("displayName").map(String::from))
            });
        let coding = c.attribute("code").map(|code| {
            let oid = c.attribute("codeSystem").unwrap_or("");
            let system = SYSTEMS
                .iter()
                .find(|(o, _)| *o == oid)
                .map_or_else(|| format!("urn:oid:{oid}"), |(_, s)| s.to_string());
            let mut one = json!({ "system": system, "code": code });
            set(
                &mut one,
                "display",
                c.attribute("displayName").map(String::from),
            );
            one
        });
        let mut v = json!({});
        set(
            &mut v,
            "text",
            text.or_else(|| Some(coding.as_ref()?["code"].as_str()?.to_string())),
        );
        if let Some(one) = coding {
            v["coding"] = json!([one]);
        }
        v.get("text").is_some().then_some(v)
    }

    /// The words in an element, or in the narrative it points at.
    fn words(&self, n: Node) -> Option<String> {
        let target = kid(n, "reference")
            .and_then(|r| r.attribute("value"))
            .and_then(|v| self.narrative.get(v.trim_start_matches('#')).copied())
            .unwrap_or(n);
        let s = target
            .descendants()
            .filter(|d| d.is_text())
            .filter_map(|d| d.text())
            .collect::<Vec<_>>()
            .join(" ");
        let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
        (!s.is_empty()).then_some(s)
    }
}

fn is(n: Node, name: &str) -> bool {
    n.is_element() && n.tag_name().name() == name && n.tag_name().namespace() == Some(V3)
}

fn kid<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children().find(|c| is(*c, name))
}

fn kids<'a, 'i: 'a>(n: Node<'a, 'i>, name: &'a str) -> impl Iterator<Item = Node<'a, 'i>> + 'a {
    n.children().filter(move |c| is(*c, name))
}

/// The first element called `name` below `n`, in document order.
fn first<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.descendants().skip(1).find(|c| is(*c, name))
}

fn has_template(n: Node, root: &str) -> bool {
    kids(n, "templateId").any(|t| t.attribute("root") == Some(root))
}

/// "Not given", "no known allergies" and the like.
fn negated(n: Node) -> bool {
    n.attribute("negationInd") == Some("true")
}

/// The coded product of a medicine or vaccine entry.
fn product<'a, 'i>(sa: Node<'a, 'i>) -> Option<Node<'a, 'i>> {
    [
        "consumable",
        "manufacturedProduct",
        "manufacturedMaterial",
        "code",
    ]
    .iter()
    .try_fold(sa, |n, name| kid(n, name))
}

/// When something happened: `effectiveTime` itself, or the start of its range.
fn when(n: Node) -> Option<String> {
    let t = kid(n, "effectiveTime")?;
    t.attribute("value")
        .or_else(|| kid(t, "low")?.attribute("value"))
        .and_then(date)
}

/// An HL7 date (`20150301`, `201503011230+0000`, `2015`) as a FHIR date.
fn date(ts: &str) -> Option<String> {
    let d: String = ts
        .chars()
        .take_while(char::is_ascii_digit)
        .take(8)
        .collect();
    match d.len() {
        8 => Some(format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..])),
        6 => Some(format!("{}-{}", &d[..4], &d[4..])),
        4 => Some(d),
        _ => None,
    }
}

fn number(s: &str) -> Option<Value> {
    let x: f64 = s.trim().parse().ok()?;
    serde_json::Number::from_f64(x).map(Value::Number)
}

fn set(v: &mut Value, key: &str, x: Option<String>) {
    if let Some(x) = x {
        v[key] = json!(x);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    // A small synthetic C-CDA: one entry per section, no real person.
    pub(crate) const SAMPLE: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<ClinicalDocument xmlns="urn:hl7-org:v3" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
 <component><structuredBody>
  <component><section>
   <templateId root="2.16.840.1.113883.10.20.22.2.5.1"/>
   <text><table><tr><td ID="prob1">Asthma, mild</td></tr></table></text>
   <entry><act><statusCode code="active"/><effectiveTime><low value="20150301"/></effectiveTime>
    <entryRelationship><observation>
     <effectiveTime><low value="20150301"/></effectiveTime>
     <value xsi:type="CD" code="195967001" codeSystem="2.16.840.1.113883.6.96"><originalText><reference value="#prob1"/></originalText></value>
    </observation></entryRelationship></act></entry>
  </section></component>
  <component><section>
   <code code="48765-2"/>
   <entry><act><entryRelationship><observation>
     <effectiveTime><low value="2009"/></effectiveTime>
     <participant><participantRole><playingEntity><code code="7980" codeSystem="2.16.840.1.113883.6.88" displayName="Penicillin G"/></playingEntity></participantRole></participant>
     <entryRelationship><observation><templateId root="2.16.840.1.113883.10.20.22.4.9"/>
      <value xsi:type="CD" code="271807003" codeSystem="2.16.840.1.113883.6.96" displayName="Rash"/></observation></entryRelationship>
   </observation></entryRelationship></act></entry>
  </section></component>
  <component><section>
   <templateId root="2.16.840.1.113883.10.20.22.2.1.1"/>
   <entry><substanceAdministration><statusCode code="active"/>
    <effectiveTime xsi:type="IVL_TS"><low value="20210315"/></effectiveTime>
    <text>One tablet   twice a day</text>
    <consumable><manufacturedProduct><manufacturedMaterial><code code="861004" codeSystem="2.16.840.1.113883.6.88" displayName="Metformin 500 mg"/></manufacturedMaterial></manufacturedProduct></consumable>
   </substanceAdministration></entry>
  </section></component>
  <component><section>
   <templateId root="2.16.840.1.113883.10.20.22.2.2.1"/>
   <entry><substanceAdministration negationInd="false"><effectiveTime value="20251004"/>
    <consumable><manufacturedProduct><manufacturedMaterial><code code="140" codeSystem="2.16.840.1.113883.12.292" displayName="Influenza, seasonal"/></manufacturedMaterial></manufacturedProduct></consumable>
   </substanceAdministration></entry>
   <entry><substanceAdministration negationInd="true"><effectiveTime value="20240101"/>
    <consumable><manufacturedProduct><manufacturedMaterial><code code="141" displayName="Refused vaccine"/></manufacturedMaterial></manufacturedProduct></consumable>
   </substanceAdministration></entry>
  </section></component>
  <component><section>
   <templateId root="2.16.840.1.113883.10.20.22.2.3.1"/>
   <entry><organizer><effectiveTime value="20251202"/>
    <component><observation><code code="4548-4" codeSystem="2.16.840.1.113883.6.1" displayName="HbA1c"/>
     <value xsi:type="PQ" value="47" unit="mmol/mol"/>
     <referenceRange><observationRange><value xsi:type="IVL_PQ"><low value="20"/><high value="41"/></value></observationRange></referenceRange>
    </observation></component>
    <component><observation><code displayName="Urine glucose"/><effectiveTime value="20251203"/><value xsi:type="ST">negative</value></observation></component>
   </organizer></entry>
  </section></component>
  <component><section>
   <templateId root="2.16.840.1.113883.10.20.22.2.7.1"/>
   <entry><procedure><code code="80146002" codeSystem="2.16.840.1.113883.6.96" displayName="Appendectomy"/><effectiveTime value="20190506"/></procedure></entry>
  </section></component>
  <component><section><code code="29762-2"/><entry><observation><code displayName="Social history"/></observation></entry></section></component>
 </structuredBody></component>
</ClinicalDocument>"##;

    #[test]
    fn reads_each_common_section() {
        assert!(is_xml(SAMPLE.as_bytes()) && !is_xml(b"  {\"a\": 1}"));
        let rs = resources(SAMPLE.as_bytes()).unwrap();
        let types: Vec<&str> = rs
            .iter()
            .map(|r| r["resourceType"].as_str().unwrap())
            .collect();
        assert_eq!(
            types,
            [
                "Condition",
                "AllergyIntolerance",
                "MedicationStatement",
                "Immunization",
                "Observation",
                "Observation",
                "Procedure"
            ]
        );
        let [cond, allergy, med, vax, a1c, urine, proc] = &rs[..] else {
            unreachable!()
        };
        assert_eq!(cond["code"]["text"], "Asthma, mild");
        assert_eq!(
            cond["code"]["coding"][0]["system"],
            "http://snomed.info/sct"
        );
        assert_eq!(cond["clinicalStatus"]["coding"][0]["code"], "active");
        assert_eq!(cond["onsetDateTime"], "2015-03-01");
        assert_eq!(allergy["code"]["text"], "Penicillin G");
        assert_eq!(allergy["reaction"][0]["manifestation"][0]["text"], "Rash");
        assert_eq!(allergy["onsetDateTime"], "2009");
        assert_eq!(med["medicationCodeableConcept"]["text"], "Metformin 500 mg");
        assert_eq!(med["effectivePeriod"]["start"], "2021-03-15");
        assert_eq!(med["dosage"][0]["text"], "One tablet twice a day");
        assert_eq!(
            vax["vaccineCode"]["coding"][0]["system"],
            "http://hl7.org/fhir/sid/cvx"
        );
        assert_eq!(vax["occurrenceDateTime"], "2025-10-04");
        assert_eq!(
            a1c["valueQuantity"],
            json!({"value": 47.0, "unit": "mmol/mol"})
        );
        assert_eq!(a1c["referenceRange"][0]["high"]["value"], 41.0);
        assert_eq!(a1c["effectiveDateTime"], "2025-12-02");
        assert_eq!(urine["valueString"], "negative");
        assert_eq!(urine["effectiveDateTime"], "2025-12-03");
        assert_eq!(proc["performedDateTime"], "2019-05-06");
    }

    #[test]
    fn refuses_other_xml_and_outside_entities() {
        assert!(resources(b"<html/>").unwrap_err().contains("not a C-CDA"));
        let xxe = br#"<?xml version="1.0"?><!DOCTYPE x [<!ENTITY e SYSTEM "file:///etc/hostname">]><ClinicalDocument xmlns="urn:hl7-org:v3">&e;</ClinicalDocument>"#;
        assert!(resources(xxe).is_err());
        assert_eq!(date("201503011230+0000").as_deref(), Some("2015-03-01"));
        assert_eq!(date("2015-03").as_deref(), Some("2015"));
    }
}
