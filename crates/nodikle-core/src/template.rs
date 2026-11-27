//! Template variable substitution and filename slug generation.

use std::collections::BTreeMap;

use chrono::Local;
use nodikle_model::PropertyValue;
use uuid::Uuid;

/// Renders a template string by replacing `{{var}}` and `{{var|default}}` placeholders.
pub fn render_template(
    template: &str,
    title: &str,
    vars: &BTreeMap<String, PropertyValue>,
) -> String {
    let now = Local::now();
    let date_str = now.format("%Y-%m-%d").to_string();
    let time_str = now.format("%H:%M").to_string();
    let datetime_str = now.to_rfc3339();
    let uuid_str = Uuid::now_v7().to_string();
    let slug_str = slugify(title);

    // Pre-pass: resolve any nested `{{date}}` / `{{time}}` tokens inside defaults.
    let mut out = String::with_capacity(template.len() + 64);
    let mut rest = template;

    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after_open = &rest[start + 2..];
        let Some(end) = find_matching_close(after_open) else {
            out.push_str(&rest[start..]);
            return out;
        };

        let expr = &after_open[..end];
        let (key, default_expr) = match expr.split_once('|') {
            Some((k, d)) => (k.trim(), Some(d.trim())),
            None => (expr.trim(), None),
        };

        let resolved = match key {
            "date" => Some(date_str.clone()),
            "time" => Some(time_str.clone()),
            "datetime" => Some(datetime_str.clone()),
            "uuid" => Some(uuid_str.clone()),
            "title" => Some(title.to_string()),
            "slug" => Some(slug_str.clone()),
            other => vars.get(other).and_then(PropertyValue::as_text),
        };

        let final_val = match resolved {
            Some(v) if !v.is_empty() => v,
            _ => match default_expr {
                Some(def) => render_template(def, title, vars),
                None => String::new(),
            },
        };

        out.push_str(&final_val);
        rest = &after_open[end + 2..];
    }

    out.push_str(rest);
    out
}

fn find_matching_close(s: &str) -> Option<usize> {
    let mut depth = 1usize;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'{' && bytes[i + 1] == b'{' {
            depth += 1;
            i += 2;
            continue;
        }
        if bytes[i] == b'}' && bytes[i + 1] == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
            i += 2;
            continue;
        }
        i += 1;
    }
    None
}

/// Converts a document title into a safe cross-platform filename slug (preserving Hangul).
pub fn slugify(input: &str) -> String {
    let mut slug = String::with_capacity(input.len());
    let mut prev_dash = false;

    for ch in input.trim().chars() {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                slug.push(lower);
            }
            prev_dash = false;
        } else if !prev_dash && !slug.is_empty() {
            slug.push('-');
            prev_dash = true;
        }
    }

    let trimmed = slug.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_variables_and_nested_defaults() {
        let mut vars = BTreeMap::new();
        vars.insert("amount".into(), PropertyValue::Number(14000.0));

        let tpl = "title: {{title}}\ndate: {{start_date|{{date}}}}\namount: {{amount|0}}\ncat: {{category|식비}}";
        let rendered = render_template(tpl, "교토 라멘", &vars);

        assert!(rendered.contains("title: 교토 라멘"));
        assert!(rendered.contains("amount: 14000"));
        assert!(rendered.contains("cat: 식비"));
        assert!(!rendered.contains("{{"));
    }

    #[test]
    fn slugifies_unicode_and_separators() {
        assert_eq!(slugify("2026 겨울 교토 3박 4일!"), "2026-겨울-교토-3박-4일");
        assert_eq!(slugify("   "), "untitled");
    }
}
