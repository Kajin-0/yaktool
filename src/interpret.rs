use crate::{
    error::{Error, Result},
    intent::{Action, Age, Filters, Intent, Location},
};
use regex::Regex;

pub trait Interpreter {
    fn interpret(&self, input: &str) -> Result<Intent>;
}
pub struct RuleInterpreter;

pub fn parse_size(number: &str, unit: &str) -> Result<u64> {
    let multiplier = match unit.to_ascii_lowercase().as_str() {
        "kb" => 1_000,
        "mb" => 1_000_000,
        "gb" => 1_000_000_000,
        "kib" => 1_024,
        "mib" => 1_048_576,
        "gib" => 1_073_741_824,
        _ => return Err(Error::new("UNSUPPORTED_REQUEST", "Unknown size unit")),
    };
    number
        .parse::<u64>()
        .ok()
        .and_then(|n| n.checked_mul(multiplier))
        .ok_or_else(|| Error::new("UNSUPPORTED_REQUEST", "Invalid or overflowing size"))
}
fn regex(s: &str) -> Result<Regex> {
    Regex::new(s).map_err(|e| Error::new("INTERNAL_ERROR", e.to_string()))
}
fn filters(s: &str) -> Result<Option<Filters>> {
    let re = regex(
        r"^(files|pdfs?|pdf files|png|png files|jpegs?|jpeg files|jpg|jpg files|text|text files)(?: (older than [0-9]+ days|newer than [0-9]+ days|modified today|modified this week|larger than [0-9]+ (?:kb|mb|gb|kib|mib|gib)))?$",
    )?;
    let Some(c) = re.captures(s) else {
        return Ok(None);
    };
    let extensions = if c[1].starts_with("pdf") {
        vec![".pdf"]
    } else if c[1].starts_with("png") {
        vec![".png"]
    } else if c[1].starts_with("jp") {
        vec![".jpg", ".jpeg"]
    } else if c[1].starts_with("text") {
        vec![".txt"]
    } else {
        vec![]
    };
    let mut f = Filters {
        extensions: extensions.into_iter().map(String::from).collect(),
        ..Filters::default()
    };
    if let Some(m) = c.get(2) {
        let words: Vec<_> = m.as_str().split_whitespace().collect();
        match words[0] {
            "older" | "newer" => {
                let n = words[2]
                    .parse::<u32>()
                    .map_err(|_| Error::new("UNSUPPORTED_REQUEST", "Invalid day count"))?;
                f.age = Some(if words[0] == "older" {
                    Age::OlderThan(n)
                } else {
                    Age::NewerThan(n)
                });
            }
            "modified" => {
                f.age = Some(if words[1] == "today" {
                    Age::Today
                } else {
                    Age::ThisWeek
                })
            }
            "larger" => f.min_size_bytes = Some(parse_size(words[2], words[3])?),
            _ => return Ok(None),
        }
    }
    Ok(Some(f))
}

/// Parse the deliberately closed, non-recursive count vocabulary.
pub fn parse_count_query(input: &str) -> Result<Option<Intent>> {
    let normalized = input.trim().trim_end_matches(['?', '.']).split_whitespace().collect::<Vec<_>>().join(" ").to_ascii_lowercase();
    let alias = r"(home|desktop|documents|downloads|pictures|archive)";
    for (kind, action) in [("directories|folders", Action::CountDirectories), ("files", Action::CountFiles)] {
        let pattern = format!(r"^(?:how many (?:{kind})(?: are)? in {alias}|count (?:{kind}) in {alias})$");
        if let Some(c) = regex(&pattern)?.captures(&normalized) {
            let mut intent = Intent::new(action);
            let location = c.iter().skip(1).flatten().next().ok_or_else(|| Error::new("INTERNAL_ERROR", "Count location missing"))?;
            intent.source = Some(Location::DirectoryAlias(location.as_str().into()));
            return Ok(Some(intent));
        }
    }
    Ok(None)
}

/// Parse the complete, closed-vocabulary move frame without touching the filesystem.
/// Returns `None` whenever the request is ambiguous or contains unrepresentable
/// semantics; callers must then preserve the existing fail-closed behavior.
pub fn parse_move_frame(input: &str) -> Result<Option<Intent>> {
    let s = input.trim().trim_end_matches('.').to_ascii_lowercase();
    if regex(r"\b(copy|duplicate|sync|backup|delete|erase|remove|rename|compress|upload|download|install|execute|run|chmod|sudo|overwrite|touch)\b")?.is_match(&s)
        || regex(r"\b(?:don't|do not|never)\s+move\b|\b(?:except|excluding|but not)\b")?.is_match(&s)
        || regex(r"\b(?:and|then)\b")?.is_match(&s)
    {
        return Ok(None);
    }
    let verbs = regex(r"\b(move|relocate|put)\b")?.find_iter(&s).count();
    if verbs != 1 {
        return Ok(None);
    }
    let loc = r"(home|desktop|documents|downloads|pictures|archive)";
    let relation = regex(&format!(r"\bfrom\s+{loc}\s+(?:over\s+to|to|into|in)\s+{loc}\b"))?;
    let Some(c) = relation.captures(&s) else { return Ok(None) };
    if c[1] == c[2] { return Ok(None); }
    if regex(&format!(r"\b{loc}\b"))?.find_iter(&s).count() != 2 { return Ok(None); }

    let mut category = None;
    for (name, pattern) in [("pdf", r"\bpdfs?(?:\s+files?)?\b"), ("png", r"\bpngs?(?:\s+files?)?\b"), ("jpeg", r"\b(?:jpegs?|jpgs?)(?:\s+files?)?\b"), ("text", r"\btext(?:\s+files?)?\b")] {
        if regex(pattern)?.is_match(&s) {
            if category.replace(name).is_some() { return Ok(None); }
        }
    }
    let age_re = regex(r"\b(older|newer)\s+than\s+([0-9]+)\s+days?\b")?;
    let age_count = age_re.find_iter(&s).count()
        + regex(r"\bmodified\s+(?:today|this\s+week)\b")?.find_iter(&s).count();
    if age_count > 1 { return Ok(None); }
    let age = if let Some(a) = age_re.captures(&s) {
        let n = a[2].parse::<u32>().map_err(|_| Error::new("UNSUPPORTED_REQUEST", "Invalid day count"))?;
        if n == 0 { return Ok(None); }
        Some(if &a[1] == "older" { Age::OlderThan(n) } else { Age::NewerThan(n) })
    } else if s.contains("modified today") { Some(Age::Today) }
    else if s.contains("modified this week") { Some(Age::ThisWeek) } else { None };
    let size_re = regex(r"\blarger\s+than\s+([0-9]+)\s+(kb|mb|gb|kib|mib|gib)\b")?;
    if size_re.find_iter(&s).count() > 1 { return Ok(None); }
    let size = if let Some(z) = size_re.captures(&s) {
        Some(parse_size(&z[1], &z[2])?)
    } else { None };

    let mut i = Intent::new(Action::Move);
    i.source = Some(Location::DirectoryAlias(c[1].into()));
    i.destination = Some(Location::DirectoryAlias(c[2].into()));
    i.filters = Filters { extensions: match category { Some("pdf") => vec![".pdf".into()], Some("png") => vec![".png".into()], Some("jpeg") => vec![".jpg".into(), ".jpeg".into()], Some("text") => vec![".txt".into()], _ => vec![] }, age, min_size_bytes: size };
    Ok(Some(i))
}
impl Interpreter for RuleInterpreter {
    fn interpret(&self, input: &str) -> Result<Intent> {
        if input.len() > 4096 {
            return Err(Error::new("UNSUPPORTED_REQUEST", "Request too long"));
        }
        let normalized = input
            .trim()
            .trim_end_matches('.')
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase();
        let alias = r"(home|desktop|documents|downloads|pictures|archive)";
        if let Some(c) = regex(&format!(
            r"^(?:show|list|show me|show me the files in) {alias}$"
        ))?
        .captures(&normalized)
        {
            let mut i = Intent::new(Action::List);
            i.source = Some(Location::DirectoryAlias(c[1].into()));
            return Ok(i);
        }
        for (pattern, action) in [
            (format!(r"^find (.+) in {alias}$"), Action::Search),
            (
                format!(r"^move (.+) from {alias} to {alias}$"),
                Action::Move,
            ),
        ] {
            if let Some(c) = regex(&pattern)?.captures(&normalized) {
                if let Some(f) = filters(&c[1])? {
                    let mut i =
                        Intent::new(if action == Action::Search && f.min_size_bytes.is_some() {
                            Action::FindLarge
                        } else {
                            action
                        });
                    i.source = Some(Location::DirectoryAlias(c[2].into()));
                    if i.action == Action::Move {
                        i.destination = Some(Location::DirectoryAlias(c[3].into()));
                    }
                    i.filters = f;
                    return Ok(i);
                }
            }
        }
        if let Some(i) = parse_count_query(&normalized)? {
            return Ok(i);
        }
        if let Some(i) = parse_move_frame(&normalized)? {
            return Ok(i);
        }
        let mut i = Intent::new(if normalized.starts_with("move ") {
            Action::Clarify
        } else {
            Action::Unsupported
        });
        i.message = Some(
            "Use a supported file category and explicit directory aliases; see --help.".into(),
        );
        Ok(i)
    }
}
