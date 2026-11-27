//! Core Markdown, frontmatter, wikilink, tag, and template engine.

pub mod frontmatter;
pub mod parser;
pub mod sample_vault;
pub mod tag;
pub mod template;
pub mod wikilink;

pub use frontmatter::{
    parse_frontmatter_yaml, patch_frontmatter, split_frontmatter, toggle_task_line,
};
pub use parser::{hash_bytes, parse_markdown_document};
pub use sample_vault::starter_vault_files;
pub use tag::{extract_tags, normalize_tag};
pub use template::{render_template, slugify};
pub use wikilink::extract_wikilinks;
