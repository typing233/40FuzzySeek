use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParserKind {
    PsAux,
    Ls,
    History,
    Csv { delimiter: Option<char> },
    Custom { separator: String, fields: Vec<usize> },
}

pub struct ParsedLine {
    pub display: String,
    pub search_text: String,
    pub output_text: String,
}

impl ParserKind {
    pub fn parse_line(&self, line: &str) -> ParsedLine {
        match self {
            ParserKind::PsAux => parse_ps_aux(line),
            ParserKind::Ls => parse_ls(line),
            ParserKind::History => parse_history(line),
            ParserKind::Csv { delimiter } => parse_csv(line, delimiter.unwrap_or(',')),
            ParserKind::Custom { separator, fields } => parse_custom(line, separator, fields),
        }
    }

    pub fn detect(sample_lines: &[&str]) -> Option<Self> {
        if sample_lines.is_empty() {
            return None;
        }
        let first = sample_lines[0];

        if first.contains("USER") && first.contains("PID") && first.contains("COMMAND") {
            return Some(ParserKind::PsAux);
        }

        if first.starts_with("total ") || looks_like_ls(first) {
            return Some(ParserKind::Ls);
        }

        if looks_like_history(first) {
            return Some(ParserKind::History);
        }

        None
    }
}

fn parse_ps_aux(line: &str) -> ParsedLine {
    let parts: Vec<&str> = line.split_whitespace().collect();
    let cmd = if parts.len() >= 11 {
        parts[10..].join(" ")
    } else {
        line.to_string()
    };
    ParsedLine {
        display: line.to_string(),
        search_text: cmd.clone(),
        output_text: cmd,
    }
}

fn parse_ls(line: &str) -> ParsedLine {
    let parts: Vec<&str> = line.split_whitespace().collect();
    let filename = parts.last().unwrap_or(&line).to_string();
    ParsedLine {
        display: line.to_string(),
        search_text: filename.clone(),
        output_text: filename,
    }
}

fn parse_history(line: &str) -> ParsedLine {
    let trimmed = line.trim_start();
    let cmd = if let Some(pos) = trimmed.find(|c: char| !c.is_ascii_digit() && c != ' ') {
        trimmed[pos..].trim_start().to_string()
    } else {
        trimmed.to_string()
    };
    ParsedLine {
        display: line.to_string(),
        search_text: cmd.clone(),
        output_text: cmd,
    }
}

fn parse_csv(line: &str, delimiter: char) -> ParsedLine {
    ParsedLine {
        display: line.to_string(),
        search_text: line.to_string(),
        output_text: line.to_string(),
    }
}

fn parse_custom(line: &str, separator: &str, fields: &[usize]) -> ParsedLine {
    let parts: Vec<&str> = line.split(separator).collect();
    let selected: Vec<&str> = fields.iter()
        .filter_map(|&i| parts.get(i).copied())
        .collect();
    let output = selected.join(separator);
    ParsedLine {
        display: line.to_string(),
        search_text: output.clone(),
        output_text: output,
    }
}

fn looks_like_ls(line: &str) -> bool {
    let first_char = line.chars().next().unwrap_or(' ');
    matches!(first_char, '-' | 'd' | 'l' | 'c' | 'b' | 'p' | 's')
        && line.len() > 10
        && line.chars().nth(1).map(|c| matches!(c, 'r' | 'w' | 'x' | '-')).unwrap_or(false)
}

fn looks_like_history(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with(|c: char| c.is_ascii_digit())
        && trimmed.contains(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ps_aux() {
        let line = "root         1  0.0  0.1 169936 11840 ?        Ss   Jun01   0:12 /sbin/init";
        let parsed = parse_ps_aux(line);
        assert_eq!(parsed.output_text, "/sbin/init");
    }

    #[test]
    fn test_parse_ps_aux_with_args() {
        let line = "user      1234  1.2  0.5 123456 56789 pts/0  S+   10:00   0:05 python3 my_script.py --verbose";
        let parsed = parse_ps_aux(line);
        assert_eq!(parsed.output_text, "python3 my_script.py --verbose");
        assert_eq!(parsed.search_text, "python3 my_script.py --verbose");
        // Display shows the full line
        assert_eq!(parsed.display, line);
    }

    #[test]
    fn test_parse_history() {
        let line = "  123  git status";
        let parsed = parse_history(line);
        assert_eq!(parsed.output_text, "git status");
    }

    #[test]
    fn test_parse_history_strips_number() {
        let line = "  9999  cd /home/user && ls -la";
        let parsed = parse_history(line);
        assert_eq!(parsed.output_text, "cd /home/user && ls -la");
        assert_eq!(parsed.search_text, "cd /home/user && ls -la");
    }

    #[test]
    fn test_parse_ls_extracts_filename() {
        let line = "-rw-r--r--  1 user user 12345 Jan 15 10:30 my file.txt";
        let parsed = parse_ls(line);
        assert_eq!(parsed.output_text, "file.txt");
        // Display shows full ls output
        assert_eq!(parsed.display, line);
    }

    #[test]
    fn test_detect_ps_aux() {
        let lines = vec!["USER       PID %CPU %MEM    VSZ   RSS TTY      STAT START   TIME COMMAND"];
        assert!(matches!(ParserKind::detect(&lines), Some(ParserKind::PsAux)));
    }

    #[test]
    fn test_detect_history() {
        let lines = vec!["  1  ls -la", "  2  cd /tmp"];
        assert!(matches!(ParserKind::detect(&lines), Some(ParserKind::History)));
    }

    #[test]
    fn test_detect_ls() {
        let lines = vec!["drwxr-xr-x  2 user user 4096 Jan  1 00:00 dir"];
        assert!(matches!(ParserKind::detect(&lines), Some(ParserKind::Ls)));
    }

    #[test]
    fn test_detect_plain_text_returns_none() {
        let lines = vec!["hello world", "foo bar", "baz"];
        assert!(ParserKind::detect(&lines).is_none());
    }

    #[test]
    fn test_parse_custom() {
        let line = "field0:field1:field2:field3";
        let parsed = parse_custom(line, ":", &[1, 3]);
        assert_eq!(parsed.output_text, "field1:field3");
    }

    #[test]
    fn test_parser_integration_ps_output_text() {
        // Simulate: user pipes `ps aux`, selects a line, gets the command
        let parser = ParserKind::PsAux;
        let lines = vec![
            "USER       PID %CPU %MEM    VSZ   RSS TTY      STAT START   TIME COMMAND",
            "root         1  0.0  0.1 169936 11840 ?        Ss   Jun01   0:12 /sbin/init splash",
            "user      5678  2.0  1.5 654321 98765 pts/1    Sl+  14:00   1:23 vim main.rs",
        ];
        // First line is header, would be filtered by search
        // When user selects 3rd line, output_text is "vim main.rs"
        let parsed = parser.parse_line(lines[2]);
        assert_eq!(parsed.output_text, "vim main.rs");
        assert_eq!(parsed.search_text, "vim main.rs");
    }

    #[test]
    fn test_parser_integration_history_output_text() {
        let parser = ParserKind::History;
        let line = "  42  docker compose up -d";
        let parsed = parser.parse_line(line);
        assert_eq!(parsed.output_text, "docker compose up -d");
    }
}
