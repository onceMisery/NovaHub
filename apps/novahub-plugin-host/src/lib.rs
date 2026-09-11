#![forbid(unsafe_code)]

use std::{collections::BTreeMap, fs, path::Path, sync::Arc};

use novahub_plugin_manager::{AuthorizedCapability, EffectiveGrant, PluginIdentity};
use novahub_plugin_runtime::{
    CapabilityAdapter, CapabilityValue, ComponentRuntime, ComponentSession, LoadedComponent,
    PluginSession, ResourceLimits, SessionId,
};
use serde::{Deserialize, Serialize};

pub const MAX_ACTIVE_SESSIONS: usize = 4;
pub const MAX_COMPONENT_BYTES: u64 = 16 * 1024 * 1024;

/// Accepts only a token supplied by the spawning `NovaHub` process.
#[must_use]
pub fn auth_token_matches(expected: Option<&str>, actual: &str) -> bool {
    expected.is_some_and(|expected| !expected.is_empty() && expected == actual)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostRequest {
    Load {
        session_id: String,
        component_path: String,
        #[serde(default = "default_plugin_id")]
        plugin_id: String,
        #[serde(default)]
        grant: EffectiveGrant,
    },
    Open {
        session_id: String,
    },
    Update {
        session_id: String,
        revision: u64,
        #[serde(default)]
        input: String,
    },
    Run {
        session_id: String,
        command_id: String,
        #[serde(default)]
        input: String,
    },
    Close {
        session_id: String,
    },
    CapabilityResponse {
        capability_request_id: String,
        result: CapabilityResponsePayload,
    },
}

fn default_plugin_id() -> String {
    "unbound".into()
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostResponse {
    Ack {
        request_id: String,
        revision: u64,
    },
    Loaded {
        request_id: String,
    },
    View {
        request_id: String,
        revision: u64,
        view: ViewPayload,
    },
    CommandResult {
        request_id: String,
        result: CommandResultPayload,
    },
    CapabilityRequest {
        request_id: String,
        capability_request_id: String,
        plugin_id: String,
        session_id: String,
        call: CapabilityCallPayload,
    },
    Error {
        request_id: String,
        code: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct CapabilityCallPayload {
    pub capability: AuthorizedCapability,
    pub payload: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CapabilityResponsePayload {
    Unit,
    Text { value: String },
    Bytes { value: Vec<u8> },
    Error { code: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ViewPayload {
    Empty {
        title: String,
        description: String,
    },
    Loading {
        title: String,
        message: String,
    },
    Progress {
        title: String,
        completed: u64,
        total: u64,
    },
    Error {
        title: String,
        message: String,
        recoverable: bool,
    },
    List {
        title: String,
        items: Vec<ListItemPayload>,
        next_cursor: Option<String>,
    },
    Grid {
        title: String,
        items: Vec<GridItemPayload>,
        next_cursor: Option<String>,
    },
    Detail {
        title: String,
        markdown: String,
        metadata: Vec<MetadataPayload>,
    },
    Form {
        title: String,
        fields: Vec<FormFieldPayload>,
    },
    ActionPanel {
        title: String,
        actions: Vec<ActionItemPayload>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct CommandResultPayload {
    pub text: String,
    pub copy_text: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ListItemPayload {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub accessory: Option<ItemAccessoryPayload>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct GridItemPayload {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub accessory: Option<ItemAccessoryPayload>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ResourceRefPayload {
    pub id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ItemAccessoryPayload {
    pub status: String,
    pub badge: String,
    pub shortcut: String,
    pub icon: Option<ResourceRefPayload>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct MetadataPayload {
    pub key: String,
    pub value: MetadataValuePayload,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum MetadataValuePayload {
    Text(String),
    Link(String),
    Tag(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct FormFieldPayload {
    pub id: String,
    pub label: String,
    pub control: String,
    pub required: bool,
    pub initial_value: String,
    pub helper: String,
    pub error: String,
    pub options: Vec<FormOptionPayload>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct FormOptionPayload {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ActionItemPayload {
    pub id: String,
    pub title: String,
    pub role: String,
    pub destructive: bool,
}

pub struct HostServer {
    sessions: BTreeMap<String, PluginSession>,
    components: BTreeMap<String, PendingComponent>,
    component_sessions: BTreeMap<String, ComponentSession>,
    runtime: Option<std::sync::Arc<ComponentRuntime>>,
    cache_directory: Option<std::path::PathBuf>,
    capability_adapter: Arc<dyn CapabilityAdapter>,
}

struct PendingComponent {
    component: LoadedComponent,
    identity: PluginIdentity,
    grant: EffectiveGrant,
}

#[derive(Default)]
struct RejectingCapabilityAdapter;

impl CapabilityAdapter for RejectingCapabilityAdapter {
    fn call(
        &self,
        _identity: &PluginIdentity,
        _capability: AuthorizedCapability,
        _payload: Option<&str>,
    ) -> Result<CapabilityValue, String> {
        Err("capability IPC adapter is unavailable".into())
    }
}

impl Default for HostServer {
    fn default() -> Self {
        Self::new()
    }
}

impl HostServer {
    #[must_use]
    pub fn new() -> Self {
        Self::with_capability_adapter(Arc::new(RejectingCapabilityAdapter))
    }

    #[must_use]
    pub fn with_capability_adapter(capability_adapter: Arc<dyn CapabilityAdapter>) -> Self {
        Self::with_capability_adapter_and_cache(capability_adapter, None)
    }

    #[must_use]
    pub fn with_capability_adapter_and_cache(
        capability_adapter: Arc<dyn CapabilityAdapter>,
        cache_directory: Option<std::path::PathBuf>,
    ) -> Self {
        Self {
            sessions: BTreeMap::new(),
            components: BTreeMap::new(),
            component_sessions: BTreeMap::new(),
            runtime: None,
            cache_directory,
            capability_adapter,
        }
    }

    #[must_use]
    pub fn handle(&mut self, request: HostRequest, request_id: impl Into<String>) -> HostResponse {
        let request_id = request_id.into();
        match request {
            HostRequest::Load {
                session_id,
                component_path,
                plugin_id,
                grant,
            } => self.load_component(&session_id, &component_path, &plugin_id, grant, request_id),
            HostRequest::Open { session_id } => self.open_session(session_id, request_id),
            HostRequest::Update {
                session_id,
                revision,
                input,
            } => self.update_session(&session_id, revision, &input, request_id),
            HostRequest::Run {
                session_id,
                command_id,
                input,
            } => self.run_command(&session_id, &command_id, &input, request_id),
            HostRequest::Close { session_id } => self.close_session(&session_id, request_id),
            HostRequest::CapabilityResponse { .. } => HostResponse::Error {
                request_id,
                code: "unexpected_capability_response".into(),
            },
        }
    }

    fn run_command(
        &mut self,
        session_id: &str,
        command_id: &str,
        input: &str,
        request_id: String,
    ) -> HostResponse {
        let Some(pending) = self.components.remove(session_id) else {
            return HostResponse::Error {
                request_id,
                code: "component_not_loaded".into(),
            };
        };
        let mut session = match pending.component.instantiate_with_adapter(
            pending.identity,
            pending.grant,
            Arc::clone(&self.capability_adapter),
        ) {
            Ok(session) => session,
            Err(error) => {
                return HostResponse::Error {
                    request_id,
                    code: format!("component_instantiate:{error}"),
                };
            }
        };
        match session.run(command_id, input) {
            Ok(result) => HostResponse::CommandResult {
                request_id,
                result: CommandResultPayload {
                    text: result.text,
                    copy_text: result.copy_text,
                },
            },
            Err(error) => HostResponse::Error {
                request_id,
                code: format!("component_run:{error}"),
            },
        }
    }

    fn open_session(&mut self, session_id: String, request_id: String) -> HostResponse {
        if let Some(pending) = self.components.remove(&session_id) {
            return self.open_component(&session_id, pending, request_id);
        }
        if self.sessions.contains_key(&session_id) {
            return HostResponse::Error {
                request_id,
                code: "session_exists".into(),
            };
        }
        if self.active_session_count() >= MAX_ACTIVE_SESSIONS {
            return HostResponse::Error {
                request_id,
                code: "session_limit".into(),
            };
        }
        let session = PluginSession::open(SessionId::new(session_id.clone()));
        let revision = session.revision();
        self.sessions.insert(session_id, session);
        HostResponse::Ack {
            request_id,
            revision,
        }
    }

    fn open_component(
        &mut self,
        session_id: &str,
        pending: PendingComponent,
        request_id: String,
    ) -> HostResponse {
        let mut component_session = match pending.component.instantiate_with_adapter(
            pending.identity,
            pending.grant,
            Arc::clone(&self.capability_adapter),
        ) {
            Ok(session) => session,
            Err(error) => {
                return HostResponse::Error {
                    request_id,
                    code: format!("component_instantiate:{error}"),
                };
            }
        };
        let view = match component_session.open() {
            Ok(view) => view,
            Err(error) => {
                return HostResponse::Error {
                    request_id,
                    code: format!("component_open:{error}"),
                };
            }
        };
        self.component_sessions
            .insert(session_id.to_owned(), component_session);
        let session = PluginSession::open(SessionId::new(session_id.to_owned()));
        let revision = session.revision();
        self.sessions.insert(session_id.to_owned(), session);
        HostResponse::View {
            request_id,
            revision,
            view: ViewPayload::from(view),
        }
    }

    fn update_session(
        &mut self,
        session_id: &str,
        revision: u64,
        input: &str,
        request_id: String,
    ) -> HostResponse {
        let Some(session) = self.sessions.get_mut(session_id) else {
            return HostResponse::Error {
                request_id,
                code: "session_not_found".into(),
            };
        };
        match session.update(revision) {
            Ok(revision) => match self.component_sessions.get_mut(session_id) {
                Some(component_session) => match component_session.update(input) {
                    Ok(view) => HostResponse::View {
                        request_id,
                        revision,
                        view: ViewPayload::from(view),
                    },
                    Err(error) => HostResponse::Error {
                        request_id,
                        code: format!("component_update:{error}"),
                    },
                },
                None => HostResponse::Ack {
                    request_id,
                    revision,
                },
            },
            Err(error) => HostResponse::Error {
                request_id,
                code: error.code().into(),
            },
        }
    }

    fn close_session(&mut self, session_id: &str, request_id: String) -> HostResponse {
        if let Some(component_session) = self.component_sessions.remove(session_id)
            && let Err(error) = component_session.close()
        {
            return HostResponse::Error {
                request_id,
                code: format!("component_close:{error}"),
            };
        }
        if let Some(mut session) = self.sessions.remove(session_id) {
            session.close();
            HostResponse::Ack {
                request_id,
                revision: session.revision(),
            }
        } else {
            HostResponse::Error {
                request_id,
                code: "session_not_found".into(),
            }
        }
    }

    fn load_component(
        &mut self,
        session_id: &str,
        component_path: &str,
        plugin_id: &str,
        grant: EffectiveGrant,
        request_id: String,
    ) -> HostResponse {
        if self.active_session_count() >= MAX_ACTIVE_SESSIONS {
            return HostResponse::Error {
                request_id,
                code: "session_limit".into(),
            };
        }
        if self.sessions.contains_key(session_id)
            || self.components.contains_key(session_id)
            || self.component_sessions.contains_key(session_id)
        {
            return HostResponse::Error {
                request_id,
                code: "session_exists".into(),
            };
        }
        let path = Path::new(component_path);
        let Ok(metadata) = fs::metadata(path) else {
            return HostResponse::Error {
                request_id,
                code: "component_not_found".into(),
            };
        };
        if metadata.len() > MAX_COMPONENT_BYTES {
            return HostResponse::Error {
                request_id,
                code: "component_too_large".into(),
            };
        }
        let Ok(bytes) = fs::read(path) else {
            return HostResponse::Error {
                request_id,
                code: "component_read_failed".into(),
            };
        };
        let runtime = if let Some(runtime) = &self.runtime {
            std::sync::Arc::clone(runtime)
        } else {
            let runtime_result = match self.cache_directory.as_deref() {
                Some(cache_directory) => {
                    ComponentRuntime::new_with_cache(ResourceLimits::default(), cache_directory)
                }
                None => ComponentRuntime::new(ResourceLimits::default()),
            };
            let runtime = match runtime_result {
                Ok(runtime) => runtime,
                Err(error) => {
                    return HostResponse::Error {
                        request_id,
                        code: format!("runtime_configuration:{error}"),
                    };
                }
            };
            self.runtime = Some(std::sync::Arc::clone(&runtime));
            runtime
        };
        match runtime.load(&bytes) {
            Ok(component) => {
                self.components.insert(
                    session_id.to_owned(),
                    PendingComponent {
                        component,
                        identity: PluginIdentity::new(plugin_id, session_id),
                        grant,
                    },
                );
                HostResponse::Loaded { request_id }
            }
            Err(error) => HostResponse::Error {
                request_id,
                code: format!("component_load:{error}"),
            },
        }
    }

    fn active_session_count(&self) -> usize {
        self.sessions.len().saturating_add(self.components.len())
    }
}

impl From<novahub_plugin_runtime::wit::View> for ViewPayload {
    fn from(view: novahub_plugin_runtime::wit::View) -> Self {
        use novahub_plugin_runtime::wit::View;
        match view {
            View::Empty(view) => Self::Empty {
                title: view.title,
                description: view.description,
            },
            View::Loading(view) => Self::Loading {
                title: view.title,
                message: view.message,
            },
            View::Progress(view) => Self::Progress {
                title: view.title,
                completed: view.completed,
                total: view.total,
            },
            View::Error(view) => Self::Error {
                title: view.title,
                message: view.message,
                recoverable: view.recoverable,
            },
            View::List(view) => Self::List {
                title: view.title,
                next_cursor: view.next_cursor,
                items: view
                    .items
                    .into_iter()
                    .map(|item| ListItemPayload {
                        id: item.id,
                        title: item.title,
                        subtitle: item.subtitle,
                        accessory: item.accessory.map(item_accessory_payload),
                    })
                    .collect(),
            },
            View::Grid(view) => Self::Grid {
                title: view.title,
                next_cursor: view.next_cursor,
                items: view
                    .items
                    .into_iter()
                    .map(|item| GridItemPayload {
                        id: item.id,
                        title: item.title,
                        subtitle: item.subtitle,
                        accessory: item.accessory.map(item_accessory_payload),
                    })
                    .collect(),
            },
            View::Detail(view) => detail_view_payload(view),
            View::Form(view) => Self::Form {
                title: view.title,
                fields: view
                    .fields
                    .into_iter()
                    .map(|field| FormFieldPayload {
                        id: field.id,
                        label: field.label,
                        control: form_control_name(field.control).into(),
                        required: field.required,
                        initial_value: field.initial_value,
                        helper: field.helper,
                        error: field.error,
                        options: field
                            .options
                            .into_iter()
                            .map(|option| FormOptionPayload {
                                id: option.id,
                                label: option.label,
                            })
                            .collect(),
                    })
                    .collect(),
            },
            View::ActionPanel(view) => Self::ActionPanel {
                title: view.title,
                actions: view
                    .actions
                    .into_iter()
                    .map(|action| ActionItemPayload {
                        id: action.id,
                        title: action.title,
                        role: action_role_name(action.role).into(),
                        destructive: action.destructive,
                    })
                    .collect(),
            },
        }
    }
}

fn action_role_name(
    role: novahub_plugin_runtime::wit::novahub::plugin::types::ActionRole,
) -> &'static str {
    use novahub_plugin_runtime::wit::novahub::plugin::types::ActionRole;
    match role {
        ActionRole::Default => "default",
        ActionRole::Secondary => "secondary",
    }
}

fn detail_view_payload(
    view: novahub_plugin_runtime::wit::novahub::plugin::types::DetailView,
) -> ViewPayload {
    ViewPayload::Detail {
        title: view.title,
        markdown: view.markdown,
        metadata: view
            .metadata
            .into_iter()
            .map(|item| MetadataPayload {
                key: item.key,
                value: metadata_value_payload(item.value),
            })
            .collect(),
    }
}

fn item_accessory_payload(
    accessory: novahub_plugin_runtime::wit::novahub::plugin::types::ItemAccessory,
) -> ItemAccessoryPayload {
    ItemAccessoryPayload {
        status: accessory.status,
        badge: accessory.badge,
        shortcut: accessory.shortcut,
        icon: accessory
            .icon
            .map(|icon| ResourceRefPayload { id: icon.id }),
    }
}

fn metadata_value_payload(
    value: novahub_plugin_runtime::wit::novahub::plugin::types::MetadataValue,
) -> MetadataValuePayload {
    use novahub_plugin_runtime::wit::novahub::plugin::types::MetadataValue;
    match value {
        MetadataValue::Text(value) => MetadataValuePayload::Text(value),
        MetadataValue::Link(value) => MetadataValuePayload::Link(value),
        MetadataValue::Tag(value) => MetadataValuePayload::Tag(value),
    }
}

fn form_control_name(
    control: novahub_plugin_runtime::wit::novahub::plugin::types::FormControl,
) -> &'static str {
    use novahub_plugin_runtime::wit::novahub::plugin::types::FormControl;
    match control {
        FormControl::Text => "text",
        FormControl::Password => "password",
        FormControl::Select => "select",
        FormControl::Checkbox => "checkbox",
        FormControl::Switch => "switch",
    }
}

impl From<novahub_plugin_runtime::wit::CommandResult> for CommandResultPayload {
    fn from(result: novahub_plugin_runtime::wit::CommandResult) -> Self {
        Self {
            text: result.text,
            copy_text: result.copy_text,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::{HostRequest, HostResponse, HostServer, MAX_ACTIVE_SESSIONS, auth_token_matches};

    #[test]
    fn ipc_authentication_requires_the_spawn_token() {
        assert!(auth_token_matches(Some("token"), "token"));
        assert!(!auth_token_matches(None, "token"));
        assert!(!auth_token_matches(Some("token"), "other"));
        assert!(!auth_token_matches(Some(""), ""));
    }

    #[test]
    fn host_routes_sessions_and_rejects_stale_updates() {
        let mut host = HostServer::new();
        assert_eq!(
            host.handle(
                HostRequest::Open {
                    session_id: "a".into()
                },
                "open-1"
            ),
            HostResponse::Ack {
                request_id: "open-1".into(),
                revision: 1
            }
        );
        assert_eq!(
            host.handle(
                HostRequest::Update {
                    session_id: "a".into(),
                    revision: 1,
                    input: String::new(),
                },
                "update-1"
            ),
            HostResponse::Ack {
                request_id: "update-1".into(),
                revision: 2
            }
        );
        assert_eq!(
            host.handle(
                HostRequest::Update {
                    session_id: "a".into(),
                    revision: 1,
                    input: String::new(),
                },
                "update-2"
            ),
            HostResponse::Error {
                request_id: "update-2".into(),
                code: "stale_revision".into()
            }
        );
    }

    #[test]
    fn host_rejects_a_fifth_active_session_before_allocating_it() {
        let mut host = HostServer::new();
        for index in 0..MAX_ACTIVE_SESSIONS {
            assert!(matches!(
                host.handle(
                    HostRequest::Open {
                        session_id: format!("session-{index}")
                    },
                    format!("open-{index}")
                ),
                HostResponse::Ack { .. }
            ));
        }
        assert_eq!(
            host.handle(
                HostRequest::Open {
                    session_id: "overflow".into()
                },
                "overflow"
            ),
            HostResponse::Error {
                request_id: "overflow".into(),
                code: "session_limit".into()
            }
        );
    }

    #[test]
    fn one_shot_requires_a_loaded_component_and_does_not_create_a_view_session() {
        let mut host = HostServer::new();
        assert_eq!(
            host.handle(
                HostRequest::Run {
                    session_id: "one-shot".into(),
                    command_id: "json.format".into(),
                    input: "{}".into(),
                },
                "run-1",
            ),
            HostResponse::Error {
                request_id: "run-1".into(),
                code: "component_not_loaded".into(),
            }
        );
    }

    #[test]
    fn component_fixture_executes_when_configured() {
        let Ok(component_path) = env::var("NOVAHUB_COMPONENT_FIXTURE") else {
            return;
        };
        let mut host = HostServer::new();
        assert_eq!(
            host.handle(
                HostRequest::Load {
                    session_id: "fixture".into(),
                    component_path: component_path.clone(),
                    plugin_id: "fixture".into(),
                    grant: novahub_plugin_manager::EffectiveGrant::default(),
                },
                "load-1",
            ),
            HostResponse::Loaded {
                request_id: "load-1".into(),
            }
        );
        let opened = host.handle(
            HostRequest::Open {
                session_id: "fixture".into(),
            },
            "open-1",
        );
        assert!(
            matches!(opened, HostResponse::View { revision: 1, .. }),
            "unexpected open response: {opened:?}"
        );
        let updated = host.handle(
            HostRequest::Update {
                session_id: "fixture".into(),
                revision: 1,
                input: "hello".into(),
            },
            "update-1",
        );
        assert!(
            matches!(updated, HostResponse::View { revision: 2, .. }),
            "unexpected update response: {updated:?}"
        );
        assert!(matches!(
            host.handle(
                HostRequest::Close {
                    session_id: "fixture".into(),
                },
                "close-1",
            ),
            HostResponse::Ack { revision: 2, .. }
        ));

        let mut one_shot_host = HostServer::new();
        assert!(matches!(
            one_shot_host.handle(
                HostRequest::Load {
                    session_id: "one-shot".into(),
                    component_path,
                    plugin_id: "fixture".into(),
                    grant: novahub_plugin_manager::EffectiveGrant::default(),
                },
                "load-one-shot",
            ),
            HostResponse::Loaded { .. }
        ));
        assert_eq!(
            one_shot_host.handle(
                HostRequest::Run {
                    session_id: "one-shot".into(),
                    command_id: "fixture.echo".into(),
                    input: "hello".into(),
                },
                "run-one-shot",
            ),
            HostResponse::CommandResult {
                request_id: "run-one-shot".into(),
                result: super::CommandResultPayload {
                    text: "fixture.echo: hello".into(),
                    copy_text: None,
                },
            }
        );
    }
}
