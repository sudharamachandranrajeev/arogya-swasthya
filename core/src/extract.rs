//! Text from documents, so search finds them by their words: a PDF's own
//! text, plain text files, and, with the reading models, text read from
//! photos, scans and scanned PDFs by OCR ([ocrs](https://github.com/robertknight/ocrs),
//! Latin alphabet). Everything runs on this device.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use image::DynamicImage;
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use pdf_extract::{Document, Object};
use rten::Model;

/// Most text kept from one file.
const MAX_TEXT: usize = 1 << 20;
/// Scanned pages read from one PDF.
const MAX_PAGES: usize = 40;
/// Images are shrunk to this many pixels on their long side before OCR.
const OCR_SIDE: u32 = 2400;
/// Smaller images in a PDF are logos and stamps, not pages.
const MIN_PAGE_SIDE: u32 = 400;

/// The reading models, loaded once.
pub struct Reader(OcrEngine);

impl Reader {
    pub fn load(detection: &Path, recognition: &Path) -> Result<Reader, String> {
        let load = |p: &Path| {
            Model::load_file(p).map_err(|e| format!("reading model {}: {e}", p.display()))
        };
        OcrEngine::new(OcrEngineParams {
            detection_model: Some(load(detection)?),
            recognition_model: Some(load(recognition)?),
            ..Default::default()
        })
        .map(Reader)
        .map_err(|e| e.to_string())
    }

    fn read(&self, img: DynamicImage) -> String {
        let img = if img.width().max(img.height()) > OCR_SIDE {
            img.resize(OCR_SIDE, OCR_SIDE, image::imageops::FilterType::Triangle)
        } else {
            img
        };
        let rgb = img.into_rgb8();
        let text = ImageSource::from_bytes(rgb.as_raw(), rgb.dimensions())
            .map_err(|e| e.to_string())
            .and_then(|src| self.0.prepare_input(src).map_err(|e| e.to_string()))
            .and_then(|input| self.0.get_text(&input).map_err(|e| e.to_string()));
        // Lines of one character are mostly specks.
        text.unwrap_or_default()
            .lines()
            .filter(|l| l.trim().chars().count() > 1)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// What reading a file gave.
#[derive(Debug, PartialEq, Eq)]
pub enum Text {
    /// The words found (possibly none).
    Found(String),
    /// Only OCR can read it, and the models are not loaded.
    NeedsOcr,
    /// A kind of file this cannot read (Word, audio, ...).
    Unsupported,
}

/// Read the text of a file of `content_type`.
pub fn text_of(content_type: &str, bytes: &[u8], ocr: Option<&Reader>) -> Text {
    let found = |s: String| Text::Found(trim(s));
    match content_type {
        "application/pdf" => {
            let own = pdf_text(bytes);
            if own.split_whitespace().count() >= 5 {
                return found(own);
            }
            match ocr {
                Some(r) => found(
                    pdf_pages(bytes)
                        .into_iter()
                        .map(|p| r.read(p))
                        .collect::<Vec<_>>()
                        .join("\n\n"),
                ),
                None => Text::NeedsOcr,
            }
        }
        "text/plain" | "text/markdown" => found(String::from_utf8_lossy(bytes).into_owned()),
        t if t.starts_with("image/") && t != "image/heic" => match ocr {
            Some(r) => found(
                image::load_from_memory(bytes)
                    .map(|i| r.read(i))
                    .unwrap_or_default(),
            ),
            None => Text::NeedsOcr,
        },
        _ => Text::Unsupported,
    }
}

/// A PDF's own text. The PDF reader panics on some odd files; that counts
/// as no text.
fn pdf_text(bytes: &[u8]) -> String {
    catch_unwind(AssertUnwindSafe(|| {
        pdf_extract::extract_text_from_mem(bytes)
    }))
    .ok()
    .and_then(Result::ok)
    .unwrap_or_default()
}

/// The page-sized images in a scanned PDF: JPEG, or plain 8-bit grey/RGB.
fn pdf_pages(bytes: &[u8]) -> Vec<DynamicImage> {
    catch_unwind(AssertUnwindSafe(|| {
        let Ok(doc) = Document::load_mem(bytes) else {
            return Vec::new();
        };
        doc.objects
            .values()
            .filter_map(|o| match o {
                Object::Stream(s) => Some(s),
                _ => None,
            })
            .filter(|s| {
                s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image".as_slice())
            })
            .filter_map(|s| {
                let num = |k: &[u8]| {
                    s.dict
                        .get(k)
                        .and_then(Object::as_i64)
                        .ok()
                        .map(|n| n as u32)
                };
                let (w, h) = (num(b"Width")?, num(b"Height")?);
                if w.max(h) < MIN_PAGE_SIDE {
                    return None;
                }
                let filter = s.dict.get(b"Filter").ok().and_then(|f| match f {
                    Object::Name(n) => Some(n.clone()),
                    Object::Array(a) if a.len() == 1 => a[0].as_name().ok().map(<[u8]>::to_vec),
                    _ => None,
                });
                match filter.as_deref() {
                    Some(b"DCTDecode") => image::load_from_memory(&s.content).ok(),
                    Some(b"FlateDecode") if num(b"BitsPerComponent") == Some(8) => {
                        let raw = s.decompressed_content().ok()?;
                        let space = s.dict.get(b"ColorSpace").and_then(Object::as_name).ok()?;
                        match space {
                            b"DeviceGray" => {
                                image::GrayImage::from_raw(w, h, raw).map(DynamicImage::from)
                            }
                            b"DeviceRGB" => {
                                image::RgbImage::from_raw(w, h, raw).map(DynamicImage::from)
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                }
            })
            .take(MAX_PAGES)
            .collect()
    }))
    .unwrap_or_default()
}

/// Tidy blank space and keep at most [`MAX_TEXT`] bytes.
fn trim(s: String) -> String {
    let mut out: String = s
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if out.len() > MAX_TEXT {
        let mut cut = MAX_TEXT;
        while !out.is_char_boundary(cut) {
            cut -= 1;
        }
        out.truncate(cut);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdf_extract::{Dictionary, Stream, dictionary};

    /// A one-page PDF saying `words` in Helvetica (synthetic, made here).
    fn pdf(words: &str) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font = doc.add_object(
            dictionary! {"Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica"},
        );
        let resources = doc.add_object(dictionary! {"Font" => dictionary! {"F1" => font}});
        let body = format!("BT /F1 18 Tf 72 720 Td ({words}) Tj ET");
        let content = doc.add_object(Stream::new(Dictionary::new(), body.into_bytes()));
        let page = doc.add_object(
            dictionary! {"Type" => "Page", "Parent" => pages_id, "Contents" => content},
        );
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1,
                "Resources" => resources, "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            }),
        );
        let catalog = doc.add_object(dictionary! {"Type" => "Catalog", "Pages" => pages_id});
        doc.trailer.set("Root", catalog);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    #[test]
    fn reads_pdf_text_plain_text_and_knows_what_needs_ocr() {
        let t = text_of(
            "application/pdf",
            &pdf("Ferritin 45 ug per litre, sample report"),
            None,
        );
        assert!(
            matches!(&t, Text::Found(s) if s.contains("Ferritin 45")),
            "{t:?}"
        );
        assert_eq!(
            text_of("text/plain", b"  a   letter \n\n from  clinic ", None),
            Text::Found("a letter\nfrom clinic".into())
        );
        assert_eq!(text_of("image/png", b"not read", None), Text::NeedsOcr);
        assert_eq!(text_of("application/pdf", &pdf("x"), None), Text::NeedsOcr);
        assert_eq!(
            text_of("application/pdf", b"%PDF-1.4 broken", None),
            Text::NeedsOcr
        );
        assert_eq!(text_of("audio/mpeg", b"", None), Text::Unsupported);
    }

    /// Needs the reading models: OCR_MODELS=/folder/with/the/onnx/files.
    #[test]
    fn reads_a_photo_of_text_with_the_models() {
        let Some(dir) = std::env::var_os("OCR_MODELS") else {
            eprintln!("OCR_MODELS not set: skipping the OCR test");
            return;
        };
        let dir = Path::new(&dir);
        let r = Reader::load(
            &dir.join("text-detection.onnx"),
            &dir.join("text-recognition.onnx"),
        )
        .unwrap();
        let png = include_bytes!("../tests/ocr-sample.png");
        let Text::Found(t) = text_of("image/png", png, Some(&r)) else {
            panic!("no text")
        };
        assert!(t.to_lowercase().contains("ferritin"), "{t}");

        // The same page as a scan inside a PDF (a JPEG image, no text).
        let mut jpeg = Vec::new();
        image::load_from_memory(png)
            .unwrap()
            .into_rgb8()
            .write_to(
                &mut std::io::Cursor::new(&mut jpeg),
                image::ImageFormat::Jpeg,
            )
            .unwrap();
        let mut doc = Document::with_version("1.5");
        doc.add_object(Stream::new(
            dictionary! {"Type" => "XObject", "Subtype" => "Image", "Width" => 900, "Height" => 260,
            "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8, "Filter" => "DCTDecode"},
            jpeg,
        ));
        let mut scanned = Vec::new();
        doc.save_to(&mut scanned).unwrap();
        let Text::Found(t) = text_of("application/pdf", &scanned, Some(&r)) else {
            panic!("no text")
        };
        assert!(t.to_lowercase().contains("ferritin"), "{t}");
    }
}
