pub fn normalize(path: &str) -> String {
    let mut normalized = path.replace('\\', "/");
    while normalized.ends_with('/') && normalized.len() > 1 {
        normalized.pop();
    }
    normalized
}

pub fn is_within_scope(path: &str, root: &str) -> bool {
    let path = normalize(path);
    let root = normalize(root);

    if path == root {
        return true;
    }

    if root == "/" {
        return path.starts_with('/');
    }

    if cfg!(windows) {
        let path_lower = path.to_ascii_lowercase();
        let root_lower = root.to_ascii_lowercase();
        return path_lower.starts_with(&(root_lower + "/"));
    }

    path.starts_with(&(root + "/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_trailing_slashes() {
        assert_eq!(normalize("/foo/bar/"), "/foo/bar");
        assert_eq!(normalize("/foo/bar///"), "/foo/bar");
        assert_eq!(normalize("/"), "/");
    }

    #[test]
    fn normalize_converts_backslashes() {
        assert_eq!(normalize("C:\\Users\\test"), "C:/Users/test");
    }

    #[test]
    fn within_scope_exact_match() {
        assert!(is_within_scope("/foo/bar", "/foo/bar"));
        assert!(is_within_scope("/foo/bar/", "/foo/bar"));
    }

    #[test]
    fn within_scope_child_path() {
        assert!(is_within_scope("/foo/bar/baz", "/foo/bar"));
        assert!(!is_within_scope("/foo/barbaz", "/foo/bar"));
    }

    #[test]
    fn within_scope_root() {
        assert!(is_within_scope("/anything", "/"));
        assert!(!is_within_scope("/anything", "/other"));
    }

    #[test]
    fn not_within_scope_sibling() {
        assert!(!is_within_scope("/foo/other", "/foo/bar"));
    }
}
