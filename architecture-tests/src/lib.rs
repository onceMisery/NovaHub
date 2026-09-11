#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    fn project_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("architecture-tests must live below the project root")
            .to_path_buf()
    }

    fn read_manifest(path: &Path) -> String {
        fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
    }

    #[test]
    fn workspace_and_dependency_boundaries_are_declared() {
        let root = project_root();
        let workspace = root.join("Cargo.toml");
        assert!(
            workspace.is_file(),
            "root Cargo.toml must declare the workspace"
        );

        let core = read_manifest(&root.join("crates/core-domain/Cargo.toml"));
        for forbidden in ["slint", "wasmtime", "rusqlite", "sqlite", "platform"] {
            assert!(
                !core.to_ascii_lowercase().contains(forbidden),
                "core-domain must not depend on {forbidden}"
            );
        }

        let app = read_manifest(&root.join("apps/novahub-app/Cargo.toml"));
        for forbidden in ["wasmtime", "plugin-runtime", "novahub-plugin-sdk"] {
            assert!(
                !app.to_ascii_lowercase().contains(forbidden),
                "novahub-app must not link {forbidden}"
            );
        }
    }

    #[test]
    fn plugin_abi_is_present_and_versioned() {
        let root = project_root();
        let wit = read_manifest(&root.join("wit/novahub-plugin/plugin.wit"));
        assert!(wit.contains("package novahub:plugin@1.6.0;"));
        for variant in ["list", "grid", "detail", "form", "action-panel"] {
            assert!(wit.contains(variant), "WIT must declare {variant} view");
        }
        assert!(wit.contains("interface capabilities"));
        for operation in [
            "read-clipboard",
            "write-clipboard",
            "http-get",
            "storage-write",
        ] {
            assert!(
                wit.contains(operation),
                "WIT must declare {operation} capability"
            );
        }
    }

    #[test]
    fn slint_shell_is_kept_outside_the_webview_boundary() {
        let root = project_root();
        assert!(root.join("crates/ui-slint/ui/shell.slint").is_file());
        let app = read_manifest(&root.join("apps/novahub-app/Cargo.toml"));
        assert!(!app.to_ascii_lowercase().contains("tauri"));
        assert!(!app.to_ascii_lowercase().contains("webview"));
    }

    #[test]
    fn official_examples_and_builtin_pets_have_manifests() {
        let root = project_root();
        for path in [
            "plugins/official/json-toolkit/novahub.toml",
            "plugins/official/translate/novahub.toml",
            "plugins/official/pets/waterman/novahub.toml",
            "plugins/official/pets/nova/novahub.toml",
            "plugins/official/pets/pixel/novahub.toml",
        ] {
            assert!(
                root.join(path).is_file(),
                "missing example manifest: {path}"
            );
        }
    }
}
