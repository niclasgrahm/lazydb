use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::db::SchemaNode;

pub fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("~/.cache"))
        .join("lazydb")
        .join("schemas")
}

pub fn cache_path(profile_key: &str) -> PathBuf {
    cache_path_in(&cache_dir(), profile_key)
}

fn cache_path_in(dir: &Path, profile_key: &str) -> PathBuf {
    dir.join(format!("{}.json", sanitize(profile_key)))
}

fn sanitize(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn load(profile_key: &str) -> Option<Vec<SchemaNode>> {
    load_in(&cache_dir(), profile_key)
}

pub fn save(profile_key: &str, schema: &[SchemaNode]) -> io::Result<()> {
    save_in(&cache_dir(), profile_key, schema)
}

pub fn delete(profile_key: &str) -> io::Result<()> {
    delete_in(&cache_dir(), profile_key)
}

fn load_in(dir: &Path, profile_key: &str) -> Option<Vec<SchemaNode>> {
    let path = cache_path_in(dir, profile_key);
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_in(dir: &Path, profile_key: &str, schema: &[SchemaNode]) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = cache_path_in(dir, profile_key);
    let json = serde_json::to_string(schema)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(&path, json)?;
    Ok(())
}

fn delete_in(dir: &Path, profile_key: &str) -> io::Result<()> {
    let path = cache_path_in(dir, profile_key);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let mut path = env::temp_dir();
            path.push(format!(
                "lazydb-cache-test-{}-{}-{:?}",
                label,
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sample_tree() -> Vec<SchemaNode> {
        vec![SchemaNode::group(
            "PROD",
            vec![SchemaNode::group(
                "PUBLIC",
                vec![
                    SchemaNode::group("Tables", vec![SchemaNode::leaf("users")]),
                    SchemaNode::group("Views", vec![]),
                ],
            )],
        )]
    }

    #[test]
    fn sanitize_replaces_unsafe_chars() {
        assert_eq!(sanitize("simple"), "simple");
        assert_eq!(sanitize("a/b"), "a_b");
        assert_eq!(sanitize("a:b c"), "a_b_c");
        assert_eq!(sanitize("ok-name_1.0"), "ok-name_1.0");
    }

    #[test]
    fn load_missing_returns_none() {
        let tmp = TempDir::new("missing");
        assert!(load_in(&tmp.0, "never-saved").is_none());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let tmp = TempDir::new("roundtrip");
        let tree = sample_tree();
        save_in(&tmp.0, "myprofile", &tree).expect("save failed");
        let loaded = load_in(&tmp.0, "myprofile").expect("expected cached tree");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].label, "PROD");
        assert_eq!(loaded[0].children[0].label, "PUBLIC");
        assert_eq!(loaded[0].children[0].children[0].label, "Tables");
        assert_eq!(loaded[0].children[0].children[0].children[0].label, "users");
    }

    #[test]
    fn save_creates_dir_if_missing() {
        let tmp = TempDir::new("createdir");
        let nested = tmp.0.join("nested").join("deep");
        save_in(&nested, "p", &sample_tree()).unwrap();
        assert!(load_in(&nested, "p").is_some());
    }

    #[test]
    fn load_corrupted_returns_none() {
        let tmp = TempDir::new("corrupt");
        fs::write(cache_path_in(&tmp.0, "corrupt"), "{{ not valid json").unwrap();
        assert!(load_in(&tmp.0, "corrupt").is_none());
    }

    #[test]
    fn delete_removes_file() {
        let tmp = TempDir::new("delete");
        save_in(&tmp.0, "gone", &sample_tree()).unwrap();
        assert!(load_in(&tmp.0, "gone").is_some());
        delete_in(&tmp.0, "gone").unwrap();
        assert!(load_in(&tmp.0, "gone").is_none());
    }

    #[test]
    fn delete_missing_is_ok() {
        let tmp = TempDir::new("delete-missing");
        delete_in(&tmp.0, "never-existed").expect("delete should be idempotent");
    }
}
