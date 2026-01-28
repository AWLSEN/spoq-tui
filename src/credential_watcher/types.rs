//! Shared types for credential change detection.

use std::path::PathBuf;

/// Source of a credential change event
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialSource {
    /// File-based credential (path included)
    File(PathBuf),
    /// macOS Keychain entry
    Keychain,
}

impl CredentialSource {
    /// Check if this is a file-based credential source
    pub fn is_file(&self) -> bool {
        matches!(self, Self::File(_))
    }

    /// Check if this is a Keychain-based credential source
    pub fn is_keychain(&self) -> bool {
        matches!(self, Self::Keychain)
    }

    /// Human-readable description for logging
    pub fn description(&self) -> String {
        match self {
            Self::File(path) => format!("file: {}", path.display()),
            Self::Keychain => "macOS Keychain".to_string(),
        }
    }
}

/// A detected credential change event
#[derive(Debug, Clone)]
pub struct CredentialChangeEvent {
    /// Source of the credential change
    pub source: CredentialSource,
    /// When the change was detected
    pub timestamp: std::time::Instant,
}

impl CredentialChangeEvent {
    /// Create a new file-based credential change event
    pub fn file(path: PathBuf) -> Self {
        Self {
            source: CredentialSource::File(path),
            timestamp: std::time::Instant::now(),
        }
    }

    /// Create a new Keychain credential change event
    pub fn keychain() -> Self {
        Self {
            source: CredentialSource::Keychain,
            timestamp: std::time::Instant::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credential_source_is_file() {
        let file_source = CredentialSource::File(PathBuf::from("/test/path"));
        let keychain_source = CredentialSource::Keychain;

        assert!(file_source.is_file());
        assert!(!file_source.is_keychain());
        assert!(!keychain_source.is_file());
        assert!(keychain_source.is_keychain());
    }

    #[test]
    fn test_credential_source_description() {
        let file_source = CredentialSource::File(PathBuf::from("/home/user/.claude.json"));
        let keychain_source = CredentialSource::Keychain;

        assert!(file_source.description().contains(".claude.json"));
        assert_eq!(keychain_source.description(), "macOS Keychain");
    }

    #[test]
    fn test_credential_change_event_creation() {
        let file_event = CredentialChangeEvent::file(PathBuf::from("/test"));
        assert!(file_event.source.is_file());

        let keychain_event = CredentialChangeEvent::keychain();
        assert!(keychain_event.source.is_keychain());
    }
}
