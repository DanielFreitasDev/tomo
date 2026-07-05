//! Inheritance resolution: collection → folder chain → request.

use indexmap::IndexMap;

use crate::model::{Auth, CollectionFile, FolderFile, Pair, RequestFile, Scripts};

pub struct Chain<'a> {
    pub collection: &'a CollectionFile,
    /// Outer → inner (collection root down to the request's folder).
    pub folders: Vec<&'a FolderFile>,
    pub request: &'a RequestFile,
}

pub struct ResolvedInputs {
    /// Merged headers (enabled only), outer→inner, case-insensitive last-wins.
    pub headers: Vec<Pair>,
    /// First non-inherit auth walking request → folders (inner→outer) → collection.
    pub auth: Auth,
    /// Script sources in execution order: collection, folders outer→inner, request.
    pub scripts_chain: Vec<Scripts>,
}

pub fn resolve_chain(chain: &Chain<'_>) -> ResolvedInputs {
    // headers: keyed by lowercase name; later levels replace earlier ones
    let mut merged: IndexMap<String, Pair> = IndexMap::new();
    let mut absorb = |pairs: &[Pair]| {
        for p in pairs {
            if p.enabled {
                merged.insert(p.name.to_ascii_lowercase(), p.clone());
            }
        }
    };
    absorb(&chain.collection.defaults.headers);
    for folder in &chain.folders {
        absorb(&folder.defaults.headers);
    }
    absorb(&chain.request.http.headers);

    // auth: request first, then folders inner→outer, then collection
    let mut auth = Auth::Inherit;
    let mut candidates: Vec<Option<&Auth>> = vec![chain.request.auth.as_ref()];
    for folder in chain.folders.iter().rev() {
        candidates.push(folder.auth.as_ref());
    }
    candidates.push(chain.collection.auth.as_ref());
    for candidate in candidates.into_iter().flatten() {
        if !matches!(candidate, Auth::Inherit) {
            auth = candidate.clone();
            break;
        }
    }
    if matches!(auth, Auth::Inherit) {
        auth = Auth::None;
    }

    let mut scripts_chain = vec![chain.collection.scripts.clone()];
    for folder in &chain.folders {
        scripts_chain.push(folder.scripts.clone());
    }
    scripts_chain.push(chain.request.scripts.clone());

    ResolvedInputs {
        headers: merged.into_values().collect(),
        auth,
        scripts_chain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CollectionFile, Defaults, FolderFile};

    fn collection_with(headers: Vec<Pair>, auth: Option<Auth>) -> CollectionFile {
        let mut c = CollectionFile::new("c");
        c.defaults = Defaults { headers };
        c.auth = auth;
        c
    }

    #[test]
    fn headers_merge_case_insensitive_inner_wins() {
        let collection = collection_with(
            vec![Pair::new("User-Agent", "tomo"), Pair::new("X-Env", "col")],
            None,
        );
        let mut folder = FolderFile::new("f");
        folder.defaults.headers = vec![Pair::new("x-env", "folder"), Pair::disabled("X-Off", "1")];
        let mut req = RequestFile::default();
        req.http.headers = vec![Pair::new("X-ENV", "request")];

        let out = resolve_chain(&Chain {
            collection: &collection,
            folders: vec![&folder],
            request: &req,
        });

        let names: Vec<(&str, &str)> = out
            .headers
            .iter()
            .map(|p| (p.name.as_str(), p.value.as_str()))
            .collect();
        assert_eq!(names, vec![("User-Agent", "tomo"), ("X-ENV", "request")]);
    }

    #[test]
    fn auth_walks_up_until_non_inherit() {
        let collection = collection_with(
            vec![],
            Some(Auth::Bearer {
                token: "col".into(),
            }),
        );
        let folder = FolderFile::new("f"); // no auth -> inherit
        let mut req = RequestFile {
            auth: None, // inherit
            ..Default::default()
        };

        let out = resolve_chain(&Chain {
            collection: &collection,
            folders: vec![&folder],
            request: &req,
        });
        assert_eq!(
            out.auth,
            Auth::Bearer {
                token: "col".into()
            }
        );

        // request opts out explicitly
        req.auth = Some(Auth::None);
        let out = resolve_chain(&Chain {
            collection: &collection,
            folders: vec![&folder],
            request: &req,
        });
        assert_eq!(out.auth, Auth::None);

        // folder-level auth beats collection
        let mut folder2 = FolderFile::new("f2");
        folder2.auth = Some(Auth::Basic {
            username: "u".into(),
            password: "p".into(),
        });
        req.auth = None;
        let out = resolve_chain(&Chain {
            collection: &collection,
            folders: vec![&folder2],
            request: &req,
        });
        assert!(matches!(out.auth, Auth::Basic { .. }));
    }
}
