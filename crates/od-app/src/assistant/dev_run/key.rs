//! Finding the Mistral key for a hand run: `MISTRAL_API_KEY`, else the file
//! named by `OPENDRAFT_ENV_FILE`, else `~/.opendraft/test.env` (or
//! `text.env`), which lives outside the repository. In a file, a line
//! `MISTRAL_API_KEY=...` (quotes and `export` allowed); failing that, a file
//! whose only line is the key itself. The key is never printed.

use std::path::PathBuf;

pub fn find() -> Option<String> {
    if let Some(key) = std::env::var("MISTRAL_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
    {
        return Some(key);
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
    let candidates: Vec<PathBuf> = std::env::var_os("OPENDRAFT_ENV_FILE")
        .map(PathBuf::from)
        .into_iter()
        .chain(home.into_iter().flat_map(|h| {
            let dir = PathBuf::from(h).join(".opendraft");
            [dir.join("test.env"), dir.join("text.env")]
        }))
        .collect();
    candidates
        .into_iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .find_map(|text| key_in(&text))
}

pub fn key_in(text: &str) -> Option<String> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let clean = |v: &str| v.trim().trim_matches(|c| c == '"' || c == '\'').to_owned();
    for line in &lines {
        let line = line.strip_prefix("export ").unwrap_or(line);
        if let Some(value) = line.strip_prefix("MISTRAL_API_KEY=") {
            let value = clean(value);
            return (!value.is_empty()).then_some(value);
        }
    }
    match lines.as_slice() {
        [only] if !only.contains('=') && !only.contains(' ') => Some(clean(only)),
        _ => None,
    }
}

#[test]
fn a_key_is_found_in_the_forms_a_file_may_hold() {
    assert_eq!(key_in("MISTRAL_API_KEY=abc\n").as_deref(), Some("abc"));
    assert_eq!(
        key_in("# note\nexport MISTRAL_API_KEY=\"abc\"").as_deref(),
        Some("abc")
    );
    assert_eq!(
        key_in("OTHER=1\nMISTRAL_API_KEY='abc'").as_deref(),
        Some("abc")
    );
    assert_eq!(key_in("abc\n").as_deref(), Some("abc"));
}

#[test]
fn a_file_without_a_key_gives_none() {
    assert_eq!(key_in(""), None);
    assert_eq!(key_in("MISTRAL_API_KEY=\n"), None);
    assert_eq!(key_in("A=1\nB=2\n"), None);
    assert_eq!(key_in("two words\n"), None);
}
