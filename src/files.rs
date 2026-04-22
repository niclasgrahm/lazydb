use std::path::{Path, PathBuf};

use crate::tree::TreeNode;

/// Sentinel label used for lazily-loaded directory placeholders.
pub const SENTINEL: &str = "…";

/// Extensions considered text-like (openable in the query editor).
const TEXT_EXTENSIONS: &[&str] = &[
    "sql", "txt", "csv", "json", "toml", "yaml", "yml", "md", "py", "sh", "rs", "go", "js",
    "ts", "lua", "cfg", "ini", "xml", "html",
];

/// Check if a file is text-like based on its extension.
pub fn is_text_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| TEXT_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
}

/// Read a directory and return sorted `(name, is_dir)` pairs.
/// Directories come first, then files, both sorted alphabetically.
/// Hidden entries (starting with `.`) are skipped. Returns empty on error.
pub fn read_directory(path: &Path) -> Vec<(String, bool)> {
    let Ok(entries) = std::fs::read_dir(path) else {
        return vec![];
    };

    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        if is_dir {
            dirs.push(name);
        } else {
            files.push(name);
        }
    }

    dirs.sort();
    files.sort();

    let mut result = Vec::with_capacity(dirs.len() + files.len());
    for d in dirs {
        result.push((d, true));
    }
    for f in files {
        result.push((f, false));
    }
    result
}

/// Build the initial file tree for a root directory.
/// Directories get a sentinel child so they show the expand icon.
pub fn build_file_tree(path: &Path) -> Vec<TreeNode> {
    read_directory(path)
        .into_iter()
        .map(|(name, is_dir)| {
            if is_dir {
                TreeNode::folder(&name, vec![TreeNode::leaf(SENTINEL)])
            } else {
                TreeNode::leaf(&name)
            }
        })
        .collect()
}

/// Populate children of a directory node at `flat_index`, replacing the sentinel.
/// `file_paths` must be the current path index (parallel to flatten_all output).
pub fn populate_children(
    nodes: &mut [TreeNode],
    flat_index: usize,
    file_paths: &[PathBuf],
) {
    let dir_path = &file_paths[flat_index];
    let entries = read_directory(dir_path);
    let children: Vec<TreeNode> = entries
        .into_iter()
        .map(|(name, is_dir)| {
            if is_dir {
                TreeNode::folder(&name, vec![TreeNode::leaf(SENTINEL)])
            } else {
                TreeNode::leaf(&name)
            }
        })
        .collect();

    let mut counter = 0;
    TreeNode::walk_mut(nodes, flat_index, &mut counter, |node| {
        node.children = children.clone();
    });
}

/// Check if a node's only child is the sentinel (not yet populated).
pub fn is_sentinel(nodes: &[TreeNode], flat_index: usize) -> bool {
    fn walk(nodes: &[TreeNode], target: usize, counter: &mut usize) -> Option<bool> {
        for node in nodes {
            if *counter == target {
                return Some(
                    node.children.len() == 1 && node.children[0].label == SENTINEL,
                );
            }
            *counter += 1;
            if node.expanded {
                if let Some(result) = walk(&node.children, target, counter) {
                    return Some(result);
                }
            }
        }
        None
    }
    let mut counter = 0;
    walk(nodes, flat_index, &mut counter).unwrap_or(false)
}

/// Build a path index parallel to `TreeNode::flatten_all()` output.
/// Each entry is the full path for the node at that flat index.
pub fn build_path_index(roots: &[TreeNode], root_path: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for node in roots {
        build_path_inner(node, root_path, &mut paths);
    }
    paths
}

fn build_path_inner(node: &TreeNode, parent_path: &Path, out: &mut Vec<PathBuf>) {
    let my_path = parent_path.join(&node.label);
    out.push(my_path.clone());
    if node.expanded {
        for child in &node.children {
            build_path_inner(child, &my_path, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn is_text_file_sql() {
        assert!(is_text_file(Path::new("query.sql")));
        assert!(is_text_file(Path::new("query.SQL")));
    }

    #[test]
    fn is_text_file_common_types() {
        assert!(is_text_file(Path::new("data.json")));
        assert!(is_text_file(Path::new("config.toml")));
        assert!(is_text_file(Path::new("notes.md")));
        assert!(is_text_file(Path::new("script.py")));
    }

    #[test]
    fn is_text_file_rejects_binary() {
        assert!(!is_text_file(Path::new("image.png")));
        assert!(!is_text_file(Path::new("app.exe")));
        assert!(!is_text_file(Path::new("noext")));
    }

    #[test]
    fn read_directory_sorted_dirs_first() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("beta_dir")).unwrap();
        fs::create_dir(dir.path().join("alpha_dir")).unwrap();
        fs::write(dir.path().join("zebra.sql"), "").unwrap();
        fs::write(dir.path().join("apple.txt"), "").unwrap();

        let entries = read_directory(dir.path());
        let names: Vec<&str> = entries.iter().map(|e| e.0.as_str()).collect();
        assert_eq!(names, vec!["alpha_dir", "beta_dir", "apple.txt", "zebra.sql"]);

        assert!(entries[0].1); // alpha_dir is_dir
        assert!(entries[1].1); // beta_dir is_dir
        assert!(!entries[2].1); // apple.txt is not dir
        assert!(!entries[3].1); // zebra.sql is not dir
    }

    #[test]
    fn read_directory_skips_hidden() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".hidden"), "").unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join("visible.sql"), "").unwrap();

        let entries = read_directory(dir.path());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "visible.sql");
    }

    #[test]
    fn read_directory_nonexistent_returns_empty() {
        let entries = read_directory(Path::new("/nonexistent/path/12345"));
        assert!(entries.is_empty());
    }

    #[test]
    fn build_file_tree_creates_sentinels_for_dirs() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        fs::write(dir.path().join("file.sql"), "").unwrap();

        let tree = build_file_tree(dir.path());
        assert_eq!(tree.len(), 2);

        // Directory has sentinel child
        assert_eq!(tree[0].label, "subdir");
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(tree[0].children[0].label, SENTINEL);

        // File has no children
        assert_eq!(tree[1].label, "file.sql");
        assert!(tree[1].children.is_empty());
    }

    #[test]
    fn build_path_index_root_level() {
        let tree = vec![
            TreeNode::leaf("file1.sql"),
            TreeNode::folder("subdir", vec![TreeNode::leaf(SENTINEL)]),
        ];
        let root = Path::new("/home/user/queries");
        let paths = build_path_index(&tree, root);

        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], PathBuf::from("/home/user/queries/file1.sql"));
        assert_eq!(paths[1], PathBuf::from("/home/user/queries/subdir"));
    }

    #[test]
    fn build_path_index_with_expanded_dir() {
        let mut tree = vec![TreeNode::folder(
            "subdir",
            vec![TreeNode::leaf("inner.sql"), TreeNode::leaf("other.txt")],
        )];
        tree[0].expanded = true;

        let root = Path::new("/root");
        let paths = build_path_index(&tree, root);

        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0], PathBuf::from("/root/subdir"));
        assert_eq!(paths[1], PathBuf::from("/root/subdir/inner.sql"));
        assert_eq!(paths[2], PathBuf::from("/root/subdir/other.txt"));
    }

    #[test]
    fn populate_children_replaces_sentinel() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("subdir");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("query.sql"), "SELECT 1").unwrap();
        fs::create_dir(sub.join("nested")).unwrap();

        let mut tree = build_file_tree(dir.path());
        // Expand the subdir so flatten shows children
        tree[0].expanded = true;
        let paths = build_path_index(&tree, dir.path());

        // Before: sentinel
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(tree[0].children[0].label, SENTINEL);

        // Populate
        populate_children(&mut tree, 0, &paths);

        // After: real children (nested dir first, then query.sql)
        assert_eq!(tree[0].children.len(), 2);
        assert_eq!(tree[0].children[0].label, "nested");
        assert!(tree[0].children[0].children.len() == 1); // sentinel
        assert_eq!(tree[0].children[1].label, "query.sql");
        assert!(tree[0].children[1].children.is_empty());
    }

    #[test]
    fn is_sentinel_detects_placeholder() {
        let tree = vec![TreeNode::folder(
            "dir",
            vec![TreeNode::leaf(SENTINEL)],
        )];
        assert!(is_sentinel(&tree, 0));
    }

    #[test]
    fn is_sentinel_false_for_real_children() {
        let mut tree = vec![TreeNode::folder(
            "dir",
            vec![TreeNode::leaf("real.sql")],
        )];
        tree[0].expanded = true;
        assert!(!is_sentinel(&tree, 0));
    }

    #[test]
    fn is_sentinel_false_for_leaf() {
        let tree = vec![TreeNode::leaf("file.sql")];
        assert!(!is_sentinel(&tree, 0));
    }
}
