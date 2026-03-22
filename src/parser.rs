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

    for cap in re.captures_iter(text) {
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

    #[test]
    fn test_parse_markdown_empty_input() {
        let (file_changes, shell_commands) = parse_markdown("");
        assert!(file_changes.is_empty());
        assert!(shell_commands.is_empty());
    }

    #[test]
    fn test_parse_markdown_no_code_blocks() {
        let text = "This is plain text with no code blocks.";
        let (file_changes, shell_commands) = parse_markdown(text);
        assert!(file_changes.is_empty());
        assert!(shell_commands.is_empty());
    }

    #[test]
    fn test_parse_markdown_sh_language() {
        let text = "```sh\necho hello\n```";
        let (_, shell_commands) = parse_markdown(text);
        assert_eq!(shell_commands.len(), 1);
        assert_eq!(shell_commands[0].command, "echo hello");
    }

    #[test]
    fn test_parse_markdown_shell_language() {
        let text = "```shell\npwd\n```";
        let (_, shell_commands) = parse_markdown(text);
        assert_eq!(shell_commands.len(), 1);
        assert_eq!(shell_commands[0].command, "pwd");
    }

    #[test]
    fn test_parse_markdown_file_content_preserved() {
        let text = "```js src/index.js\nconsole.log('hello');\n```";
        let (file_changes, _) = parse_markdown(text);
        assert_eq!(file_changes.len(), 1);
        assert_eq!(file_changes[0].content, "console.log('hello');");
    }

    #[test]
    fn test_parse_markdown_multiple_shell_commands() {
        let text = "```bash\nnpm install\n```\n```bash\nnpm test\n```";
        let (_, shell_commands) = parse_markdown(text);
        assert_eq!(shell_commands.len(), 2);
        assert_eq!(shell_commands[0].command, "npm install");
        assert_eq!(shell_commands[1].command, "npm test");
    }

    #[test]
    fn test_parse_markdown_file_lang_field() {
        let text = "```python src/main.py\nprint('hi')\n```";
        let (file_changes, _) = parse_markdown(text);
        assert_eq!(file_changes[0].lang, Some("python".to_string()));
    }

    #[test]
    fn test_parse_markdown_preceding_path_label() {
        let text = "path: config/settings.json\n```json\n{\"key\": \"value\"}\n```";
        let (file_changes, _) = parse_markdown(text);
        assert_eq!(file_changes.len(), 1);
        assert_eq!(file_changes[0].path, "config/settings.json");
    }
}
