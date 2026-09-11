#![forbid(unsafe_code)]

use novahub_core_domain::{Action, CommandDescriptor};
use novahub_platform_api::{PlatformError, PlatformServices, SystemCommand as HostSystemCommand};
use novahub_storage::Storage;

pub mod calculator;
pub mod clipboard;
pub mod system;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuiltinActionError {
    UnknownCommand,
    MissingArgument,
    InvalidCalculation,
    ConfirmationRequired,
    UnsupportedOnHost,
    Storage(String),
}

/// Executes only host-owned, bounded built-in actions. Plugin code is never
/// reached through this path.
///
/// # Errors
///
/// Returns a stable action error for unknown commands, missing arguments,
/// invalid expressions, missing confirmation, or unavailable host APIs.
pub fn execute_builtin(action: &Action, confirmed: bool) -> Result<String, BuiltinActionError> {
    match action.command_id().as_str() {
        "calculator.evaluate" => {
            let expression = action
                .arguments()
                .first()
                .ok_or(BuiltinActionError::MissingArgument)?;
            calculator::evaluate(expression)
                .map(|value| value.to_string())
                .map_err(|_| BuiltinActionError::InvalidCalculation)
        }
        "units.convert" => {
            let value = action
                .arguments()
                .first()
                .ok_or(BuiltinActionError::MissingArgument)?
                .parse::<f64>()
                .map_err(|_| BuiltinActionError::InvalidCalculation)?;
            let from = action
                .arguments()
                .get(1)
                .ok_or(BuiltinActionError::MissingArgument)?;
            let to = action
                .arguments()
                .get(2)
                .ok_or(BuiltinActionError::MissingArgument)?;
            calculator::convert(value, from, to)
                .map(|result| result.to_string())
                .map_err(|_| BuiltinActionError::InvalidCalculation)
        }
        "system.command" => {
            let command = action
                .arguments()
                .first()
                .and_then(|id| system::SystemCommand::parse(id))
                .ok_or(BuiltinActionError::UnknownCommand)?;
            if command.requires_confirmation() && !confirmed {
                return Err(BuiltinActionError::ConfirmationRequired);
            }
            Err(BuiltinActionError::UnsupportedOnHost)
        }
        "apps.launch" | "files.search" | "files.open" | "files.reveal" | "files.copy_path"
        | "clipboard.history" | "clipboard.copy" | "snippets.library" | "quicklinks.open"
        | "windows.layout" => Err(BuiltinActionError::UnsupportedOnHost),
        _ => Err(BuiltinActionError::UnknownCommand),
    }
}

/// Executes built-ins that need host-owned persistent state. The app passes its
/// existing `SQLite` connection; providers never open a second database.
///
/// # Errors
///
/// Returns the same bounded action errors as [`execute_builtin`] plus storage
/// validation or lookup failures.
pub fn execute_builtin_with_storage(
    action: &Action,
    confirmed: bool,
    storage: &Storage,
) -> Result<String, BuiltinActionError> {
    match action.command_id().as_str() {
        "quicklinks.open" => {
            let id = action
                .arguments()
                .first()
                .ok_or(BuiltinActionError::MissingArgument)?;
            let query = action.arguments().get(1).map_or("", String::as_str);
            storage
                .resolve_quicklink(id, query)
                .map_err(|error| BuiltinActionError::Storage(error.to_string()))
        }
        "snippets.library" => {
            let id = action
                .arguments()
                .first()
                .ok_or(BuiltinActionError::MissingArgument)?;
            storage
                .read_snippet(id)
                .map_err(|error| BuiltinActionError::Storage(error.to_string()))
        }
        _ => execute_builtin(action, confirmed),
    }
}

/// Executes actions that cross the host platform boundary.
///
/// # Errors
///
/// Returns confirmation, validation, storage, or platform capability errors.
pub fn execute_builtin_with_host(
    action: &Action,
    confirmed: bool,
    storage: &Storage,
    platform: &dyn PlatformServices,
) -> Result<String, BuiltinActionError> {
    match action.command_id().as_str() {
        "apps.launch" => {
            let path = action
                .arguments()
                .first()
                .ok_or(BuiltinActionError::MissingArgument)?;
            platform
                .launch_application(std::path::Path::new(path))
                .map(|()| format!("Launched {path}"))
                .map_err(platform_error)
        }
        "files.search" => {
            let root = action
                .arguments()
                .first()
                .ok_or(BuiltinActionError::MissingArgument)?;
            let query = action
                .arguments()
                .get(1)
                .ok_or(BuiltinActionError::MissingArgument)?;
            let results = platform
                .search_files(std::path::Path::new(root), query, 20)
                .map_err(platform_error)?;
            Ok(format_file_results(&results))
        }
        "files.open" => {
            let path = action
                .arguments()
                .first()
                .ok_or(BuiltinActionError::MissingArgument)?;
            platform
                .open_path(std::path::Path::new(path))
                .map(|()| format!("Opened {path}"))
                .map_err(platform_error)
        }
        "files.reveal" => {
            let path = action
                .arguments()
                .first()
                .ok_or(BuiltinActionError::MissingArgument)?;
            platform
                .reveal_path(std::path::Path::new(path))
                .map(|()| format!("Opened containing folder for {path}"))
                .map_err(platform_error)
        }
        "files.copy_path" => {
            let path = action
                .arguments()
                .first()
                .ok_or(BuiltinActionError::MissingArgument)?;
            platform
                .write_clipboard_text(path)
                .map(|()| "File path copied".to_owned())
                .map_err(platform_error)
        }
        "clipboard.copy" => {
            let text = action
                .arguments()
                .first()
                .ok_or(BuiltinActionError::MissingArgument)?;
            platform
                .write_clipboard_text(text)
                .map(|()| "Text copied".to_owned())
                .map_err(platform_error)
        }
        "system.command" => {
            let command = action
                .arguments()
                .first()
                .and_then(|id| system::SystemCommand::parse(id))
                .ok_or(BuiltinActionError::UnknownCommand)?;
            if command.requires_confirmation() && !confirmed {
                return Err(BuiltinActionError::ConfirmationRequired);
            }
            platform
                .execute_system_command(command.into())
                .map(|()| "System command sent".into())
                .map_err(platform_error)
        }
        _ => execute_builtin_with_storage(action, confirmed, storage),
    }
}

fn format_file_results(results: &[novahub_platform_api::FileSearchResult]) -> String {
    if results.is_empty() {
        "No files found".into()
    } else {
        results
            .iter()
            .map(|result| result.path.display().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn platform_error(error: PlatformError) -> BuiltinActionError {
    match error {
        PlatformError::Unavailable(_) => BuiltinActionError::UnsupportedOnHost,
        PlatformError::InvalidInput => BuiltinActionError::MissingArgument,
        PlatformError::LimitExceeded => {
            BuiltinActionError::Storage("platform response limit exceeded".into())
        }
        PlatformError::Timeout => {
            BuiltinActionError::Storage("platform operation exceeded its deadline".into())
        }
        PlatformError::Io(message) => BuiltinActionError::Storage(message),
    }
}

impl From<system::SystemCommand> for HostSystemCommand {
    fn from(command: system::SystemCommand) -> Self {
        match command {
            system::SystemCommand::OpenSettings => Self::OpenSettings,
            system::SystemCommand::Lock => Self::Lock,
            system::SystemCommand::Sleep => Self::Sleep,
            system::SystemCommand::Logout => Self::Logout,
            system::SystemCommand::Restart => Self::Restart,
        }
    }
}

#[cfg(test)]
mod calculator_contract_tests {
    use super::{
        Action, BuiltinActionError, calculator::evaluate, execute_builtin,
        execute_builtin_with_storage,
    };

    #[test]
    fn calculator_evaluates_parentheses_and_percent_without_eval() {
        assert!((evaluate("2 * (3 + 4)").expect("valid expression") - 14.0).abs() < 1e-9);
        assert!((evaluate("50%").expect("percent") - 0.5).abs() < 1e-9);
    }

    #[test]
    fn calculator_rejects_invalid_or_unsafe_expressions() {
        assert!(evaluate("2 / 0").is_err());
        assert!(evaluate("process::Command::new('bad')").is_err());
    }

    #[test]
    fn builtin_action_path_requires_confirmation_for_destructive_system_commands() {
        let action = Action::invoke("system.command", ["system.restart"]);
        assert_eq!(
            execute_builtin(&action, false),
            Err(BuiltinActionError::ConfirmationRequired)
        );
    }

    #[test]
    fn builtin_action_path_evaluates_local_calculator_without_a_script_runtime() {
        let action = Action::invoke("calculator.evaluate", ["2 + 2"]);
        assert_eq!(execute_builtin(&action, false), Ok("4".into()));
    }

    #[test]
    fn quicklinks_and_snippets_use_host_storage_without_script_execution() {
        let storage = novahub_storage::Storage::open_in_memory().expect("storage");
        storage
            .save_quicklink("docs", "Docs", "https://example.test/search?q={query}")
            .expect("quicklink");
        storage
            .save_snippet("meeting", "Meeting", "Plain text")
            .expect("snippet");
        assert_eq!(
            execute_builtin_with_storage(
                &Action::invoke("quicklinks.open", ["docs", "Rust Slint"]),
                false,
                &storage,
            ),
            Ok("https://example.test/search?q=Rust%20Slint".into())
        );
        assert_eq!(
            execute_builtin_with_storage(
                &Action::invoke("snippets.library", ["meeting"]),
                false,
                &storage,
            ),
            Ok("Plain text".into())
        );
    }
}

#[cfg(test)]
mod file_action_tests {
    use std::cell::RefCell;

    use super::{Action, BuiltinActionError, execute_builtin_with_host};
    use novahub_platform_api::{PlatformError, PlatformServices};

    #[derive(Default)]
    struct RecordingPlatform {
        calls: RefCell<Vec<String>>,
    }

    impl PlatformServices for RecordingPlatform {
        fn open_path(&self, path: &std::path::Path) -> Result<(), PlatformError> {
            self.calls
                .borrow_mut()
                .push(format!("open:{}", path.display()));
            Ok(())
        }

        fn reveal_path(&self, path: &std::path::Path) -> Result<(), PlatformError> {
            self.calls
                .borrow_mut()
                .push(format!("reveal:{}", path.display()));
            Ok(())
        }

        fn write_clipboard_text(&self, text: &str) -> Result<(), PlatformError> {
            self.calls.borrow_mut().push(format!("copy:{text}"));
            Ok(())
        }
    }

    #[test]
    fn file_result_actions_stay_inside_the_host_platform_boundary() {
        let storage = novahub_storage::Storage::open_in_memory().expect("storage");
        let platform = RecordingPlatform::default();

        assert_eq!(
            execute_builtin_with_host(
                &Action::invoke("files.open", ["C:/Docs/readme.md"]),
                false,
                &storage,
                &platform,
            ),
            Ok("Opened C:/Docs/readme.md".into())
        );
        assert_eq!(
            execute_builtin_with_host(
                &Action::invoke("files.reveal", ["C:/Docs/readme.md"]),
                false,
                &storage,
                &platform,
            ),
            Ok("Opened containing folder for C:/Docs/readme.md".into())
        );
        assert_eq!(
            execute_builtin_with_host(
                &Action::invoke("files.copy_path", ["C:/Docs/readme.md"]),
                false,
                &storage,
                &platform,
            ),
            Ok("File path copied".into())
        );
        assert_eq!(
            execute_builtin_with_host(
                &Action::invoke("clipboard.copy", ["42"]),
                false,
                &storage,
                &platform,
            ),
            Ok("Text copied".into())
        );
        assert_eq!(
            platform.calls.into_inner(),
            vec![
                "open:C:/Docs/readme.md",
                "reveal:C:/Docs/readme.md",
                "copy:C:/Docs/readme.md",
                "copy:42",
            ]
        );
    }

    #[test]
    fn file_result_actions_reject_missing_paths() {
        let storage = novahub_storage::Storage::open_in_memory().expect("storage");
        let platform = RecordingPlatform::default();
        assert_eq!(
            execute_builtin_with_host(
                &Action::invoke("files.copy_path", std::iter::empty::<&str>()),
                false,
                &storage,
                &platform,
            ),
            Err(BuiltinActionError::MissingArgument)
        );
    }
}

#[must_use]
pub fn builtin_commands() -> Vec<CommandDescriptor> {
    builtin_provider_catalog()
        .into_iter()
        .flat_map(|provider| provider.commands)
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltinProvider {
    pub id: &'static str,
    pub title: &'static str,
    pub commands: Vec<CommandDescriptor>,
}

#[must_use]
pub fn builtin_provider_catalog() -> Vec<BuiltinProvider> {
    let mut providers = [
        (
            "apps",
            "Applications",
            "apps.launch",
            "Launch an application",
        ),
        ("files", "Files", "files.search", "Search indexed files"),
        (
            "clipboard",
            "Clipboard",
            "clipboard.history",
            "Open clipboard history",
        ),
        (
            "snippets",
            "Snippets",
            "snippets.library",
            "Open snippets library",
        ),
        (
            "quicklinks",
            "Quicklinks",
            "quicklinks.open",
            "Open a saved quicklink",
        ),
        (
            "windows",
            "Window Manager",
            "windows.layout",
            "Arrange windows",
        ),
        (
            "system",
            "System",
            "system.command",
            "Run a safe system command",
        ),
        (
            "calculator",
            "Calculator",
            "calculator.evaluate",
            "Evaluate a local expression",
        ),
        (
            "preferences",
            "Preferences",
            "hotkey.configure",
            "Configure the global shortcut",
        ),
        (
            "units",
            "Unit conversion",
            "units.convert",
            "Convert common units locally",
        ),
    ]
    .into_iter()
    .map(|(id, title, command_id, subtitle)| BuiltinProvider {
        id,
        title,
        commands: vec![CommandDescriptor {
            id: novahub_core_domain::CommandId::new(command_id),
            title: title.to_owned(),
            subtitle: subtitle.to_owned(),
        }],
    })
    .collect::<Vec<_>>();
    if let Some(clipboard) = providers
        .iter_mut()
        .find(|provider| provider.id == "clipboard")
    {
        clipboard.commands.push(CommandDescriptor {
            id: novahub_core_domain::CommandId::new("clipboard.copy"),
            title: "Copy text".to_owned(),
            subtitle: "Copy bounded text to the host clipboard".to_owned(),
        });
    }
    if let Some(system) = providers
        .iter_mut()
        .find(|provider| provider.id == "system")
    {
        system.commands.push(CommandDescriptor {
            id: novahub_core_domain::CommandId::new("diagnostics.view"),
            title: "Diagnostics".to_owned(),
            subtitle: "View host diagnostics".to_owned(),
        });
    }
    providers
}

#[cfg(test)]
mod tests {
    use super::{Action, builtin_commands, builtin_provider_catalog, execute_builtin};
    use std::collections::BTreeSet;

    #[test]
    fn catalog_contains_ten_unique_builtin_provider_ids() {
        let catalog = builtin_provider_catalog();
        let ids: BTreeSet<_> = catalog.iter().map(|provider| provider.id).collect();
        assert_eq!(catalog.len(), 10);
        assert_eq!(ids.len(), 10);
        assert_eq!(builtin_commands().len(), 12);
    }

    #[test]
    fn calculator_command_is_local_and_explicitly_named() {
        let calculator = builtin_commands()
            .into_iter()
            .find(|command| command.id.as_str() == "calculator.evaluate")
            .expect("calculator provider command");
        assert!(calculator.subtitle.contains("local"));
    }

    #[test]
    fn diagnostics_command_is_host_owned_and_searchable() {
        let command = builtin_commands()
            .into_iter()
            .find(|command| command.id.as_str() == "diagnostics.view")
            .expect("diagnostics command");
        assert_eq!(command.title, "Diagnostics");
        assert!(command.subtitle.contains("host"));
    }

    #[test]
    fn unit_conversion_is_exposed_as_a_bounded_builtin_action() {
        let action = Action::invoke("units.convert", ["1", "km", "m"]);
        assert_eq!(execute_builtin(&action, false), Ok("1000".into()));
    }
}
