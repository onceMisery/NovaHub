#![forbid(unsafe_code)]

use novahub_platform_api::{
    ApplicationInfo, Capability, CapabilityRegistry, ClipboardPayload, FileSearchResult,
    HttpResponse, PlatformError, PlatformServices, SystemCommand,
};
#[cfg(target_os = "macos")]
use std::{
    io::Read,
    path::PathBuf,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default)]
pub struct MacosAdapter {
    capabilities: CapabilityRegistry,
    allow_directory_fallback: bool,
}

#[cfg(target_os = "macos")]
const PLATFORM_SEARCH_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(target_os = "macos")]
const LAUNCH_SERVICES_REGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";

impl MacosAdapter {
    #[must_use]
    pub fn probe() -> Self {
        #[cfg(target_os = "macos")]
        {
            let mut capabilities = CapabilityRegistry::new();
            capabilities.set_status(
                novahub_platform_api::Capability::ApplicationLaunch,
                novahub_platform_api::CapabilityStatus::Available,
            );
            capabilities.set_status(
                novahub_platform_api::Capability::FileOpen,
                novahub_platform_api::CapabilityStatus::Available,
            );
            capabilities.set_status(
                novahub_platform_api::Capability::Http,
                novahub_platform_api::CapabilityStatus::Available,
            );
            capabilities.set_status(
                novahub_platform_api::Capability::FileSearch,
                novahub_platform_api::CapabilityStatus::Available,
            );
            capabilities.set_status(
                novahub_platform_api::Capability::Clipboard,
                novahub_platform_api::CapabilityStatus::Available,
            );
            capabilities.set_status(
                novahub_platform_api::Capability::CredentialStore,
                novahub_platform_api::CapabilityStatus::Available,
            );
            capabilities.set_status(
                novahub_platform_api::Capability::SystemCommand,
                novahub_platform_api::CapabilityStatus::Available,
            );
            capabilities.set_status(
                novahub_platform_api::Capability::GlobalHotkey,
                novahub_platform_api::CapabilityStatus::Available,
            );
            return Self {
                capabilities,
                allow_directory_fallback: false,
            };
        }
        #[cfg(not(target_os = "macos"))]
        {
            Self {
                capabilities: CapabilityRegistry::new(),
                allow_directory_fallback: false,
            }
        }
    }

    /// Enables an explicit directory fallback for diagnostics when Spotlight
    /// is unavailable. Normal application discovery remains index-only.
    #[must_use]
    pub const fn with_directory_fallback(mut self, enabled: bool) -> Self {
        self.allow_directory_fallback = enabled;
        self
    }

    #[must_use]
    pub const fn capabilities(&self) -> &CapabilityRegistry {
        &self.capabilities
    }
}

impl PlatformServices for MacosAdapter {
    fn open_path(&self, path: &std::path::Path) -> Result<(), PlatformError> {
        let value = path.to_str().ok_or(PlatformError::InvalidInput)?;
        if value.trim().is_empty() || value.contains(['\r', '\n']) {
            return Err(PlatformError::InvalidInput);
        }
        self.capabilities
            .require(Capability::FileOpen)
            .map_err(|error| PlatformError::Unavailable(error.capability))?;
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(value)
                .spawn()
                .map(|_| ())
                .map_err(|error| PlatformError::Io(error.to_string()))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = value;
            Err(PlatformError::Unavailable(Capability::FileOpen))
        }
    }

    fn launch_application(&self, path: &std::path::Path) -> Result<(), PlatformError> {
        self.open_path(path)
    }

    fn reveal_path(&self, path: &std::path::Path) -> Result<(), PlatformError> {
        let value = path.to_str().ok_or(PlatformError::InvalidInput)?;
        if value.trim().is_empty() || value.contains(['\r', '\n']) {
            return Err(PlatformError::InvalidInput);
        }
        self.capabilities
            .require(Capability::FileOpen)
            .map_err(|error| PlatformError::Unavailable(error.capability))?;
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .args(["-R", value])
                .spawn()
                .map(|_| ())
                .map_err(|error| PlatformError::Io(error.to_string()))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = value;
            Err(PlatformError::Unavailable(Capability::FileOpen))
        }
    }

    fn execute_system_command(&self, command: SystemCommand) -> Result<(), PlatformError> {
        self.capabilities
            .require(Capability::SystemCommand)
            .map_err(|error| PlatformError::Unavailable(error.capability))?;
        #[cfg(target_os = "macos")]
        let (program, args): (&str, &[&str]) = match command {
            SystemCommand::OpenSettings => ("open", &["x-apple.systempreferences:"]),
            SystemCommand::Lock => ("pmset", &["displaysleepnow"]),
            SystemCommand::Sleep => ("pmset", &["sleepnow"]),
            SystemCommand::Logout => (
                "osascript",
                &["-e", "tell application \"System Events\" to log out"],
            ),
            SystemCommand::Restart => (
                "osascript",
                &["-e", "tell application \"System Events\" to restart"],
            ),
        };
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new(program)
                .args(args)
                .spawn()
                .map(|_| ())
                .map_err(|error| PlatformError::Io(error.to_string()))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = command;
            Err(PlatformError::Unavailable(Capability::SystemCommand))
        }
    }

    fn search_files(
        &self,
        root: &std::path::Path,
        query: &str,
        limit: usize,
    ) -> Result<Vec<FileSearchResult>, PlatformError> {
        let root = root.to_str().ok_or(PlatformError::InvalidInput)?;
        if root.trim().is_empty() || query.trim().is_empty() || query.contains(['\r', '\n']) {
            return Err(PlatformError::InvalidInput);
        }
        self.capabilities
            .require(Capability::FileSearch)
            .map_err(|error| PlatformError::Unavailable(error.capability))?;
        #[cfg(target_os = "macos")]
        {
            let mut command = Command::new("mdfind");
            command.args(["-onlyin", root, query]);
            let output = output_with_deadline(command, PLATFORM_SEARCH_TIMEOUT)?;
            return Ok(String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| !line.trim().is_empty())
                .take(limit.min(50))
                .map(|line| FileSearchResult {
                    path: std::path::PathBuf::from(line.trim()),
                })
                .collect());
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (root, query, limit);
            Err(PlatformError::Unavailable(Capability::FileSearch))
        }
    }

    fn discover_applications(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ApplicationInfo>, PlatformError> {
        if query.contains(['\r', '\n']) {
            return Err(PlatformError::InvalidInput);
        }
        self.capabilities
            .require(Capability::ApplicationLaunch)
            .map_err(|error| PlatformError::Unavailable(error.capability))?;
        #[cfg(target_os = "macos")]
        {
            let query = query.to_ascii_lowercase();
            let mut results = Vec::new();
            let index_available = collect_indexed_applications(&query, &mut results, limit.min(50));
            if !index_available && !self.allow_directory_fallback {
                return Err(PlatformError::Unavailable(Capability::ApplicationLaunch));
            }
            if self.allow_directory_fallback {
                for root in application_roots() {
                    if results.len() >= limit.min(50) {
                        break;
                    }
                    collect_applications(&root, &query, &mut results, limit.min(50));
                }
            }
            results.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
            results.truncate(limit.min(50));
            Ok(results)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = limit;
            Err(PlatformError::Unavailable(Capability::ApplicationLaunch))
        }
    }

    fn read_clipboard(&self) -> Result<ClipboardPayload, PlatformError> {
        self.capabilities
            .require(Capability::Clipboard)
            .map_err(|error| PlatformError::Unavailable(error.capability))?;
        #[cfg(target_os = "macos")]
        {
            let mut clipboard =
                arboard::Clipboard::new().map_err(|error| PlatformError::Io(error.to_string()))?;
            if let Ok(text) = clipboard.get_text() {
                return Ok(ClipboardPayload::Text(text));
            }
            if let Ok(image) = clipboard.get_image() {
                return Ok(ClipboardPayload::Image(image.bytes.into_owned()));
            }
            Err(PlatformError::Io(
                "clipboard has no supported payload".into(),
            ))
        }
        #[cfg(not(target_os = "macos"))]
        Err(PlatformError::Unavailable(Capability::Clipboard))
    }

    fn pick_text_file(&self, max_bytes: usize) -> Result<Option<Vec<u8>>, PlatformError> {
        if max_bytes == 0 {
            return Err(PlatformError::InvalidInput);
        }
        self.capabilities
            .require(Capability::FileOpen)
            .map_err(|error| PlatformError::Unavailable(error.capability))?;
        #[cfg(target_os = "macos")]
        {
            let Some(path) = rfd::FileDialog::new().pick_file() else {
                return Ok(None);
            };
            let mut file =
                std::fs::File::open(path).map_err(|error| PlatformError::Io(error.to_string()))?;
            let mut bytes = Vec::new();
            file.by_ref()
                .take(
                    u64::try_from(max_bytes)
                        .unwrap_or(u64::MAX)
                        .saturating_add(1),
                )
                .read_to_end(&mut bytes)
                .map_err(|error| PlatformError::Io(error.to_string()))?;
            if bytes.len() > max_bytes || std::str::from_utf8(&bytes).is_err() {
                return Err(PlatformError::InvalidInput);
            }
            Ok(Some(bytes))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = max_bytes;
            Err(PlatformError::Unavailable(Capability::FileOpen))
        }
    }

    fn http_get_once(
        &self,
        url: &str,
        max_bytes: usize,
        timeout: std::time::Duration,
    ) -> Result<HttpResponse, PlatformError> {
        if max_bytes == 0 || timeout.is_zero() {
            return Err(PlatformError::InvalidInput);
        }
        let parsed = url::Url::parse(url).map_err(|_| PlatformError::InvalidInput)?;
        if parsed.scheme() != "https"
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err(PlatformError::InvalidInput);
        }
        self.capabilities
            .require(Capability::Http)
            .map_err(|error| PlatformError::Unavailable(error.capability))?;
        #[cfg(target_os = "macos")]
        {
            let agent = ureq::Agent::config_builder()
                .https_only(true)
                .max_redirects(0)
                .max_redirects_will_error(false)
                .http_status_as_error(false)
                .timeout_global(Some(timeout))
                .timeout_connect(Some(timeout.min(std::time::Duration::from_secs(3))))
                .timeout_recv_body(Some(timeout.min(std::time::Duration::from_secs(5))))
                .build()
                .new_agent();
            let mut response = agent
                .get(url)
                .call()
                .map_err(|error| PlatformError::Io(error.to_string()))?;
            let location = response
                .headers()
                .get("location")
                .map(|value| value.to_str().map(str::to_owned))
                .transpose()
                .map_err(|_| PlatformError::InvalidInput)?;
            let body = response
                .body_mut()
                .with_config()
                .limit(
                    u64::try_from(max_bytes)
                        .unwrap_or(u64::MAX)
                        .saturating_add(1),
                )
                .read_to_vec()
                .map_err(|error| match error {
                    ureq::Error::BodyExceedsLimit(_) => PlatformError::LimitExceeded,
                    ureq::Error::Timeout(_) => PlatformError::Timeout,
                    error => PlatformError::Io(error.to_string()),
                })?;
            Ok(HttpResponse {
                status: response.status().as_u16(),
                location,
                body,
            })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (url, max_bytes, timeout);
            Err(PlatformError::Unavailable(Capability::Http))
        }
    }

    fn write_clipboard_text(&self, text: &str) -> Result<(), PlatformError> {
        if text.is_empty() || text.len() > 1024 * 1024 || text.chars().any(char::is_control) {
            return Err(PlatformError::InvalidInput);
        }
        self.capabilities
            .require(Capability::Clipboard)
            .map_err(|error| PlatformError::Unavailable(error.capability))?;
        #[cfg(target_os = "macos")]
        {
            let mut clipboard =
                arboard::Clipboard::new().map_err(|error| PlatformError::Io(error.to_string()))?;
            clipboard
                .set_text(text)
                .map_err(|error| PlatformError::Io(error.to_string()))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = text;
            Err(PlatformError::Unavailable(Capability::Clipboard))
        }
    }

    fn credential_key(&self, namespace: &str) -> Result<[u8; 32], PlatformError> {
        if namespace.trim().is_empty() || namespace.contains(['\r', '\n']) {
            return Err(PlatformError::InvalidInput);
        }
        self.capabilities
            .require(Capability::CredentialStore)
            .map_err(|error| PlatformError::Unavailable(error.capability))?;
        #[cfg(target_os = "macos")]
        {
            let entry = keyring::Entry::new("novahub", namespace)
                .map_err(|error| PlatformError::Io(error.to_string()))?;
            match entry.get_secret() {
                Ok(secret) if secret.len() == 32 => {
                    let mut key = [0_u8; 32];
                    key.copy_from_slice(&secret);
                    Ok(key)
                }
                Ok(_) => Err(PlatformError::Io(
                    "credential has invalid key length".into(),
                )),
                Err(keyring::Error::NoEntry) => {
                    let mut key = [0_u8; 32];
                    getrandom::fill(&mut key)
                        .map_err(|error| PlatformError::Io(error.to_string()))?;
                    entry
                        .set_secret(&key)
                        .map_err(|error| PlatformError::Io(error.to_string()))?;
                    Ok(key)
                }
                Err(error) => Err(PlatformError::Io(error.to_string())),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = namespace;
            Err(PlatformError::Unavailable(Capability::CredentialStore))
        }
    }
}

#[cfg(target_os = "macos")]
fn application_roots() -> Vec<PathBuf> {
    [
        Some(PathBuf::from("/Applications")),
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| path.join("Applications")),
    ]
    .into_iter()
    .flatten()
    .collect()
}

#[cfg(target_os = "macos")]
fn collect_applications(
    root: &std::path::Path,
    query: &str,
    results: &mut Vec<ApplicationInfo>,
    limit: usize,
) {
    if results.len() >= limit {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        if results.len() >= limit {
            break;
        }
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("app") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if query.is_empty() || name.to_ascii_lowercase().contains(query) {
            results.push(ApplicationInfo {
                name: name.to_owned(),
                launch_path: path,
            });
        }
    }
}

#[cfg(target_os = "macos")]
fn collect_indexed_applications(
    query: &str,
    results: &mut Vec<ApplicationInfo>,
    limit: usize,
) -> bool {
    if collect_launch_services_applications(query, results, limit) {
        return true;
    }
    let mut command = Command::new("mdfind");
    command.args([
        "-onlyin",
        "/Applications",
        "kMDItemContentType == 'com.apple.application-bundle'",
    ]);
    let output = output_with_deadline(command, PLATFORM_SEARCH_TIMEOUT);
    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if results.len() >= limit {
            break;
        }
        let path = PathBuf::from(line.trim());
        if path.extension().and_then(|value| value.to_str()) != Some("app") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !name.is_empty() && (query.is_empty() || name.to_ascii_lowercase().contains(query)) {
            results.push(ApplicationInfo {
                name: name.to_owned(),
                launch_path: path,
            });
        }
    }
    true
}

#[cfg(target_os = "macos")]
fn collect_launch_services_applications(
    query: &str,
    results: &mut Vec<ApplicationInfo>,
    limit: usize,
) -> bool {
    let mut command = Command::new(LAUNCH_SERVICES_REGISTER);
    command.args([
        "-dump", "-domain", "local", "-domain", "system", "-domain", "user",
    ]);
    let output = match output_with_deadline(command, PLATFORM_SEARCH_TIMEOUT) {
        Ok(output) if output.status.success() => output,
        Ok(_) | Err(_) => return false,
    };
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if results.len() >= limit {
            break;
        }
        let Some(path) = line.trim_start().strip_prefix("path: ") else {
            continue;
        };
        let path = PathBuf::from(path.trim());
        if path.extension().and_then(|value| value.to_str()) != Some("app") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !name.is_empty() && (query.is_empty() || name.to_ascii_lowercase().contains(query)) {
            results.push(ApplicationInfo {
                name: name.to_owned(),
                launch_path: path,
            });
        }
    }
    true
}

#[cfg(target_os = "macos")]
fn output_with_deadline(mut command: Command, timeout: Duration) -> Result<Output, PlatformError> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| PlatformError::Io(error.to_string()))?;
    let deadline = Instant::now() + timeout;
    loop {
        match child
            .try_wait()
            .map_err(|error| PlatformError::Io(error.to_string()))?
        {
            Some(_) => {
                return child
                    .wait_with_output()
                    .map_err(|error| PlatformError::Io(error.to_string()));
            }
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(PlatformError::Io(
                    "platform search command exceeded its deadline".into(),
                ));
            }
            None => thread::sleep(Duration::from_millis(10)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MacosAdapter;
    use novahub_platform_api::{Capability, CapabilityStatus, PlatformServices};

    #[test]
    fn probe_keeps_macos_capability_logic_outside_shared_domain() {
        let adapter = MacosAdapter::probe();
        assert!(!adapter.allow_directory_fallback);
        #[cfg(target_os = "macos")]
        assert_eq!(
            adapter.capabilities().status(Capability::FileOpen),
            CapabilityStatus::Available
        );
        #[cfg(not(target_os = "macos"))]
        assert_eq!(
            adapter.capabilities().status(Capability::FileOpen),
            CapabilityStatus::Unavailable
        );
        #[cfg(target_os = "macos")]
        assert_eq!(
            adapter.capabilities().status(Capability::Clipboard),
            CapabilityStatus::Available
        );
        #[cfg(not(target_os = "macos"))]
        assert_eq!(
            adapter.capabilities().status(Capability::Clipboard),
            CapabilityStatus::Unavailable
        );
    }

    #[test]
    fn open_path_rejects_control_characters_before_spawning_a_process() {
        let adapter = MacosAdapter::probe();
        assert_eq!(
            adapter.open_path(std::path::Path::new("bad\npath")),
            Err(novahub_platform_api::PlatformError::InvalidInput)
        );
    }

    #[test]
    fn file_search_rejects_control_characters_before_spawning_mdfind() {
        let adapter = MacosAdapter::probe();
        assert_eq!(
            adapter.search_files(std::path::Path::new("."), "bad\nquery", 10),
            Err(novahub_platform_api::PlatformError::InvalidInput)
        );
    }

    #[test]
    fn http_rejects_insecure_credentials_and_empty_limits_before_network_io() {
        let adapter = MacosAdapter::probe();
        for url in [
            "http://example.test/resource",
            "https://user:password@example.test/resource",
        ] {
            assert_eq!(
                adapter.http_get_once(url, 1024, std::time::Duration::from_secs(1)),
                Err(novahub_platform_api::PlatformError::InvalidInput)
            );
        }
        assert_eq!(
            adapter.http_get_once(
                "https://example.test/resource",
                0,
                std::time::Duration::from_secs(1)
            ),
            Err(novahub_platform_api::PlatformError::InvalidInput)
        );
        assert_eq!(
            adapter.http_get_once(
                "https://example.test/resource",
                1024,
                std::time::Duration::ZERO
            ),
            Err(novahub_platform_api::PlatformError::InvalidInput)
        );
    }
}
