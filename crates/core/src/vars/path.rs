//! Dot/index path parsing and JSON walking, shared by interpolation and
//! declarative asserts: `user.address[0].street`.

use crate::model::VarValue;

pub enum Seg<'a> {
    Key(&'a str),
    Index(usize),
}

/// Split `user.address[0].street` into ("user", [Key, Index, Key]).
pub fn split_path(token: &str) -> (&str, Vec<Seg<'_>>) {
    let mut segs = Vec::new();
    let mut root_end = token.len();
    for (i, c) in token.char_indices() {
        if c == '.' || c == '[' {
            root_end = i;
            break;
        }
    }
    let root = &token[..root_end];
    let mut rest = &token[root_end..];

    while !rest.is_empty() {
        if let Some(stripped) = rest.strip_prefix('.') {
            let end = stripped.find(['.', '[']).unwrap_or(stripped.len());
            segs.push(Seg::Key(&stripped[..end]));
            rest = &stripped[end..];
        } else if let Some(stripped) = rest.strip_prefix('[') {
            match stripped.find(']') {
                Some(close) => {
                    match stripped[..close].trim().parse::<usize>() {
                        Ok(n) => segs.push(Seg::Index(n)),
                        Err(_) => segs.push(Seg::Key(stripped[..close].trim())),
                    }
                    rest = &stripped[close + 1..];
                }
                None => break,
            }
        } else {
            break;
        }
    }
    (root, segs)
}

pub fn walk_path<'v>(value: &'v VarValue, path: &[Seg<'_>]) -> Option<&'v VarValue> {
    let mut cur = value;
    for seg in path {
        cur = match seg {
            Seg::Key(k) => cur.as_object()?.get(*k)?,
            Seg::Index(n) => cur.as_array()?.get(*n)?,
        };
    }
    Some(cur)
}
