//! SMART Health Links (STU1): a link, often shown as a QR code, that points
//! at encrypted health records on a server. The key is in the link itself,
//! so the server never sees the records in the clear. This module does the
//! parts that need no network: reading the link, decrypting what the server
//! returns, and unwrapping SMART Health Cards. The app does the fetching.
//!
//! <https://hl7.org/fhir/uv/smart-health-cards-and-links/STU1/>

use std::io::Read;

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD_INDIFFERENT as B64;
use serde::Serialize;
use serde_json::{Value, json};

use crate::import::{MAX_BYTES, entries};

/// What a link says, before anything is fetched.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub url: String,
    #[serde(skip)]
    pub key: [u8; 32],
    /// Seconds since 1970 after which the link stops working.
    pub exp: Option<i64>,
    pub label: Option<String>,
    /// The server wants a passcode, given out separately.
    pub needs_passcode: bool,
    /// The url is the one encrypted file, fetched directly (flag `U`).
    pub direct: bool,
}

/// One file named in a manifest: inside it, or at a location to fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRef {
    pub content_type: String,
    pub embedded: Option<String>,
    pub location: Option<String>,
}

/// Read a `shlink:/...` link, alone or after a viewer address and `#`.
pub fn parse(text: &str) -> Result<Link, String> {
    let not_a_link = || "This is not a SMART Health Link.".to_string();
    let at = text.find("shlink:/").ok_or_else(not_a_link)?;
    let encoded = text[at + "shlink:/".len()..].trim();
    let payload: Value = B64
        .decode(encoded)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or_else(not_a_link)?;
    let url = payload["url"].as_str().ok_or_else(not_a_link)?;
    if !url.starts_with("https://") {
        return Err("This link does not use a secure (https) address, so it is not opened.".into());
    }
    let key: [u8; 32] = payload["key"]
        .as_str()
        .and_then(|k| B64.decode(k).ok())
        .and_then(|k| k.try_into().ok())
        .ok_or_else(not_a_link)?;
    let flag = payload["flag"].as_str().unwrap_or("");
    Ok(Link {
        url: url.to_string(),
        key,
        exp: payload["exp"].as_i64(),
        label: payload["label"]
            .as_str()
            .map(|l| l.chars().take(80).collect()),
        needs_passcode: flag.contains('P'),
        direct: flag.contains('U'),
    })
}

/// The body to POST to a manifest url.
pub fn manifest_request(recipient: &str, passcode: Option<&str>) -> Value {
    let mut body = json!({ "recipient": recipient });
    if let Some(p) = passcode {
        body["passcode"] = json!(p);
    }
    body
}

/// The files a manifest lists. Locations must be https.
pub fn manifest_files(manifest: &Value) -> Result<Vec<FileRef>, String> {
    let files = manifest["files"]
        .as_array()
        .ok_or("The server's answer lists no files.")?;
    Ok(files
        .iter()
        .map(|f| FileRef {
            content_type: f["contentType"].as_str().unwrap_or("").to_string(),
            embedded: f["embedded"].as_str().map(String::from),
            location: f["location"]
                .as_str()
                .filter(|l| l.starts_with("https://"))
                .map(String::from),
        })
        .collect())
}

/// Decrypt one file (a compact JWE: `dir`, `A256GCM`, maybe DEFLATE).
/// Returns the plain bytes and the content type in the header, if any.
pub fn decrypt(jwe: &str, key: &[u8; 32]) -> Result<(Vec<u8>, Option<String>), String> {
    let bad = || "A file from the link could not be decrypted.".to_string();
    let parts: Vec<&str> = jwe.trim().split('.').collect();
    let [head, cek, iv, ct, tag] = parts[..] else {
        return Err(bad());
    };
    let header: Value =
        serde_json::from_slice(&B64.decode(head).map_err(|_| bad())?).map_err(|_| bad())?;
    if header["alg"] != "dir" || header["enc"] != "A256GCM" || !cek.is_empty() {
        return Err(bad());
    }
    let iv = B64.decode(iv).map_err(|_| bad())?;
    if iv.len() != 12 {
        return Err(bad());
    }
    let mut sealed = B64.decode(ct).map_err(|_| bad())?;
    sealed.extend(B64.decode(tag).map_err(|_| bad())?);
    let plain = Aes256Gcm::new(key.into())
        .decrypt(
            iv.as_slice().into(),
            Payload {
                msg: &sealed,
                aad: head.as_bytes(),
            },
        )
        .map_err(|_| bad())?;
    let plain = if header["zip"] == "DEF" {
        inflate(&plain)?
    } else {
        plain
    };
    Ok((plain, header["cty"].as_str().map(String::from)))
}

/// The FHIR resources in a decrypted file: FHIR JSON, or SMART Health Cards.
/// Other kinds of file (such as API access) give none.
pub fn resources(content_type: &str, plain: &[u8]) -> Result<Vec<Value>, String> {
    let json = || -> Result<Value, String> {
        serde_json::from_slice(plain).map_err(|_| "A file from the link is not valid JSON.".into())
    };
    match content_type.split(';').next().unwrap_or("").trim() {
        "application/fhir+json" => entries(&json()?).map_err(|e| e.to_string()),
        "application/smart-health-card" => {
            let cards = json()?;
            let mut out = Vec::new();
            for jws in cards["verifiableCredential"]
                .as_array()
                .into_iter()
                .flatten()
            {
                out.extend(card(jws.as_str().unwrap_or(""))?);
            }
            Ok(out)
        }
        _ => Ok(Vec::new()),
    }
}

/// The FHIR Bundle inside one SMART Health Card (a JWS). The issuer's
/// signature is not checked here: the user reviews every record anyway.
fn card(jws: &str) -> Result<Vec<Value>, String> {
    let bad = || "A health card from the link could not be read.".to_string();
    let mut parts = jws.split('.');
    let (Some(head), Some(body)) = (parts.next(), parts.next()) else {
        return Err(bad());
    };
    let header: Value =
        serde_json::from_slice(&B64.decode(head).map_err(|_| bad())?).map_err(|_| bad())?;
    let mut payload = B64.decode(body).map_err(|_| bad())?;
    if header["zip"] == "DEF" {
        payload = inflate(&payload)?;
    }
    let vc: Value = serde_json::from_slice(&payload).map_err(|_| bad())?;
    entries(&vc["vc"]["credentialSubject"]["fhirBundle"]).map_err(|_| bad())
}

/// Raw DEFLATE, refusing anything that grows past the import limit.
fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    flate2::read::DeflateDecoder::new(data)
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|_| "A file from the link is damaged.".to_string())?;
    if out.len() > MAX_BYTES {
        return Err("A file from the link is larger than 20 MB.".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use std::io::Write;

    const KEY: [u8; 32] = [7; 32];

    fn b64(b: &[u8]) -> String {
        URL_SAFE_NO_PAD.encode(b)
    }

    fn deflate(b: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(b).unwrap();
        e.finish().unwrap()
    }

    fn jwe(plain: &[u8], header: Value) -> String {
        let head = b64(&serde_json::to_vec(&header).unwrap());
        let iv = [1u8; 12];
        let sealed = Aes256Gcm::new((&KEY).into())
            .encrypt(
                (&iv).into(),
                Payload {
                    msg: plain,
                    aad: head.as_bytes(),
                },
            )
            .unwrap();
        let (ct, tag) = sealed.split_at(sealed.len() - 16);
        format!("{head}..{}.{}.{}", b64(&iv), b64(ct), b64(tag))
    }

    fn link(payload: Value) -> String {
        format!(
            "https://viewer.example.org/#shlink:/{}",
            b64(&serde_json::to_vec(&payload).unwrap())
        )
    }

    #[test]
    fn reads_links_and_refuses_odd_ones() {
        let l = parse(&link(
            json!({"url": "https://shl.example.org/m/abc", "key": b64(&KEY),
            "exp": 1_900_000_000, "flag": "LP", "label": "Sample summary"}),
        ))
        .unwrap();
        assert_eq!(l.url, "https://shl.example.org/m/abc");
        assert_eq!(l.key, KEY);
        assert!(l.needs_passcode && !l.direct);
        assert_eq!(l.label.as_deref(), Some("Sample summary"));
        assert!(parse("hello").is_err());
        let plain_http = link(json!({"url": "http://shl.example.org/m", "key": b64(&KEY)}));
        assert!(parse(&plain_http).unwrap_err().contains("https"));
        assert!(
            parse(&link(
                json!({"url": "https://a.example.org", "key": "short"})
            ))
            .is_err()
        );
        let body = manifest_request("Arogya Swasthya", Some("1234"));
        assert_eq!(
            body,
            json!({"recipient": "Arogya Swasthya", "passcode": "1234"})
        );
        let files = manifest_files(&json!({"files": [
            {"contentType": "application/fhir+json", "location": "http://plain.example.org/f"},
            {"contentType": "application/fhir+json", "embedded": "x.y"}
        ]}))
        .unwrap();
        assert_eq!(files[0].location, None);
        assert_eq!(files[1].embedded.as_deref(), Some("x.y"));
    }

    #[test]
    fn decrypts_files_and_unwraps_health_cards() {
        let bundle = json!({"resourceType": "Bundle", "type": "collection", "entry": [
            {"resource": {"resourceType": "Immunization", "status": "completed", "vaccineCode": {"text": "Sample vaccine"}}}
        ]});
        let plain = serde_json::to_vec(&bundle).unwrap();
        let file = jwe(
            &deflate(&plain),
            json!({"alg": "dir", "enc": "A256GCM", "zip": "DEF", "cty": "application/fhir+json"}),
        );
        let (out, cty) = decrypt(&file, &KEY).unwrap();
        assert_eq!(out, plain);
        assert_eq!(cty.as_deref(), Some("application/fhir+json"));
        assert_eq!(resources("application/fhir+json", &out).unwrap().len(), 1);
        assert!(decrypt(&file, &[8; 32]).is_err());
        let mut tampered = file.clone();
        tampered.pop();
        assert!(decrypt(&tampered, &KEY).is_err());

        // A health card: the JWS payload is deflated; its signature is not checked.
        let vc = json!({"vc": {"credentialSubject": {"fhirBundle": bundle}}});
        let jws = format!(
            "{}.{}.c2ln",
            b64(br#"{"alg":"ES256","zip":"DEF"}"#),
            b64(&deflate(&serde_json::to_vec(&vc).unwrap()))
        );
        let cards = serde_json::to_vec(&json!({"verifiableCredential": [jws]})).unwrap();
        let rs = resources("application/smart-health-card", &cards).unwrap();
        assert_eq!(rs[0]["vaccineCode"]["text"], "Sample vaccine");
        assert!(
            resources("application/smart-api-access", b"{}")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn refuses_a_deflate_bomb() {
        let huge = deflate(&vec![0u8; MAX_BYTES + 10]);
        assert!(inflate(&huge).unwrap_err().contains("20 MB"));
    }
}
