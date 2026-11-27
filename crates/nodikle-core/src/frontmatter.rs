//! YAML frontmatter splitting, parsing, and lossless round-trip patching.

use std::collections::{BTreeMap, HashMap};

use nodikle_model::{NodikleError, PropertyPatch, PropertyValue, Result};
use regex::Regex;

/// Result of splitting raw Markdown into optional frontmatter YAML and body slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontmatterSplit<'a> {
    pub yaml_raw: Option<&'a str>,
    pub body: &'a str,
}

/// Splits leading YAML frontmatter (`---\n...\n---`) from the Markdown body.
pub fn split_frontmatter(raw: &str) -> FrontmatterSplit<'_> {
    let normalized_start = raw.starts_with("---\n") || raw.starts_with("---\r\n");
    if !normalized_start {
        return FrontmatterSplit {
            yaml_raw: None,
            body: raw,
        };
    }

    let first_newline = match raw.find('\n') {
        Some(idx) => idx + 1,
        None => {
            return FrontmatterSplit {
                yaml_raw: None,
                body: raw,
            };
        }
    };

    let rest = &raw[first_newline..];
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" || trimmed == "..." {
            let yaml_raw = &rest[..offset];
            let body = &rest[offset + line.len()..];
            return FrontmatterSplit {
                yaml_raw: Some(yaml_raw),
                body,
            };
        }
        offset += line.len();
    }

    // Also handle closing delimiter at EOF without trailing newline.
    let tail = &rest[offset..];
    if tail == "---" || tail == "..." {
        return FrontmatterSplit {
            yaml_raw: Some(&rest[..offset]),
            body: "",
        };
    }

    FrontmatterSplit {
        yaml_raw: None,
        body: raw,
    }
}

/// Parses YAML frontmatter into typed properties while preserving key declaration order.
pub fn parse_frontmatter_yaml(
    yaml: &str,
) -> Result<(BTreeMap<String, PropertyValue>, Vec<String>)> {
    if yaml.trim().is_empty() {
        return Ok((BTreeMap::new(), Vec::new()));
    }

    let value: serde_yaml::Value =
        serde_yaml::from_str(yaml).map_err(|e| NodikleError::InvalidFrontmatter(e.to_string()))?;

    let mapping = match value {
        serde_yaml::Value::Mapping(m) => m,
        serde_yaml::Value::Null => return Ok((BTreeMap::new(), Vec::new())),
        _ => {
            return Err(NodikleError::InvalidFrontmatter(
                "frontmatter root must be a YAML mapping".into(),
            ));
        }
    };

    let mut props = BTreeMap::new();
    let mut ordered_keys = Vec::with_capacity(mapping.len());

    for (k, v) in mapping {
        let Some(key_str) = k.as_str() else {
            continue;
        };
        let key = key_str.to_string();
        let prop_val = yaml_value_to_property(&v);
        ordered_keys.push(key.clone());
        props.insert(key, prop_val);
    }

    Ok((props, ordered_keys))
}

fn yaml_value_to_property(value: &serde_yaml::Value) -> PropertyValue {
    match value {
        serde_yaml::Value::Null => PropertyValue::Null,
        serde_yaml::Value::Bool(b) => PropertyValue::Bool(*b),
        serde_yaml::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                PropertyValue::Number(f)
            } else {
                PropertyValue::Text(n.to_string())
            }
        }
        serde_yaml::Value::String(s) => {
            if looks_like_iso_date(s) {
                PropertyValue::Date(s.clone())
            } else {
                PropertyValue::Text(s.clone())
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            let items = seq
                .iter()
                .filter_map(|item| match item {
                    serde_yaml::Value::String(s) => Some(s.clone()),
                    serde_yaml::Value::Number(n) => Some(n.to_string()),
                    serde_yaml::Value::Bool(b) => Some(b.to_string()),
                    _ => None,
                })
                .collect();
            PropertyValue::List(items)
        }
        serde_yaml::Value::Mapping(_) | serde_yaml::Value::Tagged(_) => {
            let serialized = serde_yaml::to_string(value).unwrap_or_default();
            PropertyValue::Text(serialized.trim().to_string())
        }
    }
}

/// Checks if a string starts with `YYYY-MM-DD` (optionally followed by ISO time).
pub fn looks_like_iso_date(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() < 10 {
        return false;
    }
    let is_digits = |slice: &[u8]| slice.iter().all(|b| b.is_ascii_digit());
    is_digits(&bytes[0..4])
        && bytes[4] == b'-'
        && is_digits(&bytes[5..7])
        && bytes[7] == b'-'
        && is_digits(&bytes[8..10])
        && (bytes.len() == 10 || bytes[10] == b'T' || bytes[10] == b' ')
}

/// Formats a key/value pair as YAML lines using the given line ending (`\n` or `\r\n`).
fn format_yaml_entry(key: &str, value: &PropertyValue, eol: &str) -> Vec<String> {
    match value {
        PropertyValue::Null => vec![format!("{key}: null{eol}")],
        PropertyValue::Bool(b) => vec![format!("{key}: {b}{eol}")],
        PropertyValue::Number(n) => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                vec![format!("{key}: {}{eol}", *n as i64)]
            } else {
                vec![format!("{key}: {n}{eol}")]
            }
        }
        PropertyValue::Date(d) => vec![format!("{key}: {d}{eol}")],
        PropertyValue::Text(s) => vec![format!("{key}: {}{eol}", format_yaml_scalar(s))],
        PropertyValue::List(items) => {
            if items.is_empty() {
                vec![format!("{key}: []{eol}")]
            } else {
                let mut lines = Vec::with_capacity(items.len() + 1);
                lines.push(format!("{key}:{eol}"));
                for item in items {
                    lines.push(format!("  - {}{eol}", format_yaml_scalar(item)));
                }
                lines
            }
        }
    }
}

fn format_yaml_scalar(s: &str) -> String {
    let needs_quotes = s.is_empty()
        || s.starts_with([
            '-', '?', ':', '[', ']', '{', '}', '#', '&', '*', '!', '|', '>', '\'', '"', '%', '@',
            '`', ' ',
        ])
        || s.ends_with(' ')
        || s.contains(": ")
        || s.contains(" #")
        || s.contains('\n')
        || matches!(s, "true" | "false" | "null" | "yes" | "no" | "~")
        || s.parse::<f64>().is_ok();

    if needs_quotes {
        let escaped = s
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n");
        format!("\"{escaped}\"")
    } else {
        s.to_string()
    }
}

/// Patches frontmatter properties in place while preserving body bytes, comments, and key order.
pub fn patch_frontmatter(raw: &str, patches: &[PropertyPatch]) -> Result<String> {
    if patches.is_empty() {
        return Ok(raw.to_string());
    }

    let eol = if raw.contains("\r\n") { "\r\n" } else { "\n" };
    let mut patch_map: HashMap<&str, &Option<PropertyValue>> = HashMap::new();
    let mut patch_order: Vec<&str> = Vec::new();
    for p in patches {
        if !patch_map.contains_key(p.key.as_str()) {
            patch_order.push(p.key.as_str());
        }
        patch_map.insert(p.key.as_str(), &p.value);
    }

    let split = split_frontmatter(raw);
    let Some(yaml_raw) = split.yaml_raw else {
        let mut out = String::new();
        out.push_str("---");
        out.push_str(eol);
        for key in patch_order {
            if let Some(Some(val)) = patch_map.get(key) {
                for line in format_yaml_entry(key, val, eol) {
                    out.push_str(&line);
                }
            }
        }
        out.push_str("---");
        out.push_str(eol);
        out.push_str(split.body);
        return Ok(out);
    };

    // Validate existing YAML before line-level round-trip patching.
    let _ = parse_frontmatter_yaml(yaml_raw)?;

    let top_key_re = Regex::new(r"^([A-Za-z0-9_.-]+)\s*:(.*)$").expect("valid key regex");
    let raw_lines: Vec<&str> = yaml_raw.split_inclusive('\n').collect();
    let mut out_lines: Vec<String> = Vec::with_capacity(raw_lines.len() + patches.len());
    let mut seen_keys: HashMap<String, bool> = HashMap::new();

    let mut i = 0;
    while i < raw_lines.len() {
        let line = raw_lines[i];
        let trimmed_eol = line.trim_end_matches(['\r', '\n']);

        // Preserve comments and blank lines untouched.
        if trimmed_eol.trim().is_empty() || trimmed_eol.trim_start().starts_with('#') {
            out_lines.push(line.to_string());
            i += 1;
            continue;
        }

        // Top-level mapping key (no leading indentation).
        if !trimmed_eol.starts_with([' ', '\t']) {
            if let Some(caps) = top_key_re.captures(trimmed_eol) {
                let key = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let rest = caps.get(2).map(|m| m.as_str()).unwrap_or("");

                // Advance `j` past any indented continuation lines belonging to this key.
                let mut j = i + 1;
                while j < raw_lines.len() {
                    let next_trimmed = raw_lines[j].trim_end_matches(['\r', '\n']);
                    if next_trimmed.starts_with([' ', '\t'])
                        || (next_trimmed.trim_start().starts_with("- ") && rest.trim().is_empty())
                    {
                        j += 1;
                    } else {
                        break;
                    }
                }

                if let Some(replacement) = patch_map.get(key) {
                    seen_keys.insert(key.to_string(), true);
                    if let Some(val) = replacement {
                        out_lines.extend(format_yaml_entry(key, val, eol));
                    }
                    i = j;
                    continue;
                }
            }
        }

        out_lines.push(line.to_string());
        i += 1;
    }

    // Append any newly added keys at the end of the frontmatter block.
    for key in patch_order {
        if !seen_keys.contains_key(key) {
            if let Some(Some(val)) = patch_map.get(key) {
                out_lines.extend(format_yaml_entry(key, val, eol));
            }
        }
    }

    let mut result = String::with_capacity(raw.len() + 64);
    result.push_str("---");
    result.push_str(eol);
    for line in out_lines {
        result.push_str(&line);
    }
    if !result.ends_with('\n') {
        result.push_str(eol);
    }
    result.push_str("---");
    result.push_str(eol);
    result.push_str(split.body);
    Ok(result)
}

/// Toggles `- [ ]` / `- [x]` on the specified 1-based line number without modifying other lines.
pub fn toggle_task_line(raw: &str, line_no: usize, completed: bool) -> Result<String> {
    if line_no == 0 {
        return Err(NodikleError::InvalidPath(
            "line_no must be 1-based".to_string(),
        ));
    }

    let mut lines: Vec<String> = raw.split_inclusive('\n').map(str::to_string).collect();
    if line_no > lines.len() {
        return Err(NodikleError::NotFound(format!(
            "task line {line_no} out of range (total {} lines)",
            lines.len()
        )));
    }

    let target = &lines[line_no - 1];
    let replacement_box = if completed { "[x]" } else { "[ ]" };
    let updated = if let Some(idx) = target.find("[ ]") {
        format!(
            "{}{}{}",
            &target[..idx],
            replacement_box,
            &target[idx + 3..]
        )
    } else if let Some(idx) = target.find("[x]") {
        format!(
            "{}{}{}",
            &target[..idx],
            replacement_box,
            &target[idx + 3..]
        )
    } else if let Some(idx) = target.find("[X]") {
        format!(
            "{}{}{}",
            &target[..idx],
            replacement_box,
            &target[idx + 3..]
        )
    } else {
        return Err(NodikleError::NotFound(format!(
            "line {line_no} is not a markdown checklist item"
        )));
    };

    lines[line_no - 1] = updated;
    Ok(lines.concat())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_comments_unknown_keys_and_body() {
        let raw = "\
---
# Monthly expense record
type: expense
title: \"점심 백반\"
amount: 11000
custom_note: keep-me
tags:
  - 가계부/식비
---

본문 첫째 줄입니다.
- [ ] 영수증 보관
";
        let patches = vec![
            PropertyPatch {
                key: "amount".into(),
                value: Some(PropertyValue::Number(12500.0)),
            },
            PropertyPatch {
                key: "category".into(),
                value: Some(PropertyValue::Text("식비".into())),
            },
        ];

        let patched = patch_frontmatter(raw, &patches).expect("patch should succeed");
        assert!(patched.contains("# Monthly expense record"));
        assert!(patched.contains("custom_note: keep-me"));
        assert!(patched.contains("amount: 12500"));
        assert!(patched.contains("category: 식비"));
        assert!(patched.ends_with("\n본문 첫째 줄입니다.\n- [ ] 영수증 보관\n"));
    }

    #[test]
    fn patch_inserts_frontmatter_when_missing() {
        let raw = "# 일반 메모\n본문입니다.\n";
        let patched = patch_frontmatter(
            raw,
            &[PropertyPatch {
                key: "status".into(),
                value: Some(PropertyValue::Text("계획중".into())),
            }],
        )
        .expect("patch");
        assert!(patched.starts_with("---\nstatus: 계획중\n---\n# 일반 메모\n"));
    }

    #[test]
    fn toggle_task_updates_exact_line_only() {
        let raw = "# 체크\n- [ ] 첫째\n- [x] 둘째\n";
        let toggled = toggle_task_line(raw, 2, true).expect("toggle");
        assert_eq!(toggled, "# 체크\n- [x] 첫째\n- [x] 둘째\n");
    }
}
