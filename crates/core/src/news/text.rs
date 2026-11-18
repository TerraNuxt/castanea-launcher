//! The body of a news item: plain lines with `**bold**` and `[text](https://…)` links, nothing else. Everything the
//! server sends is data; it is never interpreted as markup beyond these two forms.

/// How long a link address may be.
pub const MAX_URL: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Span {
    Text(String),
    Bold(String),
    Link { text: String, url: String },
}

impl Span {
    pub fn text(&self) -> &str {
        match self {
            Self::Text(text) | Self::Bold(text) | Self::Link { text, .. } => text,
        }
    }
}

/// One line of spans; an empty line is a gap between paragraphs.
pub type Line = Vec<Span>;

/// `url` when it is a plain `https://` address: no spaces or control characters, a host, and not absurdly long.
pub fn safe_url(url: &str) -> Option<String> {
    let url = url.trim();
    let host = url.strip_prefix("https://")?.split(['/', '?', '#']).next()?;
    let bad = |c: char| c.is_control() || c.is_whitespace() || c == '<' || c == '>' || c == '"' || c == '\\';
    let fine = !host.is_empty() && !host.contains('@') && url.len() <= MAX_URL && !url.chars().any(bad);
    fine.then(|| url.to_owned())
}

/// The lines of `body`; consecutive blank lines collapse into one gap and the ends have none.
pub fn lines(body: &str) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    for raw in body.lines() {
        let line = spans(raw.trim());
        if line.is_empty() && out.last().is_none_or(Vec::is_empty) {
            continue;
        }
        out.push(line);
    }
    while out.last().is_some_and(Vec::is_empty) {
        out.pop();
    }
    out
}

fn push_text(out: &mut Line, text: &mut String) {
    if !text.is_empty() {
        out.push(Span::Text(std::mem::take(text)));
    }
}

fn find(chars: &[char], from: usize, pattern: &[char]) -> Option<usize> {
    (from..chars.len().checked_sub(pattern.len().saturating_sub(1))?).find(|&i| chars[i..].starts_with(pattern))
}

fn spans(line: &str) -> Line {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Line::new();
    let mut text = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '*' && chars.get(i + 1) == Some(&'*') {
            if let Some(end) = find(&chars, i + 2, &['*', '*']).filter(|end| *end > i + 2) {
                push_text(&mut out, &mut text);
                out.push(Span::Bold(chars[i + 2..end].iter().collect()));
                i = end + 2;
                continue;
            }
        } else if chars[i] == '['
            && let Some((label, url, end)) = link(&chars, i)
        {
            push_text(&mut out, &mut text);
            match safe_url(&url) {
                Some(url) => out.push(Span::Link { text: label, url }),
                None => out.push(Span::Text(label)),
            }
            i = end;
            continue;
        }
        text.push(chars[i]);
        i += 1;
    }
    push_text(&mut out, &mut text);
    out
}

/// `[label](target)` at `start`: the label, the target and the index after it.
fn link(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    let mid = find(chars, start + 1, &[']', '('])?;
    let close = find(chars, mid + 2, &[')'])?;
    let label: String = chars[start + 1..mid].iter().collect();
    (!label.trim().is_empty() && !label.contains('['))
        .then(|| (label, chars[mid + 2..close].iter().collect(), close + 1))
}

/// The first `budget` characters of `lines`, ending in … when something was cut; whether it was cut.
pub fn clip(lines: &[Line], budget: usize) -> (Vec<Line>, bool) {
    let mut left = budget;
    let mut out = Vec::new();
    for (n, line) in lines.iter().enumerate() {
        if left == 0 {
            return (cut_mark(out), true);
        }
        let mut kept = Line::new();
        for span in line {
            let length = span.text().chars().count();
            if length <= left {
                left -= length;
                kept.push(span.clone());
                continue;
            }
            let head: String = span.text().chars().take(left).collect::<String>().trim_end().to_owned() + "…";
            kept.push(match span {
                Span::Text(_) => Span::Text(head),
                Span::Bold(_) => Span::Bold(head),
                Span::Link { url, .. } => Span::Link { text: head, url: url.clone() },
            });
            out.push(kept);
            return (out, true);
        }
        out.push(kept);
        left = left.saturating_sub(1);
        if left == 0 && n + 1 < lines.len() {
            return (cut_mark(out), true);
        }
    }
    (out, false)
}

/// Marks a cut that fell between lines.
fn cut_mark(mut lines: Vec<Line>) -> Vec<Line> {
    while lines.last().is_some_and(Vec::is_empty) {
        lines.pop();
    }
    if let Some(last) = lines.last_mut() {
        last.push(Span::Text("…".to_owned()));
    }
    lines
}
