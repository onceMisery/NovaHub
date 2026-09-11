#![no_main]

wit_bindgen::generate!({
    world: "plugin",
    path: "../../../wit/novahub-plugin",
});

use crate::novahub::plugin::types;

struct JsonToolkit;

impl Guest for JsonToolkit {
    fn initialize() -> Result<(), String> {
        Ok(())
    }

    fn open_view() -> Result<types::View, String> {
        Ok(types::View::List(types::ListView {
            title: "JSON Toolkit".into(),
            items: vec![types::ListItem {
                id: "json.format".into(),
                title: "Format JSON".into(),
                subtitle: "Validate and pretty-print locally".into(),
                accessory: Some(types::ItemAccessory {
                    status: "Ready".into(),
                    badge: "JSON".into(),
                    shortcut: "Enter".into(),
                    icon: None,
                }),
            }],
            next_cursor: None,
        }))
    }

    fn update(input: String) -> Result<types::View, String> {
        let value: serde_json::Value =
            serde_json::from_str(&input).map_err(|error| error.to_string())?;
        let markdown = serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
        Ok(types::View::Detail(types::DetailView {
            title: "Formatted JSON".into(),
            markdown,
            metadata: vec![types::Metadata {
                key: "format".into(),
                value: types::MetadataValue::Tag("json".into()),
            }],
        }))
    }

    fn run(context: types::CommandContext) -> Result<types::CommandResult, String> {
        if context.command_id != "json.format" {
            return Err("unknown JSON Toolkit command".into());
        }
        let value: serde_json::Value =
            serde_json::from_str(&context.input).map_err(|error| error.to_string())?;
        let text = serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
        Ok(types::CommandResult {
            copy_text: Some(text.clone()),
            text,
        })
    }

    fn close_plugin() {}
}

export!(JsonToolkit);
