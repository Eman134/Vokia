use super::*;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

const OVERSIZED_ENTRY_COUNT: usize = 200;
const SCAN_OVERFLOW_ENTRY_COUNT: usize = 2100;
const LONG_NAME_LEN: usize = 100;
const LONG_NAME_COUNT: usize = 100;
const EVERYONE_SID: &str = "*S-1-1-0";

static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let id = NEXT_DIR.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("vokia-folder-tree-{}-{id}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn dir(&self, relative: &str) {
        fs::create_dir_all(self.0.join(relative)).unwrap();
    }

    fn file(&self, relative: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "x").unwrap();
    }

    fn tree(&self) -> FolderTree {
        folder_tree(&self.0).unwrap()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        restore_access(&self.0);
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(windows)]
fn deny_listing(dir: &Path) {
    let status = std::process::Command::new("icacls")
        .arg(dir)
        .args(["/deny", &format!("{EVERYONE_SID}:(RD)")])
        .output()
        .unwrap()
        .status;
    assert!(status.success());
}

#[cfg(windows)]
fn restore_access(dir: &Path) {
    let _ = std::process::Command::new("icacls")
        .arg(dir)
        .args(["/remove:d", EVERYONE_SID, "/t", "/c"])
        .output();
}

#[cfg(unix)]
fn deny_listing(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o000)).unwrap();
}

#[cfg(unix)]
fn restore_access(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fn walk(dir: &Path) {
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o755));
        if let Ok(read) = fs::read_dir(dir) {
            for entry in read.flatten() {
                if entry.path().is_dir() {
                    walk(&entry.path());
                }
            }
        }
    }
    walk(dir);
}

#[test]
fn lists_three_levels_in_tree_order_with_slash_separator() {
    let root = TempDir::new();
    root.file("zz.txt");
    root.file("README.md");
    root.file("docs/roadmap.md");
    root.file("desktop/src/a.txt");
    root.dir("desktop/src/features");
    let tree = root.tree();
    assert_eq!(
        tree.entries,
        [
            "README.md",
            "desktop/",
            "desktop/src/",
            "desktop/src/a.txt",
            "desktop/src/features/",
            "docs/",
            "docs/roadmap.md",
            "zz.txt",
        ]
    );
    assert_eq!(tree.omitted, 0);
    assert!(!tree.omitted_is_lower_bound);
    assert!(tree.entries.iter().all(|line| !line.contains('\\')));
}

#[test]
fn excluded_folders_at_any_level_are_neither_listed_nor_counted() {
    let root = TempDir::new();
    root.file("keep.txt");
    for excluded in TREE_EXCLUDED_DIRS {
        root.file(&format!("{excluded}/inside.txt"));
        root.file(&format!("src/{excluded}/inside.txt"));
    }
    root.file("src/main.rs");
    let tree = root.tree();
    assert_eq!(tree.entries, ["keep.txt", "src/", "src/main.rs"]);
    assert_eq!(tree.omitted, 0);
}

#[test]
fn excluded_names_only_hide_folders_not_files() {
    let root = TempDir::new();
    root.file("target");
    root.file("build");
    assert_eq!(root.tree().entries, ["build", "target"]);
}

#[test]
fn excluded_folder_contents_do_not_consume_the_limits() {
    let root = TempDir::new();
    for index in 0..SCAN_OVERFLOW_ENTRY_COUNT {
        root.file(&format!("node_modules/pkg-{index}.js"));
    }
    root.file("app.ts");
    let tree = root.tree();
    assert_eq!(tree.entries, ["app.ts"]);
    assert_eq!(tree.omitted, 0);
    assert!(!tree.omitted_is_lower_bound);
}

#[test]
fn level_four_is_cut_and_level_three_folder_has_no_children() {
    let root = TempDir::new();
    root.file("a/b/c/d/deep.txt");
    root.file("a/b/c/side.txt");
    let tree = root.tree();
    assert_eq!(tree.entries, ["a/", "a/b/", "a/b/c/"]);
    assert_eq!(tree.omitted, 0);
}

#[test]
fn more_than_150_entries_keeps_150_and_counts_the_excess() {
    let root = TempDir::new();
    for index in 0..OVERSIZED_ENTRY_COUNT {
        root.file(&format!("f{index:04}.txt"));
    }
    let tree = root.tree();
    assert_eq!(tree.entries.len(), TREE_MAX_ENTRIES);
    assert_eq!(tree.omitted, OVERSIZED_ENTRY_COUNT - TREE_MAX_ENTRIES);
    assert!(!tree.omitted_is_lower_bound);
    assert_eq!(tree.entries[0], "f0000.txt");
    assert_eq!(tree.entries[TREE_MAX_ENTRIES - 1], "f0149.txt");
}

#[test]
fn exactly_150_entries_omits_nothing() {
    let root = TempDir::new();
    for index in 0..TREE_MAX_ENTRIES {
        root.file(&format!("f{index:04}.txt"));
    }
    let tree = root.tree();
    assert_eq!(tree.entries.len(), TREE_MAX_ENTRIES);
    assert_eq!(tree.omitted, 0);
}

#[test]
fn more_than_2000_entries_flags_a_lower_bound() {
    let root = TempDir::new();
    for index in 0..SCAN_OVERFLOW_ENTRY_COUNT {
        root.file(&format!("f{index:04}.txt"));
    }
    let tree = root.tree();
    assert_eq!(tree.entries.len(), TREE_MAX_ENTRIES);
    assert!(tree.omitted_is_lower_bound);
    assert_eq!(tree.omitted, TREE_SCAN_LIMIT - TREE_MAX_ENTRIES);
}

#[test]
fn character_cap_cuts_long_names_before_150_entries() {
    let root = TempDir::new();
    for index in 0..LONG_NAME_COUNT {
        let name = format!("{index:03}{}", "n".repeat(LONG_NAME_LEN - 3));
        root.file(&name);
    }
    let tree = root.tree();
    let line_chars = LONG_NAME_LEN + 1;
    let expected = TREE_MAX_CHARS / line_chars;
    assert_eq!(tree.entries.len(), expected);
    assert_eq!(tree.omitted, LONG_NAME_COUNT - expected);
    let total: usize = tree
        .entries
        .iter()
        .map(|line| line.chars().count() + 1)
        .sum();
    assert!(total <= TREE_MAX_CHARS);
}

#[test]
fn accented_and_spaced_names_are_kept_as_is() {
    let root = TempDir::new();
    root.file("ação com espaço/relatório final.txt");
    assert_eq!(
        root.tree().entries,
        ["ação com espaço/", "ação com espaço/relatório final.txt"]
    );
}

#[test]
fn empty_folder_has_no_entries_and_nothing_omitted() {
    let root = TempDir::new();
    let tree = root.tree();
    assert!(tree.entries.is_empty());
    assert_eq!(tree.omitted, 0);
    assert!(!tree.omitted_is_lower_bound);
}

#[test]
fn missing_root_is_an_error() {
    let root = TempDir::new();
    assert!(folder_tree(&root.0.join("nao-existe")).is_err());
}

#[test]
fn root_that_is_a_file_is_an_error() {
    let root = TempDir::new();
    root.file("arquivo.txt");
    assert!(folder_tree(&root.0.join("arquivo.txt")).is_err());
}

#[test]
fn unreadable_subfolder_is_skipped_and_the_rest_is_listed() {
    let root = TempDir::new();
    root.file("a-ok/inside.txt");
    root.file("locked/secret.txt");
    root.file("z.txt");
    deny_listing(&root.0.join("locked"));
    let tree = root.tree();
    assert_eq!(
        tree.entries,
        ["a-ok/", "a-ok/inside.txt", "locked/", "z.txt"]
    );
}

#[cfg(windows)]
fn link_dir(target: &Path, link: &Path) {
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .unwrap()
        .status;
    assert!(status.success());
}

#[cfg(unix)]
fn link_dir(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[test]
fn linked_folder_is_listed_without_slash_and_not_opened() {
    let root = TempDir::new();
    root.file("real/inside.txt");
    link_dir(&root.0.join("real"), &root.0.join("link"));
    assert_eq!(root.tree().entries, ["link", "real/", "real/inside.txt"]);
}
