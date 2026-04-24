//! arXiv API metadata fetcher.
//!
//! Fetches paper metadata from export.arxiv.org and parses Atom XML
//! using simple string extraction (no heavy XML dependency).

use anyhow::Context;

/// Metadata extracted from an arXiv Atom entry.
#[derive(Debug, Clone, PartialEq)]
pub struct PaperMetadata {
    pub title: String,
    pub authors: String,
    pub summary: String,
    pub published: String,
    pub primary_category: String,
}

/// Fetch metadata for a given arXiv ID (e.g. "2401.12345" or "cs.AI/0202040").
pub fn fetch_arxiv_metadata(arxiv_id: &str) -> anyhow::Result<PaperMetadata> {
    let config = crate::config::Config::load()
        .ok()
        .map(|c| c.arxiv)
        .unwrap_or_default();
    let url = format!("http://export.arxiv.org/api/query?id_list={}", arxiv_id);
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(config.timeout_seconds))
        .build()
        .context("failed to build HTTP client")?;
    let resp = client.get(&url).send().context("failed to send request to arXiv API")?;
    let status = resp.status();
    let text = resp.text().context("failed to read arXiv API response body")?;

    if !status.is_success() {
        anyhow::bail!("arXiv API returned HTTP {}", status);
    }

    parse_arxiv_atom(&text)
}

fn parse_arxiv_atom(xml: &str) -> anyhow::Result<PaperMetadata> {
    // Extract <entry> block
    let entry_start = xml.find("<entry>").context("no <entry> found in arXiv response")?;
    let entry_end = xml[entry_start..]
        .find("</entry>")
        .context("no </entry> found in arXiv response")?;
    let entry = &xml[entry_start..entry_start + entry_end + 8];

    let title = extract_tag_text(entry, "title")
        .context("missing <title> in arXiv entry")?
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let authors = extract_authors(entry)?;

    let summary = extract_tag_text(entry, "summary")
        .unwrap_or_default()
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let published =
        extract_tag_text(entry, "published").context("missing <published> in arXiv entry")?;

    let primary_category = extract_attr_value(entry, "arxiv:primary_category", "term")
        .or_else(|| extract_attr_value(entry, "category", "term"))
        .unwrap_or_default();

    Ok(PaperMetadata {
        title,
        authors,
        summary,
        published,
        primary_category,
    })
}

fn extract_tag_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)?;
    Some(xml[start..start + end].trim().to_string())
}

fn extract_authors(entry: &str) -> anyhow::Result<String> {
    let mut names = Vec::new();
    let mut rest = entry;
    while let Some(start) = rest.find("<author>") {
        let author_end = rest[start..].find("</author>").context("unclosed <author> tag")?;
        let author_block = &rest[start..start + author_end + 9];
        if let Some(name) = extract_tag_text(author_block, "name") {
            names.push(name);
        }
        rest = &rest[start + author_end + 9..];
    }
    if names.is_empty() {
        anyhow::bail!("no <author> entries found");
    }
    Ok(names.join(", "))
}

fn extract_attr_value(xml: &str, tag: &str, attr: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = xml.find(&open)?;
    let tag_end = xml[start..].find('>')?;
    let tag_slice = &xml[start..start + tag_end];
    let attr_prefix = format!("{attr}=\"");
    let attr_start = tag_slice.find(&attr_prefix)? + attr_prefix.len();
    let attr_end = tag_slice[attr_start..].find('"')?;
    Some(tag_slice[attr_start..attr_start + attr_end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOCK_ATOM_OK: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:arxiv="http://arxiv.org/schemas/atom">
  <entry>
    <id>http://arxiv.org/abs/2401.12345</id>
    <title>  Attention Is All   You Need  </title>
    <author><name>Ashish Vaswani</name></author>
    <author><name>Noam Shazeer</name></author>
    <summary>
      We propose a new simple network architecture, the Transformer.
    </summary>
    <published>2017-06-12T00:00:00Z</published>
    <arxiv:primary_category term="cs.CL" />
  </entry>
</feed>"#;

    #[test]
    fn test_parse_arxiv_atom_success() {
        let meta = parse_arxiv_metadata(MOCK_ATOM_OK).unwrap();
        assert_eq!(meta.title, "Attention Is All You Need");
        assert_eq!(meta.authors, "Ashish Vaswani, Noam Shazeer");
        assert!(meta.summary.contains("Transformer"));
        assert_eq!(meta.published, "2017-06-12T00:00:00Z");
        assert_eq!(meta.primary_category, "cs.CL");
    }

    #[test]
    fn test_parse_arxiv_atom_invalid_xml() {
        let err = parse_arxiv_metadata("not xml").unwrap_err();
        assert!(err.to_string().contains("no <entry> found"));
    }

    #[test]
    fn test_parse_arxiv_atom_missing_title() {
        let xml = r#"<feed><entry><published>2020-01-01T00:00:00Z</published></entry></feed>"#;
        let err = parse_arxiv_metadata(xml).unwrap_err();
        assert!(err.to_string().contains("missing <title>"));
    }

    #[test]
    fn test_parse_arxiv_atom_no_authors() {
        let xml = r#"<feed><entry><title>Foo</title><published>2020-01-01T00:00:00Z</published></entry></feed>"#;
        let err = parse_arxiv_metadata(xml).unwrap_err();
        assert!(err.to_string().contains("no <author>"));
    }

    // Wrapper to expose parse logic without HTTP
    fn parse_arxiv_metadata(xml: &str) -> anyhow::Result<PaperMetadata> {
        parse_arxiv_atom(xml)
    }
}
