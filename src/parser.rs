use regex::Regex;

#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub content: String,
    pub lang: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ShellCommand {
    pub command: String,
    pub lang: String,
}

pub fn parse_markdown(text: &str) -> (Vec<FileChange>, Vec<ShellCommand>) {
    let mut file_changes = Vec::new();
    let mut shell_commands = Vec::new();

    // Pattern for code blocks: ```[lang][ filename]\ncontent\n```
    let re = Regex::new(r"(?s)```(?P<header>[^\n\r]*)\r?\n(?P<content>.*?)\r?\n```").unwrap();

    let mut found = false;
    for cap in re.captures_iter(text) {
        found = true;
        let header = cap.name("header").map(|m| m.as_str().trim()).unwrap_or("");
        let content = cap.name("content").map(|m| m.as_str().to_string()).unwrap_or_default();

        let parts: Vec<&str> = header.split_whitespace().collect();
        let lang = parts.first().map(|s| s.to_string());
        let filename = parts.get(1).map(|s| s.to_string());

        let l = lang.clone().unwrap_or_default().to_lowercase();
        
        if (l == "bash" || l == "sh" || l == "shell") && filename.is_none() {
            shell_commands.push(ShellCommand {
                command: content.trim().to_string(),
                lang: l,
            });
        } else if let Some(path) = filename {
            file_changes.push(FileChange {
                path,
                content,
                lang,
            });
        } else {
            // Fallback: check preceding text for filename
            let match_start = cap.get(0).unwrap().start();
            let preceding = &text[..match_start];
            let last_lines = preceding.lines().rev().take(3).collect::<Vec<&str>>();
            
            let mut found_file = false;
            let file_ref_re = Regex::new(r"(?i)(?:File|path|Filename|Target):\s*([^\s\n`]+)").unwrap();
            for line in last_lines {
                if let Some(file_cap) = file_ref_re.captures(line) {
                    file_changes.push(FileChange {
                        path: file_cap.get(1).unwrap().as_str().to_string(),
                        content: content.clone(),
                        lang: if lang.is_some() { lang.clone() } else { None },
                    });
                    found_file = true;
                    break;
                }
            }

            if !found_file {
                if l == "bash" || l == "sh" || l == "shell" || l == "zsh" || l.is_empty() {
                    shell_commands.push(ShellCommand {
                        command: content.trim().to_string(),
                        lang: l,
                    });
                }
            }
        }
    }

    (file_changes, shell_commands)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_markdown_file_change() {
        let text = "
```python src/app.py
print('hello world')
```
";
        let (file_changes, _) = parse_markdown(text);
        assert_eq!(file_changes.len(), 1);
        assert_eq!(file_changes[0].path, "src/app.py");
    }

    #[test]
    fn test_parse_markdown_shell_command() {
        let text = "
```bash
ls -la
```
";
        let (_, shell_commands) = parse_markdown(text);
        assert_eq!(shell_commands.len(), 1);
        assert_eq!(shell_commands[0].command, "ls -la");
    }

    #[test]
    fn test_parse_markdown_mixed() {
        let text = "
File: README.md
```markdown
# My Project
```

And then run:
```bash
cat README.md
```
";
        let (file_changes, shell_commands) = parse_markdown(text);
        assert_eq!(file_changes.len(), 1);
        assert_eq!(file_changes[0].path, "README.md");
        assert_eq!(shell_commands.len(), 1);
        assert_eq!(shell_commands[0].command, "cat README.md");
    }
}
