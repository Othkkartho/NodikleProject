//! Nested tag extraction and normalization (`#parent/child`).

use std::collections::BTreeSet;
use std::sync::OnceLock;

use nodikle_model::PropertyValue;
use regex::Regex;

fn inline_tag_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?:^|[\s(\[])#([0-9A-Za-z_\-\u{AC00}-\u{D7A3}\u{3131}-\u{318E}]+(?:/[0-9A-Za-z_\-\u{AC00}-\u{D7A3}\u{3131}-\u{318E}]+)*)")
            .expect("tag regex must compile")
    })
}

/// Normalizes a tag string by stripping leading `#`, trimming, and lowercasing ASCII.
pub fn normalize_tag(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_start_matches('#').trim_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    // Reject purely numeric tokens such as `#123` to avoid false positives in prose.
    if trimmed.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(trimmed.to_lowercase())
}

/// Extracts and merges tags from YAML frontmatter and non-code body segments.
pub fn extract_tags(
    frontmatter_tags: Option<&PropertyValue>,
    non_code_segments: &[&str],
) -> Vec<String> {
    let mut set = BTreeSet::new();

    if let Some(prop) = frontmatter_tags {
        match prop {
            PropertyValue::List(items) => {
                for item in items {
                    if let Some(norm) = normalize_tag(item) {
                        set.insert(norm);
                    }
                }
            }
            PropertyValue::Text(single) => {
                for part in single.split([',', ' ']) {
                    if let Some(norm) = normalize_tag(part) {
                        set.insert(norm);
                    }
                }
            }
            _ => {}
        }
    }

    let re = inline_tag_regex();
    for segment in non_code_segments {
        for caps in re.captures_iter(segment) {
            if let Some(m) = caps.get(1) {
                if let Some(norm) = normalize_tag(m.as_str()) {
                    set.insert(norm);
                }
            }
        }
    }

    set.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_frontmatter_and_inline_nested_tags() {
        let fm = PropertyValue::List(vec!["가계부/식비".into(), "#Rust".into()]);
        let tags = extract_tags(Some(&fm), &["메모 #여행/일본 #123 #가계부/식비"]);
        assert_eq!(tags, vec!["rust", "가계부/식비", "여행/일본"]);
    }
}
