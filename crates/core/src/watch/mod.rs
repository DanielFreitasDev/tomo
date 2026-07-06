//! Collection file watching: notify + debouncer → semantic events, with
//! self-write suppression so app saves don't echo back as external changes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, new_debouncer};
use serde::Serialize;

use crate::CoreError;

const DEBOUNCE: Duration = Duration::from_millis(400);
const SUPPRESS_TTL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WatchEvent {
    /// Files/folders were created, removed or renamed — rescan the tree.
    TreeChanged,
    /// A tracked file's content changed externally.
    FileChanged { rel: String, hash: String },
}

/// Remembers hashes of recent app-side writes so the watcher can tell an echo
/// of our own save from a genuine external edit.
#[derive(Debug, Default)]
pub struct WriteSuppressor {
    recent: Mutex<HashMap<PathBuf, (String, Instant)>>,
}

impl WriteSuppressor {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Call right after an app-side atomic write.
    pub fn register(&self, path: &Path, content: &str) {
        let hash = content_hash(content.as_bytes());
        self.recent
            .lock()
            .expect("suppressor lock")
            .insert(path.to_path_buf(), (hash, Instant::now()));
    }

    pub fn should_suppress(&self, path: &Path, hash: &str) -> bool {
        let mut map = self.recent.lock().expect("suppressor lock");
        map.retain(|_, (_, at)| at.elapsed() < SUPPRESS_TTL);
        map.get(path).is_some_and(|(h, _)| h == hash)
    }
}

pub fn content_hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// True if `path` currently holds content matching a recently-registered
/// app-side write — i.e. an atomic save landing as a create/rename that the
/// watcher must not echo back as an external change.
fn is_self_write(suppressor: &WriteSuppressor, path: &Path) -> bool {
    match std::fs::read(path) {
        Ok(bytes) => suppressor.should_suppress(path, &content_hash(&bytes)),
        Err(_) => false,
    }
}

/// Keep this alive for as long as the collection is open; dropping stops the watch.
pub struct Watcher {
    _debouncer: notify_debouncer_full::Debouncer<
        notify::RecommendedWatcher,
        notify_debouncer_full::RecommendedCache,
    >,
}

pub fn watch_collection(
    root: PathBuf,
    suppressor: Arc<WriteSuppressor>,
    on_event: impl Fn(WatchEvent) + Send + 'static,
) -> Result<Watcher, CoreError> {
    let watch_root = root.clone();
    let mut debouncer = new_debouncer(DEBOUNCE, None, move |result: DebounceEventResult| {
        let events = match result {
            Ok(events) => events,
            Err(_) => return,
        };

        let mut tree_changed = false;
        let mut changed_files: Vec<PathBuf> = Vec::new();

        for event in events {
            use notify::EventKind;
            let relevant: Vec<&PathBuf> = event
                .paths
                .iter()
                .filter(|p| is_relevant(&watch_root, p))
                .collect();
            if relevant.is_empty() {
                continue;
            }
            match event.kind {
                EventKind::Create(_)
                | EventKind::Remove(_)
                | EventKind::Modify(notify::event::ModifyKind::Name(_)) => {
                    // A structural change. Our own atomic saves land as a rename
                    // (create/remove of the target), so a tree rescan must fire
                    // only when at least one path is NOT a suppressed self-write
                    // — otherwise every app save echoes back as an external
                    // change and forces a redundant (racy) rescan.
                    if relevant
                        .iter()
                        .any(|p| !is_self_write(&suppressor, p.as_path()))
                    {
                        tree_changed = true;
                    }
                }
                EventKind::Modify(_) => {
                    for p in relevant {
                        if p.is_file() {
                            changed_files.push(p.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        if tree_changed {
            on_event(WatchEvent::TreeChanged);
        }
        changed_files.sort();
        changed_files.dedup();
        for path in changed_files {
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let hash = content_hash(&bytes);
            if suppressor.should_suppress(&path, &hash) {
                continue;
            }
            if let Some(rel) = rel_of(&watch_root, &path) {
                on_event(WatchEvent::FileChanged { rel, hash });
            }
        }
    })
    .map_err(|e| CoreError::Invalid(format!("failed to create watcher: {e}")))?;

    debouncer
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| CoreError::Invalid(format!("failed to watch {}: {e}", root.display())))?;

    Ok(Watcher {
        _debouncer: debouncer,
    })
}

/// Only .toml files, .env and directories inside the collection matter;
/// dotfiles (.git, editor temp files) and our own temp writes are ignored.
fn is_relevant(root: &Path, path: &Path) -> bool {
    let Some(rel) = rel_of(root, path) else {
        return false;
    };
    if rel.is_empty() {
        return false;
    }
    // any hidden segment (.git, .idea, editor swap dirs) except the literal .env
    for seg in rel.split('/') {
        if seg.starts_with('.') && seg != ".env" {
            return false;
        }
        if seg.starts_with(".tomo-write-") {
            return false;
        }
    }
    // directories always matter (create/remove); files must be .toml or .env
    if path.is_dir() {
        return true;
    }
    rel.ends_with(".toml") || rel.ends_with(".env") || !path.exists() /* removals */
}

fn rel_of(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    Some(rel.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suppressor_matches_only_fresh_identical_content() {
        let s = WriteSuppressor::new();
        let path = Path::new("/tmp/x.toml");
        s.register(path, "hello");

        let hello = content_hash(b"hello");
        let world = content_hash(b"world");
        assert!(s.should_suppress(path, &hello));
        assert!(
            !s.should_suppress(path, &world),
            "different content is external"
        );
        assert!(
            !s.should_suppress(Path::new("/tmp/y.toml"), &hello),
            "different path is external"
        );
    }

    #[test]
    fn self_write_recognizes_own_atomic_save_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.toml");
        std::fs::write(&path, "hello = 1\n").unwrap();
        let s = WriteSuppressor::new();
        assert!(!is_self_write(&s, &path), "unregistered write is external");
        s.register(&path, "hello = 1\n");
        assert!(
            is_self_write(&s, &path),
            "registered identical content is self"
        );
        std::fs::write(&path, "hello = 2\n").unwrap();
        assert!(
            !is_self_write(&s, &path),
            "content changed after register is external"
        );
    }

    #[test]
    fn relevance_filter() {
        let root = Path::new("/c");
        assert!(is_relevant(root, Path::new("/c/users/create.toml")));
        assert!(is_relevant(root, Path::new("/c/.env")));
        assert!(!is_relevant(root, Path::new("/c/.git/HEAD")));
        assert!(!is_relevant(root, Path::new("/c/.tomo-write-abc.tmp")));
        assert!(!is_relevant(root, Path::new("/elsewhere/x.toml")));
        // removed .toml files (no longer exist) still count
        assert!(is_relevant(root, Path::new("/c/gone.toml")));
    }

    /// Real-FS smoke test — inherently timing-dependent, run locally with
    /// `cargo test -- --ignored`. CI watcher tests are flake farms (plan).
    #[test]
    #[ignore]
    fn watcher_smoke_external_edit_and_self_write_suppression() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::write(root.join("req.toml"), "[meta]\nname='x'\n").unwrap();

        let (tx, rx) = std::sync::mpsc::channel::<WatchEvent>();
        let suppressor = WriteSuppressor::new();
        let _watcher = watch_collection(root.clone(), suppressor.clone(), move |e| {
            let _ = tx.send(e);
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(300));

        // external edit -> FileChanged
        std::fs::write(root.join("req.toml"), "[meta]\nname='edited'\n").unwrap();
        let event = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("event arrives");
        match event {
            WatchEvent::FileChanged { rel, .. } => assert_eq!(rel, "req.toml"),
            other => panic!("expected FileChanged, got {other:?}"),
        }

        // self-write (registered) -> suppressed
        let content = "[meta]\nname='app-save'\n";
        suppressor.register(&root.join("req.toml"), content);
        std::fs::write(root.join("req.toml"), content).unwrap();
        match rx.recv_timeout(Duration::from_secs(2)) {
            Err(_) => {}
            Ok(WatchEvent::TreeChanged) => {}
            Ok(other) => panic!("self write must be suppressed, got {other:?}"),
        }
    }
}
