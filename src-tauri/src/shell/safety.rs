use crate::error::AppError;
use std::path::{Component, Path};

const FORBIDDEN_PATTERNS: &[&str] = &[";", "&&", "||", "|", "`", "$(", "${", "\n", "\r"];

const PROTECTED_ROOTS: &[&str] = &[
    "/bin",
    "/sbin",
    "/usr",
    "/System",
    "/Library",
    "/etc",
    "C:\\Windows",
    "C:\\Program Files",
];

pub fn validate_path(path: &str) -> Result<(), AppError> {
    if path.is_empty() {
        return Err(AppError::General("path is empty".to_string()));
    }

    for pattern in FORBIDDEN_PATTERNS {
        if path.contains(pattern) {
            return Err(AppError::General(format!(
                "path contains forbidden pattern: {pattern}"
            )));
        }
    }

    let p = Path::new(path);
    for component in p.components() {
        if let Component::Normal(s) = component {
            let s = s.to_string_lossy();
            if s == ".." {
                return Err(AppError::General(
                    "path traversal (.. component) not allowed".to_string(),
                ));
            }
        }
    }

    Ok(())
}

pub fn is_protected_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    PROTECTED_ROOTS.iter().any(|root| {
        let root_normalized = root.replace('\\', "/");
        normalized == root_normalized || normalized.starts_with(&format!("{root_normalized}/"))
    })
}

pub fn validate_not_protected(path: &str) -> Result<(), AppError> {
    if is_protected_path(path) {
        return Err(AppError::General(format!(
            "operation on protected path not allowed: {path}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_paths() {
        assert!(validate_path("/Users/test/file.txt").is_ok());
        assert!(validate_path("/tmp/folder").is_ok());
        assert!(validate_path("/home/user/docs/report.pdf").is_ok());
    }

    #[test]
    fn test_empty_path_rejected() {
        assert!(validate_path("").is_err());
    }

    #[test]
    fn test_injection_patterns_rejected() {
        assert!(validate_path("/tmp/file; rm -rf /").is_err());
        assert!(validate_path("/tmp/$(whoami)").is_err());
        assert!(validate_path("/tmp/file && cat /etc/passwd").is_err());
        assert!(validate_path("/tmp/file | grep secret").is_err());
        assert!(validate_path("/tmp/`id`").is_err());
        assert!(validate_path("/tmp/file\n/etc/passwd").is_err());
    }

    #[test]
    fn test_protected_paths() {
        assert!(is_protected_path("/bin"));
        assert!(is_protected_path("/usr"));
        assert!(is_protected_path("/System"));
        assert!(is_protected_path("/usr/local/bin"));
        assert!(!is_protected_path("/Users/test"));
        assert!(!is_protected_path("/tmp"));
    }

    #[test]
    fn test_validate_not_protected() {
        assert!(validate_not_protected("/Users/test").is_ok());
        assert!(validate_not_protected("/bin").is_err());
        assert!(validate_not_protected("/System/Library").is_err());
    }
}
