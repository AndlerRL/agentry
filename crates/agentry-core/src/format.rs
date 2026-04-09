use crate::models::{PromptFormat, UnifiedPrompt, XmlTagWrap};
use std::collections::BTreeMap;

/// Trait for converting between UnifiedPrompt and a specific file format.
pub trait FormatConverter {
    fn format(&self) -> PromptFormat;
    /// Parse raw file content into a UnifiedPrompt.
    fn parse(&self, name: &str, content: &str, path: Option<std::path::PathBuf>) -> anyhow::Result<UnifiedPrompt>;
    /// Serialize a UnifiedPrompt into this format's file content.
    fn serialize(&self, prompt: &UnifiedPrompt) -> anyhow::Result<String>;
}

// ─── Plain Markdown ──────────────────────────────────────────────────────────

pub struct PlainMarkdownConverter;

impl FormatConverter for PlainMarkdownConverter {
    fn format(&self) -> PromptFormat {
        PromptFormat::PlainMd
    }

    fn parse(&self, name: &str, content: &str, path: Option<std::path::PathBuf>) -> anyhow::Result<UnifiedPrompt> {
        Ok(UnifiedPrompt {
            id: name.to_string(),
            name: name.to_string(),
            description: String::new(),
            frontmatter: BTreeMap::new(),
            body: content.to_string(),
            xml_tags: Vec::new(),
            scope: crate::models::PromptScope::Global,
            source_format: PromptFormat::PlainMd,
            source_path: path,
        })
    }

    fn serialize(&self, prompt: &UnifiedPrompt) -> anyhow::Result<String> {
        Ok(prompt.body.clone())
    }
}

// ─── Frontmatter + Markdown ──────────────────────────────────────────────────

pub struct FrontmatterMdConverter;

impl FormatConverter for FrontmatterMdConverter {
    fn format(&self) -> PromptFormat {
        PromptFormat::FrontmatterMd
    }

    fn parse(&self, name: &str, content: &str, path: Option<std::path::PathBuf>) -> anyhow::Result<UnifiedPrompt> {
        let (frontmatter, body) = parse_frontmatter(content)?;

        let description = frontmatter
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(UnifiedPrompt {
            id: name.to_string(),
            name: name.to_string(),
            description,
            frontmatter,
            body,
            xml_tags: Vec::new(),
            scope: crate::models::PromptScope::Global,
            source_format: PromptFormat::FrontmatterMd,
            source_path: path,
        })
    }

    fn serialize(&self, prompt: &UnifiedPrompt) -> anyhow::Result<String> {
        let mut out = String::from("---\n");
        // Always write name and description in frontmatter
        let mut fm = prompt.frontmatter.clone();
        fm.insert("name".to_string(), serde_yaml::Value::String(prompt.name.clone()));
        if !prompt.description.is_empty() {
            fm.insert(
                "description".to_string(),
                serde_yaml::Value::String(prompt.description.clone()),
            );
        }
        out.push_str(&serde_yaml::to_string(&fm)?);
        out.push_str("---\n\n");
        out.push_str(&prompt.body);
        Ok(out)
    }
}

// ─── MDC Format (Firebender) ─────────────────────────────────────────────────

pub struct MdcConverter;

impl FormatConverter for MdcConverter {
    fn format(&self) -> PromptFormat {
        PromptFormat::Mdc
    }

    fn parse(&self, name: &str, content: &str, path: Option<std::path::PathBuf>) -> anyhow::Result<UnifiedPrompt> {
        // MDC files have --- frontmatter --- but with different fields (globs, description)
        let (frontmatter, body) = parse_frontmatter(content)?;
        let description = frontmatter
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(UnifiedPrompt {
            id: name.to_string(),
            name: name.to_string(),
            description,
            frontmatter,
            body,
            xml_tags: Vec::new(),
            scope: crate::models::PromptScope::Global,
            source_format: PromptFormat::Mdc,
            source_path: path,
        })
    }

    fn serialize(&self, prompt: &UnifiedPrompt) -> anyhow::Result<String> {
        let mut fm = prompt.frontmatter.clone();
        fm.insert("description".to_string(), serde_yaml::Value::String(prompt.description.clone()));
        let mut out = String::from("---\n");
        out.push_str(&serde_yaml::to_string(&fm)?);
        out.push_str("---\n\n");
        out.push_str(&prompt.body);
        Ok(out)
    }
}

// ─── XML Tag Markdown ────────────────────────────────────────────────────────

pub struct XmlTagMdConverter;

impl FormatConverter for XmlTagMdConverter {
    fn format(&self) -> PromptFormat {
        PromptFormat::XmlTagMd
    }

    fn parse(&self, name: &str, content: &str, path: Option<std::path::PathBuf>) -> anyhow::Result<UnifiedPrompt> {
        // May have frontmatter + XML tags in body
        let (frontmatter, body) = parse_frontmatter(content)?;

        let description = frontmatter
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Extract XML tags from body
        let (clean_body, xml_tags) = extract_xml_tags(&body);

        Ok(UnifiedPrompt {
            id: name.to_string(),
            name: name.to_string(),
            description,
            frontmatter,
            body: clean_body,
            xml_tags,
            scope: crate::models::PromptScope::Global,
            source_format: PromptFormat::XmlTagMd,
            source_path: path,
        })
    }

    fn serialize(&self, prompt: &UnifiedPrompt) -> anyhow::Result<String> {
        let mut out = String::new();
        if !prompt.frontmatter.is_empty() || !prompt.description.is_empty() || !prompt.name.is_empty() {
            out.push_str("---\n");
            let mut fm = prompt.frontmatter.clone();
            fm.insert("name".to_string(), serde_yaml::Value::String(prompt.name.clone()));
            if !prompt.description.is_empty() {
                fm.insert("description".to_string(), serde_yaml::Value::String(prompt.description.clone()));
            }
            out.push_str(&serde_yaml::to_string(&fm)?);
            out.push_str("---\n\n");
        }

        // Wrap body in XML tags
        if let Some(tag) = prompt.xml_tags.first() {
            out.push_str(&format!("<{}>\n{}\n</{}>\n", tag.tag, prompt.body.trim(), tag.tag));
        } else {
            out.push_str(&prompt.body);
        }
        Ok(out)
    }
}

// ─── Lobster YAML ────────────────────────────────────────────────────────────

pub struct LobsterYamlConverter;

impl FormatConverter for LobsterYamlConverter {
    fn format(&self) -> PromptFormat {
        PromptFormat::LobsterYaml
    }

    fn parse(&self, name: &str, content: &str, path: Option<std::path::PathBuf>) -> anyhow::Result<UnifiedPrompt> {
        let yaml: serde_yaml::Value = serde_yaml::from_str(content)?;
        let mut frontmatter = BTreeMap::new();

        if let serde_yaml::Value::Mapping(map) = &yaml {
            for (k, v) in map {
                if let serde_yaml::Value::String(k_str) = k {
                    frontmatter.insert(k_str.clone(), v.clone());
                }
            }
        }

        let description = frontmatter
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(UnifiedPrompt {
            id: name.to_string(),
            name: name.to_string(),
            description,
            frontmatter,
            body: content.to_string(),
            xml_tags: Vec::new(),
            scope: crate::models::PromptScope::Global,
            source_format: PromptFormat::LobsterYaml,
            source_path: path,
        })
    }

    fn serialize(&self, prompt: &UnifiedPrompt) -> anyhow::Result<String> {
        // For Lobster YAML, the body is the raw YAML content
        Ok(prompt.body.clone())
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Parse frontmatter from a string. Returns (frontmatter_map, remaining_body).
fn parse_frontmatter(content: &str) -> anyhow::Result<(BTreeMap<String, serde_yaml::Value>, String)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Ok((BTreeMap::new(), content.to_string()));
    }

    // Find closing ---
    let after_first = &trimmed[3..];
    let start = after_first.find(|c: char| !c.is_whitespace()).unwrap_or(0);
    let search_from = 3 + start;
    let remaining = &content[search_from..];

    if let Some(close_offset) = remaining.find("\n---") {
        let fm_content = &remaining[..close_offset];
        let body_start = close_offset + 4; // skip \n---
        let body = remaining.get(body_start..).unwrap_or("").trim_start().to_string();

        let fm: BTreeMap<String, serde_yaml::Value> = serde_yaml::from_str(fm_content)?;
        Ok((fm, body))
    } else {
        Ok((BTreeMap::new(), content.to_string()))
    }
}

/// Extract XML tag wrappers from markdown body.
/// Returns (cleaned_body, extracted_tags).
fn extract_xml_tags(body: &str) -> (String, Vec<XmlTagWrap>) {
    // Simple loop-based XML tag extraction (no regex dependency needed)
    let mut result = body.to_string();
    let mut tags_found = Vec::new();

    while let Some(p) = result.find('<') {
        let open_start = p;
        let open_end = match result[open_start..].find('>') {
            Some(p) => open_start + p,
            None => break,
        };
        let tag_name = result[open_start + 1..open_end].trim().to_string();

        // Skip if it looks like an HTML comment or processing instruction
        if tag_name.starts_with('!') || tag_name.starts_with('?') || tag_name.starts_with('/') {
            // Not a simple tag, skip
            break;
        }

        let close_tag = format!("</{}>", tag_name);
        let close_pos = match result[open_end + 1..].find(&close_tag) {
            Some(p) => open_end + 1 + p,
            None => break,
        };

        let content = result[open_end + 1..close_pos].trim().to_string();
        tags_found.push(XmlTagWrap {
            tag: tag_name,
            content: content.clone(),
        });

        // Remove the tag from the result, leaving just the content
        let _full_tag_len = close_pos + close_tag.len() - open_start;
        let content_with_newlines = format!("\n{}\n", content);
        result = format!(
            "{}{}{}",
            &result[..open_start],
            content_with_newlines,
            &result[close_pos + close_tag.len()..]
        );
    }

    (result.trim().to_string(), tags_found)
}

/// Get the appropriate converter for a given format.
pub fn converter_for(format: PromptFormat) -> Box<dyn FormatConverter> {
    match format {
        PromptFormat::PlainMd => Box::new(PlainMarkdownConverter),
        PromptFormat::FrontmatterMd => Box::new(FrontmatterMdConverter),
        PromptFormat::Mdc => Box::new(MdcConverter),
        PromptFormat::XmlTagMd => Box::new(XmlTagMdConverter),
        PromptFormat::LobsterYaml => Box::new(LobsterYamlConverter),
    }
}

/// Convert a UnifiedPrompt to a target format.
pub fn convert_to(prompt: &UnifiedPrompt, target_format: PromptFormat) -> anyhow::Result<String> {
    let converter = converter_for(target_format);
    converter.serialize(prompt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PromptScope;

    #[test]
    fn test_plain_md_roundtrip() {
        let content = "# Hello\n\nThis is plain markdown.\n";
        let converter = PlainMarkdownConverter;
        let prompt = converter.parse("test", content, None).unwrap();
        assert_eq!(prompt.body, content);
        let serialized = converter.serialize(&prompt).unwrap();
        assert_eq!(serialized, content);
    }

    #[test]
    fn test_frontmatter_md_parse() {
        let content = "---\nname: software-architect\ndescription: A prompt\ninvokable: true\n---\n\nYou are an architect.\n";
        let converter = FrontmatterMdConverter;
        let prompt = converter.parse("software-architect", content, None).unwrap();
        assert_eq!(prompt.name, "software-architect");
        assert_eq!(prompt.description, "A prompt");
        assert!(prompt.body.contains("You are an architect"));
    }

    #[test]
    fn test_frontmatter_md_serialize() {
        let mut fm = BTreeMap::new();
        fm.insert("invokable".to_string(), serde_yaml::Value::Bool(true));
        let prompt = UnifiedPrompt {
            id: "test".to_string(),
            name: "software-architect".to_string(),
            description: "A prompt".to_string(),
            frontmatter: fm,
            body: "You are an architect.\n".to_string(),
            xml_tags: vec![],
            scope: PromptScope::Global,
            source_format: PromptFormat::FrontmatterMd,
            source_path: None,
        };
        let converter = FrontmatterMdConverter;
        let serialized = converter.serialize(&prompt).unwrap();
        assert!(serialized.starts_with("---"));
        assert!(serialized.contains("name: software-architect"));
        assert!(serialized.contains("You are an architect"));
    }

    #[test]
    fn test_xml_tag_parse() {
        let content = "---\nname: test\n---\n\n<expertise>\nYou are a senior developer.\n</expertise>\n";
        let converter = XmlTagMdConverter;
        let prompt = converter.parse("test", content, None).unwrap();
        assert_eq!(prompt.xml_tags.len(), 1);
        assert_eq!(prompt.xml_tags[0].tag, "expertise");
        assert!(prompt.xml_tags[0].content.contains("senior developer"));
    }

    #[test]
    fn test_xml_tag_roundtrip() {
        let content = "---\nname: software-architect\ndescription: Software Architect prompt\ninvokable: true\n---\n\n<expertise>\nYou are a senior software architect.\n</expertise>\n";
        let converter = XmlTagMdConverter;
        let prompt = converter.parse("software-architect", content, None).unwrap();
        assert_eq!(prompt.xml_tags.len(), 1);
        let serialized = converter.serialize(&prompt).unwrap();
        assert!(serialized.contains("<expertise>"));
        assert!(serialized.contains("</expertise>"));
    }

    #[test]
    fn test_mdc_roundtrip() {
        let content = "---\ndescription: TypeScript best practices\nglobs: \"**/*.ts\"\n---\n\nAlways use strict TypeScript.\n";
        let converter = MdcConverter;
        let prompt = converter.parse("typescript-rules", content, None).unwrap();
        assert_eq!(prompt.description, "TypeScript best practices");
        let serialized = converter.serialize(&prompt).unwrap();
        assert!(serialized.starts_with("---"));
        assert!(serialized.contains("globs"));
    }

    #[test]
    fn test_frontmatter_roundtrip() {
        let content = "---\nname: my-prompt\ndescription: Test prompt\ninvokable: true\n---\n\nSome body text.\n";
        let converter = FrontmatterMdConverter;
        let prompt = converter.parse("my-prompt", content, None).unwrap();
        let serialized = converter.serialize(&prompt).unwrap();
        // Re-parse the serialized output
        let reparsed = converter.parse("my-prompt", &serialized, None).unwrap();
        assert_eq!(reparsed.name, prompt.name);
        assert_eq!(reparsed.description, prompt.description);
        assert_eq!(reparsed.body.trim(), prompt.body.trim());
    }

    #[test]
    fn test_cross_format_conversion() {
        // Parse a FrontmatterMd prompt and convert to PlainMd
        let fm_content = "---\nname: test\ndescription: A test\n---\n\nHello world.\n";
        let fm_converter = FrontmatterMdConverter;
        let prompt = fm_converter.parse("test", fm_content, None).unwrap();

        let plain_output = convert_to(&prompt, PromptFormat::PlainMd).unwrap();
        assert!(plain_output.contains("Hello world"));
        assert!(!plain_output.contains("---"));
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let content = "Just plain text without any frontmatter.";
        let converter = FrontmatterMdConverter;
        let prompt = converter.parse("test", content, None).unwrap();
        assert!(prompt.frontmatter.is_empty());
        assert_eq!(prompt.body, content);
    }

    #[test]
    fn test_xml_base_rules_tag() {
        let content = "---\ndescription: Base rules\n---\n\n<base_rules>\n- Do not refactor unrelated code.\n- Follow @AGENTS.md.\n</base_rules>\n";
        let converter = XmlTagMdConverter;
        let prompt = converter.parse("base-rules", content, None).unwrap();
        assert_eq!(prompt.xml_tags.len(), 1);
        assert_eq!(prompt.xml_tags[0].tag, "base_rules");
        assert!(prompt.xml_tags[0].content.contains("Do not refactor"));
    }
}