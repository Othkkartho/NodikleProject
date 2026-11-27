//! Markdown + YAML frontmatter parser with code-block awareness.

use std::collections::BTreeMap;

use nodikle_model::{ParsedDocument, PropertyValue, Result, TaskItem};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::frontmatter::{parse_frontmatter_yaml, split_frontmatter};
use crate::tag::extract_tags;
use crate::wikilink::extract_wikilinks;

/// Computes the lowercase hex BLAKE3 digest of arbitrary bytes.
pub fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// Parses a complete raw Markdown document into structured metadata, links, tags, and tasks.
pub fn parse_markdown_document(raw: &str) -> Result<ParsedDocument> {
    let content_hash = hash_bytes(raw.as_bytes());
    let split = split_frontmatter(raw);

    let (properties, ordered_keys) = match split.yaml_raw {
        Some(yaml) => parse_frontmatter_yaml(yaml)?,
        None => (BTreeMap::new(), Vec::new()),
    };

    let id = properties.get("id").and_then(PropertyValue::as_text);
    let doc_type = properties.get("type").and_then(PropertyValue::as_text);
    let fm_title = properties.get("title").and_then(PropertyValue::as_text);

    let body_md = split.body.to_string();
    let (body_plain, first_h1, non_code_segments) = extract_markdown_text(&body_md);

    let segment_refs: Vec<&str> = non_code_segments.iter().map(String::as_str).collect();
    let wikilinks = extract_wikilinks(&segment_refs);
    let tags = extract_tags(properties.get("tags"), &segment_refs);
    let tasks = extract_tasks_with_line_numbers(raw, split.body);

    let title = fm_title.or(first_h1);

    Ok(ParsedDocument {
        id,
        title,
        doc_type,
        properties,
        ordered_keys,
        tags,
        wikilinks,
        tasks,
        body_md,
        body_plain,
        content_hash,
    })
}

fn extract_markdown_text(body: &str) -> (String, Option<String>, Vec<String>) {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(body, options);
    let mut plain = String::with_capacity(body.len());
    let mut non_code_segments = Vec::new();
    let mut current_segment = String::new();
    let mut first_h1: Option<String> = None;
    let mut in_h1 = false;
    let mut h1_buf = String::new();
    let mut in_code_block = false;

    let flush_segment = |buf: &mut String, out: &mut Vec<String>| {
        if !buf.is_empty() {
            out.push(std::mem::take(buf));
        }
    };

    for event in parser {
        match event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                ..
            }) => {
                if first_h1.is_none() {
                    in_h1 = true;
                    h1_buf.clear();
                }
            }
            Event::End(TagEnd::Heading(HeadingLevel::H1)) => {
                if in_h1 {
                    let trimmed = h1_buf.trim().to_string();
                    if !trimmed.is_empty() {
                        first_h1 = Some(trimmed);
                    }
                    in_h1 = false;
                }
                flush_segment(&mut current_segment, &mut non_code_segments);
                plain.push('\n');
            }
            Event::Start(Tag::CodeBlock(_)) => {
                flush_segment(&mut current_segment, &mut non_code_segments);
                in_code_block = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                plain.push('\n');
            }
            Event::Text(text) => {
                if in_h1 {
                    h1_buf.push_str(&text);
                }
                if !in_code_block {
                    current_segment.push_str(&text);
                }
                plain.push_str(&text);
            }
            Event::Code(code) => {
                flush_segment(&mut current_segment, &mut non_code_segments);
                if in_h1 {
                    h1_buf.push_str(&code);
                }
                plain.push_str(&code);
            }
            Event::SoftBreak | Event::HardBreak => {
                flush_segment(&mut current_segment, &mut non_code_segments);
                plain.push('\n');
            }
            Event::End(TagEnd::Paragraph | TagEnd::Item | TagEnd::Heading(_)) => {
                flush_segment(&mut current_segment, &mut non_code_segments);
                plain.push('\n');
            }
            _ => {}
        }
    }
    flush_segment(&mut current_segment, &mut non_code_segments);

    (plain.trim().to_string(), first_h1, non_code_segments)
}

fn extract_tasks_with_line_numbers(full_raw: &str, body: &str) -> Vec<TaskItem> {
    let body_offset_bytes = full_raw.len().saturating_sub(body.len());
    let start_line_no = full_raw[..body_offset_bytes].matches('\n').count() + 1;

    let mut tasks = Vec::new();
    let mut in_fence: Option<(char, usize)> = None;

    for (idx, line) in body.lines().enumerate() {
        let trimmed_start = line.trim_start();
        if let Some((fence_ch, fence_len)) = in_fence {
            let count = trimmed_start.chars().take_while(|c| *c == fence_ch).count();
            if count >= fence_len && trimmed_start[count..].trim().is_empty() {
                in_fence = None;
            }
            continue;
        }

        if trimmed_start.starts_with("```") || trimmed_start.starts_with("~~~") {
            let ch = trimmed_start.chars().next().unwrap_or('`');
            let count = trimmed_start.chars().take_while(|c| *c == ch).count();
            in_fence = Some((ch, count));
            continue;
        }

        if let Some(rest) = strip_list_marker(trimmed_start) {
            if let Some(body_text) = rest.strip_prefix("[ ] ") {
                tasks.push(TaskItem {
                    line_no: start_line_no + idx,
                    completed: false,
                    body: body_text.trim().to_string(),
                });
            } else if let Some(body_text) = rest
                .strip_prefix("[x] ")
                .or_else(|| rest.strip_prefix("[X] "))
            {
                tasks.push(TaskItem {
                    line_no: start_line_no + idx,
                    completed: true,
                    body: body_text.trim().to_string(),
                });
            }
        }
    }

    tasks
}

fn strip_list_marker(trimmed: &str) -> Option<&str> {
    if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
    {
        return Some(rest);
    }

    // Ordered list marker such as `1. [ ] task`
    let digit_len = trimmed.bytes().take_while(|b| b.is_ascii_digit()).count();
    if digit_len > 0 {
        let after_digits = &trimmed[digit_len..];
        if let Some(rest) = after_digits
            .strip_prefix(". ")
            .or_else(|| after_digits.strip_prefix(") "))
        {
            return Some(rest);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_wikilinks_tags_and_tasks_inside_code_blocks() {
        let raw = "\
---
title: 테스트 노트
---

본문 [[valid-link]] 그리고 #valid/tag

- [ ] 실제 할 일
- [x] 완료된 할 일

```markdown
[[ignored-in-fence]]
#ignored/tag
- [ ] fenced task
```

인라인 코드 `[[ignored-inline]] #ignored-inline`
";
        let parsed = parse_markdown_document(raw).expect("parse should succeed");
        assert_eq!(parsed.wikilinks.len(), 1);
        assert_eq!(parsed.wikilinks[0].raw_target, "valid-link");
        assert_eq!(parsed.tags, vec!["valid/tag"]);
        assert_eq!(parsed.tasks.len(), 2);
        assert_eq!(parsed.tasks[0].line_no, 7);
        assert!(!parsed.tasks[0].completed);
        assert_eq!(parsed.tasks[1].line_no, 8);
        assert!(parsed.tasks[1].completed);
    }

    #[test]
    fn falls_back_to_first_h1_when_frontmatter_title_absent() {
        let raw = "# 첫 번째 제목\n\n내용입니다.\n";
        let parsed = parse_markdown_document(raw).expect("parse");
        assert_eq!(parsed.title.as_deref(), Some("첫 번째 제목"));
    }
}
