//! Bounded Crossref bibliographic discovery, not a scientific truth oracle.
//!
//! These records identify possible reading material. A title and a DOI do not
//! establish a theory's validity, applicability, or support for an answer.
//! Official protocol references checked 2026-11-12:
//! https://www.crossref.org/documentation/retrieve-metadata/rest-api
//! https://api.crossref.org/swagger-ui/index.html

use crate::contract::hash;
use reqwest::{Url, blocking::Client};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    io::Read,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const ENDPOINT: &str = "https://api.crossref.org/works";
pub const FORMAT: &str = "percent-grading-crossref-candidates-v1";
const MAX_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchConfig {
    pub query: String,
    pub rows: u16,
    pub timeout_seconds: u64,
}

impl SearchConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.query.trim().is_empty()
            || self.query.len() > 512
            || self.query.chars().any(char::is_control)
        {
            return Err(
                "Discovery query must contain 1..512 UTF-8 bytes without control characters".into(),
            );
        }
        if !(1..=20).contains(&self.rows) {
            return Err("Discovery rows must be 1..20".into());
        }
        if !(1..=120).contains(&self.timeout_seconds) {
            return Err("Discovery timeout_seconds must be 1..120".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub doi: String,
    pub title: String,
    pub url: String,
    pub publisher: String,
    pub publication_year: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchBundle {
    pub format: String,
    pub query: String,
    pub requested_rows: u16,
    pub endpoint: String,
    /// None identifies a parsed/imported snapshot, not a claimed live retrieval.
    pub retrieved_at_unix: Option<u64>,
    pub items: Vec<Candidate>,
    pub skipped_items: usize,
    pub bundle_hash: String,
}

fn payload_hash(bundle: &SearchBundle) -> Result<String, String> {
    let payload = (
        &bundle.format,
        &bundle.query,
        bundle.requested_rows,
        &bundle.endpoint,
        bundle.retrieved_at_unix,
        &bundle.items,
        bundle.skipped_items,
    );
    Ok(hash(
        &serde_json::to_vec(&payload).map_err(|e| e.to_string())?,
    ))
}

fn canonical_doi(raw: &str) -> Result<(String, String), String> {
    if raw.is_empty()
        || raw.len() > 512
        || !raw.is_ascii()
        || raw
            .bytes()
            .any(|c| c.is_ascii_whitespace() || c.is_ascii_control())
    {
        return Err("Unsupported or malformed DOI".into());
    }
    let doi = raw.to_ascii_lowercase();
    let (prefix, suffix) = doi.split_once('/').ok_or("DOI is missing its suffix")?;
    let registrant = prefix
        .strip_prefix("10.")
        .ok_or("DOI prefix must begin with 10.")?;
    if !(4..=9).contains(&registrant.len())
        || !registrant.bytes().all(|c| c.is_ascii_digit())
        || suffix.is_empty()
        || suffix
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("Unsupported or malformed DOI".into());
    }
    let mut url = Url::parse("https://doi.org/").expect("fixed DOI origin");
    url.path_segments_mut()
        .map_err(|_| "DOI URL has no path")?
        .clear()
        .extend(doi.split('/'));
    Ok((doi, url.into()))
}

fn valid_text(text: &str, max: usize) -> bool {
    !text.trim().is_empty() && text.len() <= max && !text.chars().any(char::is_control)
}

impl SearchBundle {
    /// Detect edits and malformed metadata. This is not a signed Crossref attestation.
    pub fn verify(&self) -> Result<(), String> {
        SearchConfig {
            query: self.query.clone(),
            rows: self.requested_rows,
            timeout_seconds: 1,
        }
        .validate()?;
        if self.format != FORMAT || self.endpoint != ENDPOINT {
            return Err("Unsupported discovery snapshot format or endpoint".into());
        }
        if self
            .items
            .len()
            .checked_add(self.skipped_items)
            .is_none_or(|n| n > usize::from(self.requested_rows))
        {
            return Err("Discovery snapshot exceeds its requested row limit".into());
        }
        let mut seen = BTreeSet::new();
        for item in &self.items {
            let (doi, url) = canonical_doi(&item.doi)?;
            if doi != item.doi
                || url != item.url
                || !seen.insert(doi)
                || !valid_text(&item.title, 2048)
                || !valid_text(&item.publisher, 512)
                || item
                    .publication_year
                    .is_some_and(|year| !(1..=9999).contains(&year))
            {
                return Err(
                    "Discovery snapshot contains invalid or duplicate candidate metadata".into(),
                );
            }
        }
        if self.bundle_hash != payload_hash(self)? {
            return Err("Discovery snapshot hash mismatch".into());
        }
        Ok(())
    }
}

fn candidate(raw: &Value) -> Option<Candidate> {
    let (doi, url) = canonical_doi(raw.get("DOI")?.as_str()?).ok()?;
    let title = raw
        .get("title")?
        .as_array()?
        .first()?
        .as_str()?
        .trim()
        .to_owned();
    let publisher = raw
        .get("publisher")
        .and_then(Value::as_str)
        .unwrap_or("Unknown publisher")
        .trim()
        .to_owned();
    if !valid_text(&title, 2048) || !valid_text(&publisher, 512) {
        return None;
    }
    let publication_year = raw
        .pointer("/published/date-parts/0/0")
        .and_then(Value::as_u64)
        .filter(|year| (1..=9999).contains(year))
        .map(|year| year as u32);
    // The upstream URL is deliberately not copied or fetched. Generate a safe
    // HTTPS resolver link from the identifier; abstracts and other prose are omitted.
    Some(Candidate {
        doi,
        title,
        url,
        publisher,
        publication_year,
    })
}

/// Parse a captured API envelope without claiming that this function used the network.
pub fn parse_response(config: &SearchConfig, raw: &Value) -> Result<SearchBundle, String> {
    config.validate()?;
    if raw.get("status").and_then(Value::as_str) != Some("ok")
        || raw.get("message-type").and_then(Value::as_str) != Some("work-list")
    {
        return Err("Crossref returned an unexpected metadata envelope".into());
    }
    let rows = raw
        .pointer("/message/items")
        .and_then(Value::as_array)
        .ok_or("Crossref response has no item list")?;
    if rows.len() > usize::from(config.rows) {
        return Err("Crossref returned more records than requested".into());
    }
    let mut seen = BTreeSet::new();
    let mut items = Vec::new();
    let mut skipped_items = 0;
    for row in rows {
        match candidate(row) {
            Some(item) if seen.insert(item.doi.clone()) => items.push(item),
            _ => skipped_items += 1,
        }
    }
    let mut bundle = SearchBundle {
        format: FORMAT.into(),
        query: config.query.clone(),
        requested_rows: config.rows,
        endpoint: ENDPOINT.into(),
        retrieved_at_unix: None,
        items,
        skipped_items,
        bundle_hash: String::new(),
    };
    bundle.bundle_hash = payload_hash(&bundle)?;
    bundle.verify()?;
    Ok(bundle)
}

fn request_url(config: &SearchConfig) -> Result<Url, String> {
    config.validate()?;
    let mut url = Url::parse(ENDPOINT).expect("fixed Crossref endpoint");
    url.query_pairs_mut()
        .append_pair("query.bibliographic", &config.query)
        .append_pair("rows", &config.rows.to_string())
        .append_pair("select", "DOI,title,publisher,published");
    Ok(url)
}

fn require_success(status: u16) -> Result<(), String> {
    if status != 200 {
        return Err(format!(
            "Crossref HTTP {status}; no discovery snapshot accepted; retry later if the service is rate-limited"
        ));
    }
    Ok(())
}

/// Perform one public metadata request. No redirects, proxies, credentials,
/// tools, model calls, arbitrary origins, or automatic retries are used.
pub fn search(config: &SearchConfig) -> Result<SearchBundle, String> {
    let url = request_url(config)?;
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = Client::builder()
        .timeout(Duration::from_secs(config.timeout_seconds))
        .connect_timeout(Duration::from_secs(config.timeout_seconds.min(10)))
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .https_only(true)
        .no_proxy()
        .user_agent("PercentGrading/0.3 (+https://github.com/kimyubin33/percent-grading)")
        .build()
        .map_err(|_| "Could not initialize the discovery HTTPS client")?;
    let response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .map_err(|_| "Crossref HTTPS request failed; no snapshot accepted")?;
    require_success(response.status().as_u16())?;
    if response
        .content_length()
        .is_some_and(|n| n > MAX_RESPONSE_BYTES)
    {
        return Err("Crossref response exceeded the byte limit".into());
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Crossref response could not be read")?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err("Crossref response exceeded the byte limit".into());
    }
    let raw: Value =
        serde_json::from_slice(&bytes).map_err(|_| "Crossref returned invalid JSON")?;
    let mut bundle = parse_response(config, &raw)?;
    bundle.retrieved_at_unix = Some(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "System clock precedes the Unix epoch")?
            .as_secs(),
    );
    bundle.bundle_hash = payload_hash(&bundle)?;
    bundle.verify()?;
    Ok(bundle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn config() -> SearchConfig {
        SearchConfig {
            query: "apple optical reflectance".into(),
            rows: 5,
            timeout_seconds: 10,
        }
    }
    fn record(doi: &str, title: &str) -> Value {
        json!({"DOI":doi,"title":[title],"publisher":"Example publisher","published":{"date-parts":[[2024,2,1]]}})
    }
    fn envelope(items: Vec<Value>) -> Value {
        json!({"status":"ok","message-type":"work-list","message":{"items":items}})
    }

    #[test]
    fn imported_snapshot_is_deterministic_and_not_marked_live() {
        let raw = envelope(vec![record("10.1234/Example", "Example title")]);
        let bundle = parse_response(&config(), &raw).unwrap();
        assert_eq!(bundle, parse_response(&config(), &raw).unwrap());
        assert_eq!(bundle.retrieved_at_unix, None);
        assert_eq!(bundle.items[0].doi, "10.1234/example");
        assert_eq!(bundle.items[0].publication_year, Some(2024));
        bundle.verify().unwrap();
    }

    #[test]
    fn malformed_envelopes_and_extra_rows_fail_closed() {
        for raw in [
            json!({}),
            json!({"status":"error","message-type":"work-list","message":{"items":[]}}),
            json!({"status":"ok","message-type":"work","message":{"items":[]}}),
            json!({"status":"ok","message-type":"work-list","message":{"items":{}}}),
        ] {
            assert!(parse_response(&config(), &raw).is_err());
        }
        let mut cfg = config();
        cfg.rows = 1;
        assert!(
            parse_response(
                &cfg,
                &envelope(vec![record("10.1234/a", "A"), record("10.1234/b", "B")])
            )
            .is_err()
        );
    }

    #[test]
    fn empty_invalid_and_duplicate_records_are_counted_as_skipped() {
        let raw = envelope(vec![
            record("10.1234/a", "A"),
            record("10.1234/A", "Duplicate"),
            record("10.1234/b", " "),
            record("not-a-doi", "Invalid"),
        ]);
        let bundle = parse_response(&config(), &raw).unwrap();
        assert_eq!(bundle.items.len(), 1);
        assert_eq!(bundle.skipped_items, 3);
    }

    #[test]
    fn untrusted_urls_are_ignored_and_doi_links_stay_on_fixed_origin() {
        let mut item = record("10.1234/a?redirect=evil.example#fragment", "Title");
        item["URL"] = json!("http://user:secret@127.0.0.1/private");
        let bundle = parse_response(&config(), &envelope(vec![item])).unwrap();
        let url = Url::parse(&bundle.items[0].url).unwrap();
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("doi.org"));
        assert_eq!(url.query(), None);
        assert_eq!(url.fragment(), None);
        assert!(url.username().is_empty());
        assert!(!bundle.items[0].url.contains("secret"));
        assert!(canonical_doi("10.1234/../private").is_err());
    }

    #[test]
    fn query_is_encoded_as_one_parameter_not_an_endpoint_or_option() {
        let mut cfg = config();
        cfg.query = "apple&rows=999#https://localhost/".into();
        let url = request_url(&cfg).unwrap();
        assert_eq!(url.host_str(), Some("api.crossref.org"));
        let pairs: Vec<_> = url.query_pairs().collect();
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0].1, cfg.query);
        assert_eq!(pairs[1].1, "5");
        assert_eq!(url.fragment(), None);
    }

    #[test]
    fn invalid_limits_and_non_success_statuses_are_rejected() {
        for rows in [0, 21] {
            let mut cfg = config();
            cfg.rows = rows;
            assert!(request_url(&cfg).is_err());
        }
        for query in [
            "".to_owned(),
            " ".to_owned(),
            "x".repeat(513),
            "x\ny".to_owned(),
        ] {
            let mut cfg = config();
            cfg.query = query;
            assert!(request_url(&cfg).is_err());
        }
        for timeout in [0, 121] {
            let mut cfg = config();
            cfg.timeout_seconds = timeout;
            assert!(request_url(&cfg).is_err());
        }
        for status in [201, 301, 400, 403, 429, 500] {
            assert!(require_success(status).is_err());
        }
        require_success(200).unwrap();
    }

    #[test]
    fn hashes_and_structural_verification_detect_modified_snapshots() {
        let bundle = parse_response(&config(), &envelope(vec![record("10.1234/a", "A")])).unwrap();
        let mut changed = bundle.clone();
        changed.items[0].title = "B".into();
        assert!(changed.verify().is_err());
        let mut changed = bundle.clone();
        changed.endpoint = "https://evil.example".into();
        changed.bundle_hash = payload_hash(&changed).unwrap();
        assert!(changed.verify().is_err());
        let mut changed = bundle;
        changed.items[0].url = "http://doi.org/10.1234/a".into();
        changed.bundle_hash = payload_hash(&changed).unwrap();
        assert!(changed.verify().is_err());
    }

    #[test]
    fn metadata_does_not_import_abstracts_or_turn_titles_into_support() {
        let mut item = record("10.1234/a", "Claim in title");
        item["abstract"] = json!("<p>Untrusted copyrighted abstract</p>");
        let bundle = parse_response(&config(), &envelope(vec![item])).unwrap();
        let text = serde_json::to_string(&bundle).unwrap();
        assert!(!text.contains("abstract"));
        assert!(!text.contains("Untrusted"));
        let mut value = serde_json::to_value(bundle).unwrap();
        value["truth_probability"] = json!(1.0);
        assert!(serde_json::from_value::<SearchBundle>(value).is_err());
    }
}
