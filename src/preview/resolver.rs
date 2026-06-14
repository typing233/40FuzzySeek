use serde::Deserialize;
use std::path::Path;

#[derive(Clone, Debug, Deserialize)]
pub struct PreviewRule {
    pub pattern: String,
    pub cmd: String,
}

#[derive(Clone, Debug)]
pub enum RulePattern {
    Extension(String),
    Prefix(String),
    Contains(String),
    PathExists,
}

struct CompiledRule {
    pattern: RulePattern,
    cmd_template: String,
}

pub struct PreviewResolver {
    rules: Vec<CompiledRule>,
    default_cmd: String,
}

impl PreviewResolver {
    pub fn new(default_cmd: String) -> Self {
        Self {
            rules: Vec::new(),
            default_cmd,
        }
    }

    pub fn add_rule_from_config(&mut self, rule: &PreviewRule) {
        let pattern = parse_rule_pattern(&rule.pattern);
        self.rules.push(CompiledRule {
            pattern,
            cmd_template: rule.cmd.clone(),
        });
    }

    pub fn resolve(&self, line: &str) -> String {
        let trimmed = line.trim();
        for rule in &self.rules {
            if rule.pattern.matches(trimmed) {
                return rule.cmd_template.replace("{}", &shell_escape(trimmed));
            }
        }
        self.default_cmd.replace("{}", &shell_escape(trimmed))
    }

    pub fn has_rules(&self) -> bool {
        !self.rules.is_empty()
    }
}

impl RulePattern {
    pub fn matches(&self, text: &str) -> bool {
        match self {
            RulePattern::Extension(ext) => {
                text.rsplit('.')
                    .next()
                    .map(|e| e.eq_ignore_ascii_case(ext))
                    .unwrap_or(false)
            }
            RulePattern::Prefix(prefix) => text.starts_with(prefix.as_str()),
            RulePattern::Contains(sub) => text.contains(sub.as_str()),
            RulePattern::PathExists => Path::new(text).exists(),
        }
    }
}

fn parse_rule_pattern(pattern: &str) -> RulePattern {
    if let Some(ext) = pattern.strip_prefix("ext:") {
        RulePattern::Extension(ext.to_string())
    } else if let Some(prefix) = pattern.strip_prefix("prefix:") {
        RulePattern::Prefix(prefix.to_string())
    } else if let Some(sub) = pattern.strip_prefix("contains:") {
        RulePattern::Contains(sub.to_string())
    } else if pattern == "path_exists" {
        RulePattern::PathExists
    } else {
        RulePattern::Extension(pattern.to_string())
    }
}

fn shell_escape(s: &str) -> String {
    if s.chars().all(|c| c.is_alphanumeric() || c == '/' || c == '.' || c == '-' || c == '_') {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_matching() {
        let p = RulePattern::Extension("rs".to_string());
        assert!(p.matches("src/main.rs"));
        assert!(!p.matches("src/main.py"));
        assert!(p.matches("file.RS"));
    }

    #[test]
    fn test_prefix_matching() {
        let p = RulePattern::Prefix("/home".to_string());
        assert!(p.matches("/home/user/file"));
        assert!(!p.matches("/tmp/file"));
    }

    #[test]
    fn test_contains_matching() {
        let p = RulePattern::Contains("error".to_string());
        assert!(p.matches("some error here"));
        assert!(!p.matches("all good"));
    }

    #[test]
    fn test_resolver_default() {
        let resolver = PreviewResolver::new("cat {}".to_string());
        assert_eq!(resolver.resolve("file.txt"), "cat file.txt");
    }

    #[test]
    fn test_resolver_with_rule() {
        let mut resolver = PreviewResolver::new("cat {}".to_string());
        resolver.add_rule_from_config(&PreviewRule {
            pattern: "ext:rs".to_string(),
            cmd: "bat --color=always {}".to_string(),
        });
        assert_eq!(resolver.resolve("main.rs"), "bat --color=always main.rs");
        assert_eq!(resolver.resolve("main.py"), "cat main.py");
    }

    #[test]
    fn test_shell_escape() {
        assert_eq!(shell_escape("simple"), "simple");
        assert_eq!(shell_escape("has space"), "'has space'");
        assert_eq!(shell_escape("it's"), "'it'\\''s'");
    }

    #[test]
    fn test_parse_rule_pattern() {
        assert!(matches!(parse_rule_pattern("ext:py"), RulePattern::Extension(_)));
        assert!(matches!(parse_rule_pattern("prefix:/usr"), RulePattern::Prefix(_)));
        assert!(matches!(parse_rule_pattern("contains:foo"), RulePattern::Contains(_)));
        assert!(matches!(parse_rule_pattern("path_exists"), RulePattern::PathExists));
        assert!(matches!(parse_rule_pattern("rs"), RulePattern::Extension(_)));
    }
}
