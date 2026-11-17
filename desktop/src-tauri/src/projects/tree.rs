use std::collections::VecDeque;
use std::fs;
use std::path::Path;

const TREE_EXCLUDED_DIRS: [&str; 10] = [
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".maestri",
    ".vokia",
    ".venv",
    "__pycache__",
    ".next",
];
pub(crate) const TREE_MAX_DEPTH: usize = 3;
const TREE_MAX_ENTRIES: usize = 150;
const TREE_MAX_CHARS: usize = 4000;
const TREE_SCAN_LIMIT: usize = 2000;

#[derive(Debug)]
pub(crate) struct FolderTree {
    pub entries: Vec<String>,
    pub omitted: usize,
    pub omitted_is_lower_bound: bool,
}

pub(crate) fn folder_tree(dir: &Path) -> std::io::Result<FolderTree> {
    let mut walk = TreeWalk::default();
    let mut queue = VecDeque::new();
    walk.list_children(&[], read_children(dir)?, &mut queue);
    while let Some(components) = queue.pop_front() {
        if walk.scan_exhausted {
            break;
        }
        let path = components
            .iter()
            .fold(dir.to_path_buf(), |path, name| path.join(name));
        if let Ok(children) = read_children(&path) {
            walk.list_children(&components, children, &mut queue);
        }
    }
    Ok(walk.finish())
}

struct Child {
    name: String,
    is_dir: bool,
}

fn read_children(dir: &Path) -> std::io::Result<Vec<Child>> {
    let mut children = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let is_dir = entry.file_type()?.is_dir();
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_dir && TREE_EXCLUDED_DIRS.contains(&name.as_str()) {
            continue;
        }
        children.push(Child { name, is_dir });
    }
    children.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(children)
}

#[derive(Default)]
struct TreeWalk {
    listed: Vec<(Vec<String>, String)>,
    chars: usize,
    read: usize,
    omitted: usize,
    scan_exhausted: bool,
}

impl TreeWalk {
    fn list_children(
        &mut self,
        prefix: &[String],
        children: Vec<Child>,
        queue: &mut VecDeque<Vec<String>>,
    ) {
        for Child { name, is_dir } in children {
            if self.read == TREE_SCAN_LIMIT {
                self.scan_exhausted = true;
                return;
            }
            self.read += 1;
            let mut components = prefix.to_vec();
            components.push(name);
            let line = tree_line(&components, is_dir);
            let line_chars = line.chars().count() + 1;
            if self.listed.len() >= TREE_MAX_ENTRIES || self.chars + line_chars > TREE_MAX_CHARS {
                self.omitted += 1;
                continue;
            }
            self.chars += line_chars;
            if is_dir && components.len() < TREE_MAX_DEPTH {
                queue.push_back(components.clone());
            }
            self.listed.push((components, line));
        }
    }

    fn finish(mut self) -> FolderTree {
        self.listed.sort_by(|a, b| a.0.cmp(&b.0));
        FolderTree {
            entries: self.listed.into_iter().map(|(_, line)| line).collect(),
            omitted: self.omitted,
            omitted_is_lower_bound: self.scan_exhausted,
        }
    }
}

fn tree_line(components: &[String], is_dir: bool) -> String {
    let mut line = components.join("/");
    if is_dir {
        line.push('/');
    }
    line
}

#[cfg(test)]
#[path = "../../tests/tree_tests.rs"]
mod folder_tree_tests;
