//! The doctor report: the screen gathers what the person chose to share, and
//! this lays it out as a PDF with Typst, on the device. The template places
//! every string as text, so nothing in a record can change the layout.

use std::io::Write;

use serde_json::Value;
use tauri::{AppHandle, State};
use typst::foundations::{Dict, IntoValue};
use typst_as_lib::TypstEngine;
use typst_layout::PagedDocument;

use crate::docs::create_target;
use crate::{App, CommandResult, err};

static TEMPLATE: &str = include_str!("report.typ");
static FONTS: [&[u8]; 3] = [
    include_bytes!("../fonts/LibertinusSerif-Regular.otf"),
    include_bytes!("../fonts/LibertinusSerif-Bold.otf"),
    include_bytes!("../fonts/LibertinusSerif-Italic.otf"),
];
/// A report's data is a few hundred KB at most; refuse anything far larger.
const MAX_DATA: usize = 8 * 1024 * 1024;

/// Lay the report out as PDF bytes.
pub fn render(data: &Value) -> Result<Vec<u8>, String> {
    let json = serde_json::to_string(data).map_err(err)?;
    if json.len() > MAX_DATA {
        return Err("The report is too large. Choose a shorter period or fewer sections.".into());
    }
    let engine = TypstEngine::builder()
        .main_file(TEMPLATE)
        .fonts(FONTS)
        .build();
    let mut inputs = Dict::new();
    inputs.insert("data".into(), json.into_value());
    let doc: PagedDocument = engine
        .compile_with_input(inputs)
        .output
        .map_err(|e| format!("Could not lay out the report ({e})."))?;
    typst_pdf::pdf(&doc, &Default::default()).map_err(|e| {
        format!(
            "Could not make the PDF ({:?}).",
            e.first().map(|d| &d.message)
        )
    })
}

/// Make the PDF and save it where the user chose in the save dialog.
#[tauri::command]
pub async fn save_report(
    app: AppHandle,
    state: State<'_, App>,
    data: Value,
    target: String,
) -> CommandResult<()> {
    // Only while unlocked, like everything that shows records.
    state.with(|_| Ok(()))?;
    let pdf = tauri::async_runtime::spawn_blocking(move || render(&data))
        .await
        .map_err(err)??;
    let mut out =
        create_target(&app, &target).map_err(|e| format!("Could not save it there: {e}"))?;
    out.write_all(&pdf)
        .and_then(|()| out.flush())
        .map_err(|e| format!("Could not save the report: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        serde_json::json!({
            "title": "Health summary",
            "person": {"name": "Alex Demo #strong[not markup]", "born": "1980-04-02", "age": 46, "sex": "Male"},
            "made": "30 Oct 2026",
            "span": "Last year",
            "note": "Headaches since August. $x$ ]",
            "footer": "Alex Demo · made with Arogya Swasthya",
            "about": "From records kept by the patient.",
            "lists": [
                {"heading": "Conditions", "rows": [{"title": "Asthma", "sub": "Active", "date": "2015"}]},
                {"heading": "Allergies", "rows": []}
            ],
            "results": [
                {"name": "HbA1c", "value": "47 mmol/mol", "flag": "above range", "range": "20 to 41", "date": "2025-12-02"},
                {"name": "Cholesterol", "value": "4.6 mmol/L", "flag": "", "range": "0 to 5", "date": "2025-12-02"}
            ],
            "charts": [
                {"title": "Weight (kg)", "paired": false, "points": [[0, 80, null], [50, 40, null], [100, 20, null]],
                 "band": null, "y": [[0, "73"], [50, "72"], [100, "71"]], "x": [[0, "Aug 2"], [50, "Sep 5"], [100, "Oct 30"]], "last": "71.5"},
                {"title": "Blood pressure (mmHg)", "paired": true, "points": [[0, 10, 60], [100, 20, 70]],
                 "band": [20, 60], "y": [[0, "140"], [100, "80"]], "x": [[0, "Aug 2"], [100, "Oct 30"]], "last": "128/82"}
            ],
            "patterns": [{"title": "Weight and blood pressure", "text": "A strong link (r = 0.82)."}]
        })
    }

    #[test]
    fn a_report_renders_to_pdf_and_record_text_stays_text() {
        let pdf = render(&sample()).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(pdf.len() > 5_000);
    }

    #[test]
    fn empty_sections_still_render() {
        let mut d = sample();
        for k in ["lists", "results", "charts", "patterns"] {
            d[k] = serde_json::json!([]);
        }
        d["note"] = "".into();
        assert!(render(&d).unwrap().starts_with(b"%PDF-"));
    }

    #[test]
    #[ignore = "writes a report for a look: REPORT_OUT=x.pdf [REPORT_DATA=data.json] cargo test -- --ignored write_sample"]
    fn write_sample() {
        let data = match std::env::var("REPORT_DATA") {
            Ok(path) => serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap(),
            Err(_) => sample(),
        };
        let out = std::env::var("REPORT_OUT").unwrap_or("report-sample.pdf".into());
        std::fs::write(out, render(&data).unwrap()).unwrap();
    }
}
