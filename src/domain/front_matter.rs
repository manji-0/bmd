//! Document front matter extracted from metadata blocks.

/// Syntax family of a metadata block at the document start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontMatterKind {
    /// `---` … `---` YAML block.
    Yaml,
    /// `+++` … `+++` TOML block.
    Toml,
}

/// Raw front matter content; not rendered as part of the document body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontMatter {
    pub kind: FrontMatterKind,
    pub raw: String,
}

impl FrontMatter {
    /// Read a string field from parsed front matter, when syntax and type allow.
    pub fn get_str(&self, key: &str) -> Option<String> {
        match self.kind {
            FrontMatterKind::Yaml => yaml_field(&self.raw, key),
            FrontMatterKind::Toml => toml_field(&self.raw, key),
        }
    }

    /// Common `title` field, when present and scalar.
    pub fn title(&self) -> Option<String> {
        self.get_str("title")
    }
}

fn yaml_field(raw: &str, key: &str) -> Option<String> {
    let value: serde_yaml::Value = serde_yaml::from_str(raw).ok()?;
    yaml_scalar_to_string(value.get(key)?)
}

fn toml_field(raw: &str, key: &str) -> Option<String> {
    let table: toml::Table = toml::from_str(raw).ok()?;
    toml_scalar_to_string(table.get(key)?)
}

fn toml_scalar_to_string(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(s) => Some(s.clone()),
        toml::Value::Integer(i) => Some(i.to_string()),
        toml::Value::Float(f) => Some(f.to_string()),
        toml::Value::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

fn yaml_scalar_to_string(value: &serde_yaml::Value) -> Option<String> {
    match value {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_title_field() {
        let fm = FrontMatter {
            kind: FrontMatterKind::Yaml,
            raw: "title: My Doc\ntags: [a, b]\n".into(),
        };
        assert_eq!(fm.title().as_deref(), Some("My Doc"));
        assert_eq!(fm.get_str("tags"), None);
    }

    #[test]
    fn toml_title_field() {
        let fm = FrontMatter {
            kind: FrontMatterKind::Toml,
            raw: "title = \"Guide\"\n".into(),
        };
        assert_eq!(fm.title().as_deref(), Some("Guide"));
    }
}
