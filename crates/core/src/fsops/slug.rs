//! Display names → safe, portable file names.

use std::collections::HashSet;

const MAX_LEN: usize = 80;

/// Windows reserved device names (case-insensitive, extension-independent).
const WINDOWS_RESERVED: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Names Tomo itself uses inside collections.
const TOMO_RESERVED: &[&str] = &["collection", "folder", "secrets", "environments"];

/// Lowercased, ASCII-folded, `[a-z0-9-]` slug. Never empty, never reserved.
pub fn slugify(name: &str) -> String {
    let ascii = deunicode::deunicode(name);
    let mut out = String::with_capacity(ascii.len());
    let mut prev_dash = true; // trims leading dashes
    for c in ascii.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.len() > MAX_LEN {
        out.truncate(MAX_LEN);
        while out.ends_with('-') {
            out.pop();
        }
    }
    if out.is_empty() {
        out = "request".to_string();
    }
    if WINDOWS_RESERVED.contains(&out.as_str()) || TOMO_RESERVED.contains(&out.as_str()) {
        out = format!("req-{out}");
    }
    out
}

/// Resolve collisions against `taken` (case-insensitively — Windows/macOS
/// filesystems fold case) by appending `-2`, `-3`, …
pub fn unique_slug(name: &str, taken: &HashSet<String>) -> String {
    let taken_folded: HashSet<String> = taken.iter().map(|s| s.to_ascii_lowercase()).collect();
    let base = slugify(name);
    if !taken_folded.contains(&base) {
        return base;
    }
    for n in 2.. {
        let candidate = format!("{base}-{n}");
        if !taken_folded.contains(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_portuguese_and_unicode() {
        assert_eq!(slugify("Ação de Usuário"), "acao-de-usuario");
        assert_eq!(slugify("Criar pedido — teste"), "criar-pedido-teste");
        assert_eq!(slugify("日本語テスト"), "ri-ben-yu-tesuto");
    }

    #[test]
    fn strips_symbols_and_collapses_dashes() {
        assert_eq!(slugify("GET /users?id={{id}}"), "get-users-id-id");
        assert_eq!(slugify("--weird--   name--"), "weird-name");
    }

    #[test]
    fn guards_reserved_names() {
        assert_eq!(slugify("CON"), "req-con");
        assert_eq!(slugify("com1"), "req-com1");
        assert_eq!(slugify("Collection"), "req-collection");
        assert_eq!(slugify("folder"), "req-folder");
        assert_eq!(slugify("secrets"), "req-secrets");
    }

    #[test]
    fn never_empty_and_bounded() {
        assert_eq!(slugify("!!!"), "request");
        assert_eq!(slugify(""), "request");
        let long = "x".repeat(300);
        assert!(slugify(&long).len() <= 80);
    }

    #[test]
    fn collisions_get_numeric_suffixes_case_insensitively() {
        let mut taken = HashSet::new();
        assert_eq!(unique_slug("Users", &taken), "users");
        taken.insert("users".into());
        assert_eq!(unique_slug("Users", &taken), "users-2");
        taken.insert("USERS-2".into()); // case-folded collision
        assert_eq!(unique_slug("users", &taken), "users-3");
    }
}
