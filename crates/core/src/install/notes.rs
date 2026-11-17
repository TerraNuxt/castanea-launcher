//! Release notes: markdown to plain text for the "What's new" popover.

/// The notes as plain lines: headings and emphasis lose their marks, list items get a bullet, links keep their text.
pub fn plain(markdown: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut fenced = false;
    for raw in markdown.lines() {
        let line = raw.trim_end();
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            out.push(format!("  {line}"));
            continue;
        }
        let trimmed = line.trim_start();
        let text = if let Some(rest) = trimmed.strip_prefix(['-', '*', '+']).filter(|r| r.starts_with(' ')) {
            format!("• {}", inline(rest.trim()))
        } else {
            inline(trimmed.trim_start_matches('#').trim_start_matches('>').trim())
        };
        if text.is_empty() && out.last().is_none_or(String::is_empty) {
            continue;
        }
        out.push(text);
    }
    while out.last().is_some_and(String::is_empty) {
        out.pop();
    }
    out.join("\n")
}

/// Removes emphasis and code marks, turns `[text](url)` into `text`, drops images and HTML tags.
fn inline(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' if i > 0 && chars[i - 1] == ' ' && chars.get(i + 1) == Some(&' ') => out.push('*'),
            '*' | '`' => {}
            '~' if chars.get(i + 1) == Some(&'~') => {}
            '!' if chars.get(i + 1) == Some(&'[') => {
                if let Some(end) = skip_link(&chars, i + 1) {
                    i = end;
                    continue;
                }
                out.push('!');
            }
            '[' => match link(&chars, i) {
                Some((label, end)) => {
                    out.push_str(&inline(&label));
                    i = end;
                    continue;
                }
                None => out.push('['),
            },
            '<' => {
                let tag = chars.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic() || *c == '/');
                match chars[i..].iter().position(|c| *c == '>') {
                    Some(end) if tag => {
                        i += end + 1;
                        continue;
                    }
                    _ => out.push('<'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    out.trim().to_owned()
}

/// `[label](target)` starting at `start`: the label and the index after it.
fn link(chars: &[char], start: usize) -> Option<(String, usize)> {
    let close = chars[start..].iter().position(|c| *c == ']')? + start;
    (chars.get(close + 1) == Some(&'(')).then_some(())?;
    let end = chars[close..].iter().position(|c| *c == ')')? + close + 1;
    Some((chars[start + 1..close].iter().collect(), end))
}

fn skip_link(chars: &[char], start: usize) -> Option<usize> {
    link(chars, start).map(|(_, end)| end)
}

#[cfg(test)]
mod tests {
    use super::plain;

    #[test]
    fn markdown_becomes_plain_text() {
        let md = "## What's new\n\n- **Faster** world loading\n- Fixes [issue 12](https://x.y/12)\n\n```\ncode here\n```\n\n> Thanks to `all` of you!\n\n\n";
        assert_eq!(
            plain(md),
            "What's new\n\n• Faster world loading\n• Fixes issue 12\n\n  code here\n\nThanks to all of you!"
        );
    }

    #[test]
    fn images_tags_and_lone_marks_are_handled() {
        assert_eq!(plain("![logo](a.png) text <b>bold</b> 2 * 3 and snake_case"), "text bold 2 * 3 and snake_case");
        assert_eq!(plain(""), "");
    }
}
