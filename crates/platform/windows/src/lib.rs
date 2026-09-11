#![forbid(unsafe_code)]

use novahub_platform_api::{
    ApplicationInfo, Capability, CapabilityRegistry, ClipboardPayload, FileSearchResult,
    HttpResponse, PlatformError, PlatformServices, SystemCommand,
};
#[cfg(windows)]
use std::{
    io::Read,
    path::PathBuf,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default)]
pub struct WindowsAdapter {
    capabilities: CapabilityRegistry,
    allow_recursive_file_fallback: bool,
}

#[cfg(windows)]
const PLATFORM_SEARCH_TIMEOUT: Duration = Duration::from_secs(5);

impl WindowsAdapter {
    #[must_use]
    pub fn probe() -> Self {
        let mut capabilities = CapabilityRegistry::new();
        #[cfg(windows)]
        for capability in [
            Capability::ApplicationLaunch,
            Capability::FileSearch,
            Capability::FileOpen,
            Capability::Http,
            Capability::SystemCommand,
            Capability::Clipboard,
            Capability::CredentialStore,
            Capability::GlobalHotkey,
        ] {
            capabilities.set_status(
                capability,
                novahub_platform_api::CapabilityStatus::Available,
            );
        }
        Self {
            capabilities,
            allow_recursive_file_fallback: false,
        }
    }

    /// Enables the explicitly requested, potentially expensive `where.exe`
    /// fallback for troubleshooting. Production search keeps this disabled so
    /// an unavailable Windows Search index is reported instead of hidden.
    #[must_use]
    pub const fn with_recursive_file_fallback(mut self, enabled: bool) -> Self {
        self.allow_recursive_file_fallback = enabled;
        self
    }

    #[must_use]
    pub const fn capabilities(&self) -> &CapabilityRegistry {
        &self.capabilities
    }
}

impl PlatformServices for WindowsAdapter {
    fn open_path(&self, path: &std::path::Path) -> Result<(), PlatformError> {
        let value = path.to_str().ok_or(PlatformError::InvalidInput)?;
        if value.trim().is_empty() || value.contains(['\r', '\n']) {
            return Err(PlatformError::InvalidInput);
        }
        self.capabilities
            .require(Capability::FileOpen)
            .map_err(|error| PlatformError::Unavailable(error.capability))?;
        #[cfg(windows)]
        {
            std::process::Command::new("explorer.exe")
                .arg(value)
                .spawn()
                .map(|_| ())
                .map_err(|error| PlatformError::Io(error.to_string()))
        }
        #[cfg(not(windows))]
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
        #[cfg(windows)]
        {
            std::process::Command::new("explorer.exe")
                .args(["/select,", value])
                .spawn()
                .map(|_| ())
                .map_err(|error| PlatformError::Io(error.to_string()))
        }
        #[cfg(not(windows))]
        {
            let _ = value;
            Err(PlatformError::Unavailable(Capability::FileOpen))
        }
    }

    fn execute_system_command(&self, command: SystemCommand) -> Result<(), PlatformError> {
        self.capabilities
            .require(Capability::SystemCommand)
            .map_err(|error| PlatformError::Unavailable(error.capability))?;
        #[cfg(windows)]
        let (program, args): (&str, &[&str]) = match command {
            SystemCommand::OpenSettings => ("explorer.exe", &["ms-settings:"]),
            SystemCommand::Lock => ("rundll32.exe", &["user32.dll,LockWorkStation"]),
            SystemCommand::Sleep => ("rundll32.exe", &["powrprof.dll,SetSuspendState", "0,1,0"]),
            SystemCommand::Logout => ("shutdown.exe", &["/l"]),
            SystemCommand::Restart => ("shutdown.exe", &["/r", "/t", "0"]),
        };
        #[cfg(windows)]
        {
            std::process::Command::new(program)
                .args(args)
                .spawn()
                .map(|_| ())
                .map_err(|error| PlatformError::Io(error.to_string()))
        }
        #[cfg(not(windows))]
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
        if let Some(results) = search_windows_index(std::path::Path::new(root), query, limit) {
            return results;
        }
        if !self.allow_recursive_file_fallback {
            return Err(PlatformError::Unavailable(Capability::FileSearch));
        }
        let mut command = Command::new("where.exe");
        command.args(["/r", root, &format!("*{query}*")]);
        let output = output_with_deadline(command, PLATFORM_SEARCH_TIMEOUT)?;
        if !output.status.success() {
            return Ok(Vec::new());
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .take(limit.min(50))
            .map(|line| FileSearchResult {
                path: std::path::PathBuf::from(line.trim()),
            })
            .collect())
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
        #[cfg(windows)]
        {
            let query = query.to_ascii_lowercase();
            let mut results = Vec::new();
            for root in application_roots() {
                collect_applications(&root, &query, 0, &mut results, limit.min(50));
                if results.len() >= limit.min(50) {
                    break;
                }
            }
            if results.len() < limit.min(50) {
                collect_registered_applications(&query, &mut results, limit.min(50));
            }
            results.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
            results.truncate(limit.min(50));
            Ok(results)
        }
        #[cfg(not(windows))]
        {
            let _ = limit;
            Err(PlatformError::Unavailable(Capability::ApplicationLaunch))
        }
    }

    fn read_clipboard(&self) -> Result<ClipboardPayload, PlatformError> {
        self.capabilities
            .require(Capability::Clipboard)
            .map_err(|error| PlatformError::Unavailable(error.capability))?;
        #[cfg(windows)]
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
        #[cfg(not(windows))]
        Err(PlatformError::Unavailable(Capability::Clipboard))
    }

    fn pick_text_file(&self, max_bytes: usize) -> Result<Option<Vec<u8>>, PlatformError> {
        if max_bytes == 0 {
            return Err(PlatformError::InvalidInput);
        }
        self.capabilities
            .require(Capability::FileOpen)
            .map_err(|error| PlatformError::Unavailable(error.capability))?;
        #[cfg(windows)]
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
        #[cfg(not(windows))]
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
        #[cfg(windows)]
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
        #[cfg(not(windows))]
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
        #[cfg(windows)]
        {
            let mut clipboard =
                arboard::Clipboard::new().map_err(|error| PlatformError::Io(error.to_string()))?;
            clipboard
                .set_text(text)
                .map_err(|error| PlatformError::Io(error.to_string()))
        }
        #[cfg(not(windows))]
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
        #[cfg(windows)]
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
        #[cfg(not(windows))]
        {
            let _ = namespace;
            Err(PlatformError::Unavailable(Capability::CredentialStore))
        }
    }
}

#[cfg(windows)]
fn application_roots() -> Vec<PathBuf> {
    [
        std::env::var_os("PROGRAMDATA")
            .map(PathBuf::from)
            .map(|path| path.join("Microsoft\\Windows\\Start Menu\\Programs")),
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("Microsoft\\Windows\\Start Menu\\Programs")),
    ]
    .into_iter()
    .flatten()
    .collect()
}

#[cfg(windows)]
fn collect_applications(
    root: &std::path::Path,
    query: &str,
    depth: usize,
    results: &mut Vec<ApplicationInfo>,
    limit: usize,
) {
    if depth > 4 || results.len() >= limit {
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
        if path.is_dir() {
            collect_applications(&path, query, depth + 1, results, limit);
            continue;
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !matches!(extension.to_ascii_lowercase().as_str(), "lnk" | "exe") {
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

#[cfg(windows)]
fn collect_registered_applications(query: &str, results: &mut Vec<ApplicationInfo>, limit: usize) {
    if results.len() >= limit {
        return;
    }
    let mut command = Command::new("powershell.exe");
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "Get-StartApps | ForEach-Object { \"$($_.Name)`t$($_.AppID)\" }",
    ]);
    let output = output_with_deadline(command, PLATFORM_SEARCH_TIMEOUT);
    let Ok(output) = output else {
        return;
    };
    if !output.status.success() {
        return;
    }
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if results.len() >= limit {
            break;
        }
        let Some((name, app_id)) = line.split_once('\t') else {
            continue;
        };
        if name.trim().is_empty()
            || app_id.trim().is_empty()
            || (!query.is_empty() && !name.to_ascii_lowercase().contains(query))
        {
            continue;
        }
        results.push(ApplicationInfo {
            name: name.trim().to_owned(),
            launch_path: PathBuf::from(format!("shell:AppsFolder\\{}", app_id.trim())),
        });
    }
}

#[cfg(windows)]
fn search_windows_index(
    root: &std::path::Path,
    query: &str,
    limit: usize,
) -> Option<Result<Vec<FileSearchResult>, PlatformError>> {
    let root = root.to_str()?;
    let mut command = Command::new("powershell.exe");
    command
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$root=[Environment]::GetEnvironmentVariable('NOVAHUB_SEARCH_ROOT'); $query=[Environment]::GetEnvironmentVariable('NOVAHUB_SEARCH_QUERY'); $connection=New-Object System.Data.OleDb.OleDbConnection(\"Provider=Search.CollatorDSO;Extended Properties='Application=Windows'\"); $connection.Open(); $command=$connection.CreateCommand(); $safeRoot=$root.Replace(\"'\",\"''\"); $safeQuery=$query.Replace(\"'\",\"''\"); $command.CommandText=\"SELECT System.ItemPathDisplay FROM SYSTEMINDEX WHERE System.ItemPathDisplay LIKE '%$safeQuery%' AND System.ItemPathDisplay LIKE '$safeRoot%'\"; $reader=$command.ExecuteReader(); while($reader.Read()){ $reader.GetValue(0) }; $reader.Close(); $connection.Close();",
        ])
        .env("NOVAHUB_SEARCH_ROOT", root)
        .env("NOVAHUB_SEARCH_QUERY", query);
    let output = match output_with_deadline(command, PLATFORM_SEARCH_TIMEOUT) {
        Ok(output) => output,
        Err(error) => return Some(Err(error)),
    };
    if !output.status.success() {
        return None;
    }
    let results = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(limit.min(50))
        .map(|line| FileSearchResult {
            path: std::path::PathBuf::from(line.trim()),
        })
        .collect::<Vec<_>>();
    (!results.is_empty()).then_some(Ok(results))
}

#[cfg(windows)]
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

#[cfg(not(windows))]
fn search_windows_index(
    _root: &std::path::Path,
    _query: &str,
    _limit: usize,
) -> Option<Result<Vec<FileSearchResult>, PlatformError>> {
    None
}

#[cfg(test)]
mod tests {
    use super::WindowsAdapter;
    use novahub_platform_api::{Capability, CapabilityStatus, PlatformServices};

    #[test]
    fn probe_uses_shared_capability_matrix() {
        let adapter = WindowsAdapter::probe();
        assert!(!adapter.allow_recursive_file_fallback);
        #[cfg(windows)]
        assert_eq!(
            adapter.capabilities().status(Capability::FileOpen),
            CapabilityStatus::Available
        );
        #[cfg(not(windows))]
        assert_eq!(
            adapter.capabilities().status(Capability::FileOpen),
            CapabilityStatus::Unavailable
        );
        #[cfg(windows)]
        assert_eq!(
            adapter.capabilities().status(Capability::Clipboard),
            CapabilityStatus::Available
        );
        #[cfg(not(windows))]
        assert_eq!(
            adapter.capabilities().status(Capability::Clipboard),
            CapabilityStatus::Unavailable
        );
    }

    #[test]
    fn open_path_rejects_control_characters_before_spawning_a_process() {
        let adapter = WindowsAdapter::probe();
        assert_eq!(
            adapter.open_path(std::path::Path::new("bad\npath")),
            Err(novahub_platform_api::PlatformError::InvalidInput)
        );
    }

    #[test]
    fn file_search_rejects_control_characters_before_spawning_where() {
        let adapter = WindowsAdapter::probe();
        assert_eq!(
            adapter.search_files(std::path::Path::new("."), "bad\nquery", 10),
            Err(novahub_platform_api::PlatformError::InvalidInput)
        );
    }

    #[test]
    fn http_rejects_insecure_credentials_and_empty_limits_before_network_io() {
        let adapter = WindowsAdapter::probe();
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

    #[cfg(windows)]
    #[test]
    fn file_search_uses_where_for_an_explicit_root() {
        let root = std::env::temp_dir().join(format!("novahub-file-search-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("search root");
        let marker = root.join("novahub-search-marker.txt");
        std::fs::write(&marker, b"marker").expect("marker file");
        let result = WindowsAdapter::probe()
            .with_recursive_file_fallback(true)
            .search_files(&root, "novahub-search-marker", 10)
            .expect("where search");
        assert!(result.iter().any(|item| item.path == marker));
        std::fs::remove_dir_all(&root).expect("remove exact test root");
    }
}
