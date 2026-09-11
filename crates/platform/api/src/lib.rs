#![forbid(unsafe_code)]

use std::collections::BTreeSet;

/// Host-owned capability identifiers shared by platform adapters.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Capability {
    ApplicationLaunch,
    Clipboard,
    CredentialStore,
    FileSearch,
    FileOpen,
    Http,
    GlobalHotkey,
    SystemCommand,
    WindowManagement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemCommand {
    OpenSettings,
    Lock,
    Sleep,
    Logout,
    Restart,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityStatus {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityError {
    pub capability: Capability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlatformError {
    Unavailable(Capability),
    InvalidInput,
    LimitExceeded,
    Timeout,
    Io(String),
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(capability) => {
                write!(formatter, "capability unavailable: {capability:?}")
            }
            Self::InvalidInput => formatter.write_str("invalid platform input"),
            Self::LimitExceeded => formatter.write_str("platform response limit exceeded"),
            Self::Timeout => formatter.write_str("platform operation exceeded its deadline"),
            Self::Io(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for PlatformError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationInfo {
    pub name: String,
    pub launch_path: std::path::PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardPayload {
    Text(String),
    Image(Vec<u8>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileSearchResult {
    pub path: std::path::PathBuf,
}

/// One bounded HTTP response returned by a host-owned platform adapter.
/// Redirects are deliberately returned to the App broker instead of being
/// followed inside the adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpResponse {
    pub status: u16,
    pub location: Option<String>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    #[must_use]
    pub fn ok(body: Vec<u8>) -> Self {
        Self {
            status: 200,
            location: None,
            body,
        }
    }

    #[must_use]
    pub fn redirect(location: impl Into<String>) -> Self {
        Self {
            status: 302,
            location: Some(location.into()),
            body: Vec::new(),
        }
    }
}

/// Small host-owned platform surface. Implementations must not expose raw
/// paths or OS handles to plugins; the host validates and invokes them.
pub trait PlatformServices {
    /// Opens a host-approved path using the platform's default handler.
    ///
    /// # Errors
    ///
    /// Returns `InvalidInput` for unsafe paths, `Unavailable` when the
    /// capability is not present, or `Io` when the OS launch fails.
    fn open_path(&self, path: &std::path::Path) -> Result<(), PlatformError>;

    /// Opens the directory containing a host-approved path.
    ///
    /// Adapters may select the file when their native file manager supports
    /// it; the default keeps the action useful by opening the parent folder.
    ///
    /// # Errors
    ///
    /// Returns `InvalidInput` when the path has no parent, `Unavailable` when
    /// file opening is not available, or `Io` when the OS launch fails.
    fn reveal_path(&self, path: &std::path::Path) -> Result<(), PlatformError> {
        let parent = path.parent().ok_or(PlatformError::InvalidInput)?;
        self.open_path(parent)
    }

    /// Copies bounded text into the host clipboard.
    ///
    /// # Errors
    ///
    /// Returns `InvalidInput` for control characters or oversized text,
    /// `Unavailable` when clipboard access is not available, or `Io` when
    /// the platform clipboard rejects the write.
    fn write_clipboard_text(&self, text: &str) -> Result<(), PlatformError> {
        if text.is_empty() || text.len() > 1024 * 1024 || text.chars().any(char::is_control) {
            return Err(PlatformError::InvalidInput);
        }
        Err(PlatformError::Unavailable(Capability::Clipboard))
    }

    /// Launches a host-approved application path.
    ///
    /// # Errors
    ///
    /// Returns `Unavailable` until the platform adapter has a real launcher.
    fn launch_application(&self, path: &std::path::Path) -> Result<(), PlatformError> {
        let _ = path;
        Err(PlatformError::Unavailable(Capability::ApplicationLaunch))
    }

    /// Executes one explicit, host-owned system command.
    ///
    /// # Errors
    ///
    /// Returns `Unavailable` when the platform does not implement the command.
    fn execute_system_command(&self, command: SystemCommand) -> Result<(), PlatformError> {
        let _ = command;
        Err(PlatformError::Unavailable(Capability::SystemCommand))
    }

    /// Searches an explicit root through the platform index/search utility.
    ///
    /// # Errors
    ///
    /// Returns `Unavailable` when the platform index is not available.
    fn search_files(
        &self,
        root: &std::path::Path,
        query: &str,
        limit: usize,
    ) -> Result<Vec<FileSearchResult>, PlatformError> {
        let _ = (root, query, limit);
        Err(PlatformError::Unavailable(Capability::FileSearch))
    }

    /// Returns a bounded application snapshot for host search.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError::Unavailable`] when application discovery is
    /// not implemented by the active platform adapter.
    fn discover_applications(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ApplicationInfo>, PlatformError> {
        let _ = (query, limit);
        Err(PlatformError::Unavailable(Capability::ApplicationLaunch))
    }

    /// Reads the current clipboard payload through the platform owner.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError::Unavailable`] when clipboard access is not
    /// implemented by the active platform adapter.
    fn read_clipboard(&self) -> Result<ClipboardPayload, PlatformError> {
        Err(PlatformError::Unavailable(Capability::Clipboard))
    }

    /// Opens the native file picker and returns one bounded UTF-8 snapshot.
    ///
    /// The selected path and native handle remain inside the adapter. A
    /// `None` result means the user cancelled the picker.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError::Unavailable`] when the native picker is not
    /// available, [`PlatformError::InvalidInput`] for an invalid size, or
    /// [`PlatformError::Io`] when the selected file cannot be read.
    fn pick_text_file(&self, max_bytes: usize) -> Result<Option<Vec<u8>>, PlatformError> {
        if max_bytes == 0 {
            return Err(PlatformError::InvalidInput);
        }
        Err(PlatformError::Unavailable(Capability::FileOpen))
    }

    /// Performs one bounded HTTPS GET without following redirects.
    ///
    /// The App broker owns redirect policy and calls this method again after
    /// authorizing each returned `Location`.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError::Unavailable`] when HTTP is not implemented,
    /// [`PlatformError::InvalidInput`] for invalid limits, or [`PlatformError::Io`]
    /// for transport failures.
    fn http_get_once(
        &self,
        url: &str,
        max_bytes: usize,
        timeout: std::time::Duration,
    ) -> Result<HttpResponse, PlatformError> {
        if url.trim().is_empty() || max_bytes == 0 || timeout.is_zero() {
            return Err(PlatformError::InvalidInput);
        }
        let _ = (url, max_bytes, timeout);
        Err(PlatformError::Unavailable(Capability::Http))
    }

    /// Returns a per-installation key from the platform credential store.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError::Unavailable`] when the active platform adapter
    /// has no credential-store implementation.
    fn credential_key(&self, namespace: &str) -> Result<[u8; 32], PlatformError> {
        let _ = namespace;
        Err(PlatformError::Unavailable(Capability::CredentialStore))
    }
}

/// Small capability table; unavailable operations cannot report success.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CapabilityRegistry {
    available: BTreeSet<Capability>,
}

impl CapabilityRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_status(&mut self, capability: Capability, status: CapabilityStatus) {
        match status {
            CapabilityStatus::Available => {
                self.available.insert(capability);
            }
            CapabilityStatus::Unavailable => {
                self.available.remove(&capability);
            }
        }
    }

    #[must_use]
    pub fn status(&self, capability: Capability) -> CapabilityStatus {
        if self.available.contains(&capability) {
            CapabilityStatus::Available
        } else {
            CapabilityStatus::Unavailable
        }
    }

    /// Requires an explicitly available host capability.
    ///
    /// # Errors
    ///
    /// Returns the unavailable capability instead of performing a no-op.
    pub fn require(&self, capability: Capability) -> Result<(), CapabilityError> {
        (self.status(capability) == CapabilityStatus::Available)
            .then_some(())
            .ok_or(CapabilityError { capability })
    }
}

#[cfg(test)]
mod tests {
    use super::{Capability, CapabilityRegistry, CapabilityStatus};

    #[test]
    fn unsupported_capabilities_are_explicitly_unavailable() {
        let registry = CapabilityRegistry::new();
        assert_eq!(
            registry.status(Capability::Clipboard),
            CapabilityStatus::Unavailable
        );
        assert!(registry.require(Capability::Clipboard).is_err());
    }

    #[test]
    fn adapter_can_publish_supported_capability() {
        let mut registry = CapabilityRegistry::new();
        registry.set_status(Capability::Clipboard, CapabilityStatus::Available);
        assert!(registry.require(Capability::Clipboard).is_ok());
    }
}
