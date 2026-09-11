#![forbid(unsafe_code)]

pub mod json_toolkit {
    use novahub_ui_protocol::View;

    /// Formats JSON locally and returns a host-owned detail view.
    ///
    /// # Errors
    ///
    /// Returns a parse diagnostic without executing input.
    pub fn format(input: &str) -> Result<View, String> {
        let value: serde_json::Value =
            serde_json::from_str(input).map_err(|error| error.to_string())?;
        let formatted = serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
        Ok(View::detail("JSON Toolkit", &formatted, Vec::new()))
    }
}

pub mod translate {
    use novahub_ui_protocol::{FormField, View};

    #[must_use]
    pub fn form() -> View {
        View::form(
            "Translate",
            vec![
                FormField::new("source_language", "Source language", true),
                FormField::new("target_language", "Target language", true),
                FormField::new("text", "Text", true),
            ],
        )
    }

    #[must_use]
    pub fn offline_result(source: &str, target: &str, text: &str) -> View {
        View::detail(
            "Translation",
            format!("**{source} → {target}**\n\n{text}"),
            Vec::new(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{json_toolkit, translate};

    #[test]
    fn json_plugin_returns_valid_detail_view_and_rejects_invalid_input() {
        assert!(
            json_toolkit::format("{\"ok\":true}")
                .expect("valid JSON view")
                .validate()
                .is_ok()
        );
        assert!(json_toolkit::format("not json").is_err());
    }

    #[test]
    fn translation_plugin_returns_valid_form_and_result_views() {
        assert!(translate::form().validate().is_ok());
        assert!(
            translate::offline_result("en", "zh", "你好")
                .validate()
                .is_ok()
        );
    }
}
