wit_bindgen::generate!({
    world: "plugin",
    path: "../../../wit/novahub-plugin",
});

use crate::novahub::plugin::types;

struct FixturePlugin;

impl Guest for FixturePlugin {
    fn initialize() -> Result<(), String> {
        Ok(())
    }

    fn open_view() -> Result<types::View, String> {
        Ok(types::View::List(types::ListView {
            title: "Fixture Plugin".into(),
            items: vec![types::ListItem {
                id: "fixture.open".into(),
                title: "Fixture ready".into(),
                subtitle: "Executed inside Wasmtime".into(),
                accessory: Some(types::ItemAccessory {
                    status: "Ready".into(),
                    badge: "FIXTURE".into(),
                    shortcut: "Enter".into(),
                    icon: Some(types::ResourceRef {
                        id: "fixture.icon".into(),
                    }),
                }),
            }],
            next_cursor: Some("fixture-page-2".into()),
        }))
    }

    fn update(input: String) -> Result<types::View, String> {
        if input == "view:actions" {
            return Ok(types::View::ActionPanel(types::ActionPanelView {
                title: "Fixture actions".into(),
                actions: vec![
                    types::ActionItem {
                        id: "fixture.open".into(),
                        title: "Open fixture".into(),
                        role: types::ActionRole::Default,
                        destructive: false,
                    },
                    types::ActionItem {
                        id: "fixture.delete".into(),
                        title: "Delete fixture".into(),
                        role: types::ActionRole::Secondary,
                        destructive: true,
                    },
                ],
            }));
        }
        if input.contains("\"event\":\"load_more\"")
            && input.contains("\"cursor\":\"fixture-page-2\"")
        {
            return Ok(types::View::List(types::ListView {
                title: "Fixture Plugin".into(),
                items: vec![types::ListItem {
                    id: "fixture.page-2".into(),
                    title: "Fixture page 2".into(),
                    subtitle: "Loaded from an opaque cursor".into(),
                    accessory: None,
                }],
                next_cursor: None,
            }));
        }
        if let Some(total_bytes) = input.strip_prefix("capability:storage:") {
            let markdown = total_bytes.parse::<u64>().map_or_else(
                |_| "storage:error:invalid byte count".to_owned(),
                |total_bytes| {
                    crate::novahub::plugin::capabilities::storage_write(total_bytes).map_or_else(
                        |error| format!("storage:error:{error}"),
                        |()| "storage:ok".to_owned(),
                    )
                },
            );
            return Ok(types::View::Detail(types::DetailView {
                title: "Storage capability".into(),
                markdown,
                metadata: Vec::new(),
            }));
        }
        if let Some(token) = input.strip_prefix("capability:file:") {
            let markdown = if token == "pick" {
                crate::novahub::plugin::capabilities::pick_file()
                    .and_then(|token| {
                        token.map_or_else(
                            || Err("file_picker_cancelled".into()),
                            |token| crate::novahub::plugin::capabilities::read_file(&token),
                        )
                    })
                    .map_or_else(
                        |error| format!("file:error:{error}"),
                        |value| format!("file:ok:{value}"),
                    )
            } else {
                crate::novahub::plugin::capabilities::read_file(token).map_or_else(
                    |error| format!("file:error:{error}"),
                    |value| format!("file:ok:{value}"),
                )
            };
            return Ok(types::View::Detail(types::DetailView {
                title: "File capability".into(),
                markdown,
                metadata: Vec::new(),
            }));
        }
        if let Some(url) = input.strip_prefix("capability:http:") {
            let markdown = crate::novahub::plugin::capabilities::http_get(url).map_or_else(
                |error| format!("http:error:{error}"),
                |value| format!("http:ok:{value}"),
            );
            return Ok(types::View::Detail(types::DetailView {
                title: "HTTP capability".into(),
                markdown,
                metadata: Vec::new(),
            }));
        }
        if let Some(entry) = input.strip_prefix("capability:kv:") {
            let markdown = entry.split_once('=').map_or_else(
                || "kv:error:invalid entry".to_owned(),
                |(key, value)| {
                    crate::novahub::plugin::capabilities::storage_put(key, value.as_bytes())
                        .and_then(|()| {
                            crate::novahub::plugin::capabilities::storage_get(key)
                        })
                        .map_or_else(
                            |error| format!("kv:error:{error}"),
                            |value| {
                                String::from_utf8(value).map_or_else(
                                    |_| "kv:error:invalid UTF-8".to_owned(),
                                    |value| format!("kv:ok:{value}"),
                                )
                            },
                        )
                },
            );
            return Ok(types::View::Detail(types::DetailView {
                title: "Storage KV capability".into(),
                markdown,
                metadata: Vec::new(),
            }));
        }
        Ok(types::View::Detail(types::DetailView {
            title: "Fixture update".into(),
            markdown: input,
            metadata: Vec::new(),
        }))
    }

    fn run(context: types::CommandContext) -> Result<types::CommandResult, String> {
        Ok(types::CommandResult {
            text: format!("{}: {}", context.command_id, context.input),
            copy_text: None,
        })
    }

    fn close_plugin() {}
}

export!(FixturePlugin);
