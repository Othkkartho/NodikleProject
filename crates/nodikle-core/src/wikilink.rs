//! Wikilink (`[[target|alias#anchor]]`) and embed (`![[asset]]`) extraction.

use std::sync::OnceLock;

use nodikle_model::Wikilink;
use regex::Regex;

fn wikilink_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(!?)\[\[([^\]\r\n]+?)\]\]").expect("wikilink regex must compile")
    })
}

/// Extracts wikilinks and embeds from non-code text segments.
pub fn extract_wikilinks(segments: &[&str]) -> Vec<Wikilink> {
    let mut results = Vec::new();
    let re = wikilink_regex();

    for segment in segments {
        for caps in re.captures_iter(segment) {
            let is_embed = caps.get(1).map(|m| m.as_str() == "!").unwrap_or(false);
            let inner = caps.get(2).map(|m| m.as_str()).unwrap_or("").trim();
            if inner.is_empty() {
                continue;
            }

            let (target_and_anchor, alias) = match inner.split_once('|') {
                Some((left, right)) => {
                    let alias_trimmed = right.trim();
                    let alias = if alias_trimmed.is_empty() {
                        None
                    } else {
                        Some(alias_trimmed.to_string())
                    };
                    (left.trim(), alias)
                }
                None => (inner, None),
            };

            let (raw_target, anchor) = match target_and_anchor.split_once('#') {
                Some((t, a)) => {
                    let anchor_trimmed = a.trim();
                    let anchor = if anchor_trimmed.is_empty() {
                        None
                    } else {
                        Some(anchor_trimmed.to_string())
                    };
                    (t.trim().to_string(), anchor)
                }
                None => (target_and_anchor.to_string(), None),
            };

            if raw_target.is_empty() {
                continue;
            }

            results.push(Wikilink {
                raw_target,
                anchor,
                alias,
                is_embed,
            });
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_aliases_anchors_and_embeds() {
        let links = extract_wikilinks(&[
            "참고: [[2026-12-kyoto#예산 요약|교토 예산]] 및 ![[attachments/receipt.jpg]]",
        ]);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].raw_target, "2026-12-kyoto");
        assert_eq!(links[0].anchor.as_deref(), Some("예산 요약"));
        assert_eq!(links[0].alias.as_deref(), Some("교토 예산"));
        assert!(!links[0].is_embed);

        assert_eq!(links[1].raw_target, "attachments/receipt.jpg");
        assert!(links[1].is_embed);
    }
}
