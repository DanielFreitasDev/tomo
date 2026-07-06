//! Integration tests for filesystem operations on a real (temp) collection.

use std::path::Path;

use pretty_assertions::assert_eq;
use tomo_core::format::parse_request;
use tomo_core::fsops::{
    Node, create_collection, create_folder, create_request, delete_node, duplicate_request,
    move_node, read_text, rename_request, reorder_nodes, resolve_rel, scan_collection,
};

fn names(nodes: &[Node]) -> Vec<&str> {
    nodes
        .iter()
        .map(|n| match n {
            Node::Folder(f) => f.name.as_str(),
            Node::Request(r) => r.name.as_str(),
        })
        .collect()
}

#[test]
fn full_collection_lifecycle() {
    let tmp = tempfile::tempdir().unwrap();
    let root = create_collection(tmp.path(), "Acme API").unwrap();
    assert!(root.ends_with("acme-api"));
    assert!(root.join("collection.toml").exists());
    assert!(
        read_text(&root.join(".gitignore"))
            .unwrap()
            .contains("secrets.toml")
    );

    // folders + requests
    let users_rel = create_folder(&root, "", "Users").unwrap();
    assert_eq!(users_rel, "users");
    let r1 = create_request(&root, "", "Health check").unwrap();
    assert_eq!(r1, "health-check.toml");
    let r2 = create_request(&root, &users_rel, "Create user").unwrap();
    let r3 = create_request(&root, &users_rel, "List users").unwrap();

    // slug collision inside the same dir
    let dup = create_request(&root, &users_rel, "Create user").unwrap();
    assert_eq!(dup, "users/create-user-2.toml");

    let tree = scan_collection(&root).unwrap();
    assert_eq!(tree.collection.meta.name, "Acme API");
    assert!(tree.invalid.is_empty());
    // no seq yet: sorted by file name, folder + request intermixed
    assert_eq!(names(&tree.nodes), vec!["Health check", "Users"]);

    // order children explicitly: List first, then the two Creates
    reorder_nodes(
        &root,
        &[
            format!("{users_rel}/list-users.toml"),
            format!("{users_rel}/create-user.toml"),
            format!("{users_rel}/create-user-2.toml"),
        ],
    )
    .unwrap();
    let tree = scan_collection(&root).unwrap();
    let users = match &tree.nodes[..] {
        [Node::Request(_), Node::Folder(f)] => f,
        other => panic!("unexpected tree shape: {other:?}"),
    };
    assert_eq!(
        names(&users.children),
        vec![
            "List users",
            "Create user",
            "Create user (copy)".trim_end_matches(" (copy)")
        ]
    );

    // reorder is idempotent — second run must not rewrite files
    let before =
        read_text(&resolve_rel(&root, &format!("{users_rel}/list-users.toml")).unwrap()).unwrap();
    reorder_nodes(
        &root,
        &[
            format!("{users_rel}/list-users.toml"),
            format!("{users_rel}/create-user.toml"),
            format!("{users_rel}/create-user-2.toml"),
        ],
    )
    .unwrap();
    let after =
        read_text(&resolve_rel(&root, &format!("{users_rel}/list-users.toml")).unwrap()).unwrap();
    assert_eq!(before, after);

    let _ = (r2, r3);
}

#[test]
fn rename_updates_name_and_reslugs_file() {
    let tmp = tempfile::tempdir().unwrap();
    let root = create_collection(tmp.path(), "C").unwrap();
    let rel = create_request(&root, "", "Old name").unwrap();

    // hand-add a comment to prove renames go through surgical sync
    let path = resolve_rel(&root, &rel).unwrap();
    let text = read_text(&path).unwrap();
    let commented = text.replace("[http]", "# precious comment\n[http]");
    std::fs::write(&path, &commented).unwrap();

    let new_rel = rename_request(&root, &rel, "Ação de Usuário").unwrap();
    assert_eq!(new_rel, "acao-de-usuario.toml");
    assert!(!path.exists(), "old file removed");

    let new_text = read_text(&resolve_rel(&root, &new_rel).unwrap()).unwrap();
    assert!(new_text.contains("# precious comment"));
    let req = parse_request(&new_text, Path::new("x.toml")).unwrap();
    assert_eq!(req.meta.name, "Ação de Usuário");
}

#[test]
fn move_duplicate_delete() {
    let tmp = tempfile::tempdir().unwrap();
    let root = create_collection(tmp.path(), "C").unwrap();
    let folder = create_folder(&root, "", "Target").unwrap();
    let rel = create_request(&root, "", "Ping").unwrap();

    let moved = move_node(&root, &rel, &folder).unwrap();
    assert_eq!(moved, "target/ping.toml");
    assert!(resolve_rel(&root, &moved).unwrap().exists());

    let copy = duplicate_request(&root, &moved).unwrap();
    assert_eq!(copy, "target/ping-copy.toml");
    let copy_req = parse_request(
        &read_text(&resolve_rel(&root, &copy).unwrap()).unwrap(),
        Path::new("x.toml"),
    )
    .unwrap();
    assert_eq!(copy_req.meta.name, "Ping (copy)");

    delete_node(&root, &copy).unwrap();
    assert!(!resolve_rel(&root, &copy).unwrap().exists());
    delete_node(&root, &folder).unwrap();
    assert!(!resolve_rel(&root, &folder).unwrap().exists());
}

#[test]
fn moving_folder_into_itself_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let root = create_collection(tmp.path(), "C").unwrap();
    let outer = create_folder(&root, "", "Outer").unwrap();
    let inner = create_folder(&root, &outer, "Inner").unwrap();
    let err = move_node(&root, &outer, &inner).unwrap_err();
    assert!(err.to_string().contains("into itself"));
}

#[test]
fn path_traversal_is_rejected() {
    let root = Path::new("/tmp/fake-root");
    assert!(resolve_rel(root, "../evil").is_err());
    assert!(resolve_rel(root, "a/../../evil").is_err());
    assert!(resolve_rel(root, "/etc/passwd").is_err());
    assert!(resolve_rel(root, "a\\..\\evil").is_err());
    // Windows drive prefixes: PathBuf::push("C:...") REPLACES the buffer on
    // Windows, escaping the root. Rejected on every platform so CI catches it.
    assert!(resolve_rel(root, "C:/Users/evil.toml").is_err());
    assert!(resolve_rel(root, "C:evil").is_err());
    assert!(resolve_rel(root, "ok/C:evil").is_err());
    assert!(resolve_rel(root, "ok/nested.toml").is_ok());
}

#[test]
fn scan_collects_invalid_files_without_failing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = create_collection(tmp.path(), "C").unwrap();
    create_request(&root, "", "Good").unwrap();
    std::fs::write(root.join("broken.toml"), "[meta\nname=").unwrap();

    let tree = scan_collection(&root).unwrap();
    assert_eq!(names(&tree.nodes), vec!["Good"]);
    assert_eq!(tree.invalid.len(), 1);
    assert_eq!(tree.invalid[0].rel, "broken.toml");
}

#[test]
fn cannot_delete_collection_root() {
    let tmp = tempfile::tempdir().unwrap();
    let root = create_collection(tmp.path(), "C").unwrap();
    assert!(delete_node(&root, "").is_err());
    assert!(delete_node(&root, "/").is_err());
    assert!(root.exists());
}
