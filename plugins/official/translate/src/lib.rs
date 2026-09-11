#![no_main]

wit_bindgen::generate!({
    world: "plugin",
    path: "../../../wit/novahub-plugin",
});

use crate::novahub::plugin::types;

struct Translate;

impl Guest for Translate {
    fn initialize() -> Result<(), String> {
        Ok(())
    }

    fn open_view() -> Result<types::View, String> {
        Ok(types::View::Form(types::FormView {
            title: "Translate".into(),
            fields: vec![
                types::FormField {
                    id: "source_language".into(),
                    label: "Source language".into(),
                    control: types::FormControl::Select,
                    required: true,
                    initial_value: "en".into(),
                    helper: "Choose the input language".into(),
                    error: String::new(),
                    options: vec![
                        types::FormOption {
                            id: "en".into(),
                            label: "English".into(),
                        },
                        types::FormOption {
                            id: "zh".into(),
                            label: "Chinese".into(),
                        },
                    ],
                },
                types::FormField {
                    id: "target_language".into(),
                    label: "Target language".into(),
                    control: types::FormControl::Select,
                    required: true,
                    initial_value: "zh".into(),
                    helper: "Choose the output language".into(),
                    error: String::new(),
                    options: vec![
                        types::FormOption {
                            id: "en".into(),
                            label: "English".into(),
                        },
                        types::FormOption {
                            id: "zh".into(),
                            label: "Chinese".into(),
                        },
                    ],
                },
                types::FormField {
                    id: "text".into(),
                    label: "Text".into(),
                    control: types::FormControl::Text,
                    required: true,
                    initial_value: String::new(),
                    helper: "Enter text to translate".into(),
                    error: String::new(),
                    options: Vec::new(),
                },
            ],
        }))
    }

    fn update(input: String) -> Result<types::View, String> {
        if input.trim().is_empty() {
            return Err("translation input is empty".into());
        }
        Ok(types::View::Detail(types::DetailView {
            title: "Translation".into(),
            markdown: format!("Offline translation preview:\n\n{input}"),
            metadata: Vec::new(),
        }))
    }

    fn run(context: types::CommandContext) -> Result<types::CommandResult, String> {
        if context.command_id != "translate.preview" || context.input.trim().is_empty() {
            return Err("translation input is empty or command is unknown".into());
        }
        Ok(types::CommandResult {
            copy_text: Some(context.input.clone()),
            text: format!("Offline translation preview: {}", context.input),
        })
    }

    fn close_plugin() {}
}

export!(Translate);
