#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use novahub_ui_protocol::{ActionItem, GridItem, ListItem, Metadata, View};
pub use novahub_ui_protocol::{
    ActionRole, FormControl, FormField, FormOption, ItemAccessory, MetadataValue, ResourceRef,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginContext {
    plugin_id: String,
    permissions: BTreeSet<String>,
}

impl PluginContext {
    #[must_use]
    pub fn new(
        plugin_id: impl Into<String>,
        permissions: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            permissions: permissions.into_iter().map(Into::into).collect(),
        }
    }

    #[must_use]
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    #[must_use]
    pub fn has_permission(&self, permission: impl AsRef<str>) -> bool {
        self.permissions.contains(permission.as_ref())
    }

    /// Checks that the host granted a capability before an SDK operation.
    ///
    /// # Errors
    ///
    /// Returns the capability name when it was not granted.
    pub fn require(&self, permission: impl AsRef<str>) -> Result<(), String> {
        if self.has_permission(permission.as_ref()) {
            Ok(())
        } else {
            Err(format!("permission denied: {}", permission.as_ref()))
        }
    }
}

#[must_use]
pub fn list_view(
    title: impl Into<String>,
    items: impl IntoIterator<Item = (String, String, String)>,
) -> View {
    View::list(
        title,
        items
            .into_iter()
            .map(|(id, item_title, subtitle)| ListItem::new(id, item_title, subtitle))
            .collect(),
    )
}

/// Entry point for the SDK's host-owned official View builders.
#[derive(Clone, Copy, Debug, Default)]
pub struct ViewBuilder;

impl ViewBuilder {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    #[must_use]
    pub fn empty(
        &self,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> EmptyViewBuilder {
        EmptyViewBuilder {
            title: title.into(),
            description: description.into(),
        }
    }

    #[must_use]
    pub fn list(&self, title: impl Into<String>) -> ListViewBuilder {
        ListViewBuilder {
            title: title.into(),
            items: Vec::new(),
            next_cursor: None,
        }
    }

    #[must_use]
    pub fn grid(&self, title: impl Into<String>) -> GridViewBuilder {
        GridViewBuilder {
            title: title.into(),
            items: Vec::new(),
            next_cursor: None,
        }
    }

    #[must_use]
    pub fn detail(
        &self,
        title: impl Into<String>,
        markdown: impl Into<String>,
    ) -> DetailViewBuilder {
        DetailViewBuilder {
            title: title.into(),
            markdown: markdown.into(),
            metadata: Vec::new(),
        }
    }

    #[must_use]
    pub fn form(&self, title: impl Into<String>) -> FormViewBuilder {
        FormViewBuilder {
            title: title.into(),
            fields: Vec::new(),
        }
    }

    #[must_use]
    pub fn action_panel(&self, title: impl Into<String>) -> ActionPanelBuilder {
        ActionPanelBuilder {
            title: title.into(),
            actions: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmptyViewBuilder {
    title: String,
    description: String,
}

impl EmptyViewBuilder {
    ///
    /// # Errors
    ///
    /// Returns the host validation error when title or description is blank.
    pub fn build(self) -> Result<View, String> {
        let view = View::Empty {
            title: self.title,
            description: self.description,
        };
        view.validate().map(|()| view)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListViewBuilder {
    title: String,
    items: Vec<ListItem>,
    next_cursor: Option<String>,
}

impl ListViewBuilder {
    #[must_use]
    pub fn next_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.next_cursor = Some(cursor.into());
        self
    }

    #[must_use]
    pub fn item(
        mut self,
        id: impl Into<String>,
        title: impl Into<String>,
        subtitle: impl Into<String>,
    ) -> Self {
        self.items.push(ListItem::new(id, title, subtitle));
        self
    }

    #[must_use]
    pub fn item_with_accessory(
        mut self,
        id: impl Into<String>,
        title: impl Into<String>,
        subtitle: impl Into<String>,
        accessory: ItemAccessory,
    ) -> Self {
        self.items
            .push(ListItem::new(id, title, subtitle).with_accessory(accessory));
        self
    }

    ///
    /// # Errors
    ///
    /// Returns the host validation error when an item ID is invalid or the
    /// list exceeds the bounded page size.
    pub fn build(self) -> Result<View, String> {
        let view = View::list_page(self.title, self.items, self.next_cursor);
        view.validate().map(|()| view)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GridViewBuilder {
    title: String,
    items: Vec<GridItem>,
    next_cursor: Option<String>,
}

impl GridViewBuilder {
    #[must_use]
    pub fn next_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.next_cursor = Some(cursor.into());
        self
    }

    #[must_use]
    pub fn item(
        mut self,
        id: impl Into<String>,
        title: impl Into<String>,
        subtitle: impl Into<String>,
    ) -> Self {
        self.items.push(GridItem::new(id, title, subtitle));
        self
    }

    #[must_use]
    pub fn item_with_accessory(
        mut self,
        id: impl Into<String>,
        title: impl Into<String>,
        subtitle: impl Into<String>,
        accessory: ItemAccessory,
    ) -> Self {
        self.items
            .push(GridItem::new(id, title, subtitle).with_accessory(accessory));
        self
    }

    ///
    /// # Errors
    ///
    /// Returns the host validation error when an item ID is invalid or the
    /// grid exceeds the bounded page size.
    pub fn build(self) -> Result<View, String> {
        let view = View::grid_page(self.title, self.items, self.next_cursor);
        view.validate().map(|()| view)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetailViewBuilder {
    title: String,
    markdown: String,
    metadata: Vec<Metadata>,
}

impl DetailViewBuilder {
    #[must_use]
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.push(Metadata::new(key, value));
        self
    }

    #[must_use]
    pub fn metadata_link(mut self, key: impl Into<String>, url: impl Into<String>) -> Self {
        self.metadata.push(Metadata::link(key, url));
        self
    }

    #[must_use]
    pub fn metadata_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.push(Metadata::tag(key, value));
        self
    }

    ///
    /// # Errors
    ///
    /// Returns the host validation error when the title or Markdown is blank
    /// or exceeds the protocol size limit.
    pub fn build(self) -> Result<View, String> {
        let view = View::detail(self.title, self.markdown, self.metadata);
        view.validate().map(|()| view)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormViewBuilder {
    title: String,
    fields: Vec<FormField>,
}

impl FormViewBuilder {
    #[must_use]
    pub fn field(
        mut self,
        id: impl Into<String>,
        label: impl Into<String>,
        required: bool,
    ) -> Self {
        self.fields.push(FormField::new(id, label, required));
        self
    }

    #[must_use]
    pub fn configured_field(mut self, field: FormField) -> Self {
        self.fields.push(field);
        self
    }

    #[must_use]
    pub fn password_field(
        self,
        id: impl Into<String>,
        label: impl Into<String>,
        required: bool,
        helper: impl Into<String>,
    ) -> Self {
        self.configured_field(
            FormField::new(id, label, required)
                .with_control(FormControl::Password)
                .with_helper(helper),
        )
    }

    #[must_use]
    pub fn select_field(
        self,
        id: impl Into<String>,
        label: impl Into<String>,
        required: bool,
        initial_value: impl Into<String>,
        options: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        self.configured_field(
            FormField::new(id, label, required)
                .with_control(FormControl::Select)
                .with_initial_value(initial_value)
                .with_options(
                    options
                        .into_iter()
                        .map(|(id, label)| FormOption::new(id, label))
                        .collect(),
                ),
        )
    }

    #[must_use]
    pub fn checkbox_field(
        self,
        id: impl Into<String>,
        label: impl Into<String>,
        initial_value: bool,
    ) -> Self {
        self.configured_field(
            FormField::new(id, label, false)
                .with_control(FormControl::Checkbox)
                .with_initial_value(initial_value.to_string()),
        )
    }

    #[must_use]
    pub fn switch_field(
        self,
        id: impl Into<String>,
        label: impl Into<String>,
        initial_value: bool,
    ) -> Self {
        self.configured_field(
            FormField::new(id, label, false)
                .with_control(FormControl::Switch)
                .with_initial_value(initial_value.to_string()),
        )
    }

    ///
    /// # Errors
    ///
    /// Returns the host validation error when a field ID is invalid or the
    /// form exceeds the bounded field count.
    pub fn build(self) -> Result<View, String> {
        let view = View::form(self.title, self.fields);
        view.validate().map(|()| view)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionPanelBuilder {
    title: String,
    actions: Vec<ActionItem>,
}

impl ActionPanelBuilder {
    #[must_use]
    pub fn action(
        mut self,
        id: impl Into<String>,
        title: impl Into<String>,
        destructive: bool,
    ) -> Self {
        let action = if self.actions.is_empty() {
            ActionItem::default_action(id, title, destructive)
        } else {
            ActionItem::new(id, title, destructive)
        };
        self.actions.push(action);
        self
    }

    #[must_use]
    pub fn default_action(
        mut self,
        id: impl Into<String>,
        title: impl Into<String>,
        destructive: bool,
    ) -> Self {
        self.actions
            .push(ActionItem::default_action(id, title, destructive));
        self
    }

    #[must_use]
    pub fn secondary_action(
        mut self,
        id: impl Into<String>,
        title: impl Into<String>,
        destructive: bool,
    ) -> Self {
        self.actions.push(ActionItem::new(id, title, destructive));
        self
    }

    ///
    /// # Errors
    ///
    /// Returns the host validation error when an action ID is invalid or the
    /// panel exceeds the bounded action count.
    pub fn build(self) -> Result<View, String> {
        let view = View::action_panel(self.title, self.actions);
        view.validate().map(|()| view)
    }
}

/// Minimal host double for SDK tests. It validates exactly the same bounded
/// View contract used by the real Shell before retaining the latest view.
#[derive(Clone, Debug, Default)]
pub struct MockHost {
    latest_view: Option<View>,
}

impl MockHost {
    ///
    /// # Errors
    ///
    /// Returns the same bounded View validation error used by the real host.
    pub fn render(&mut self, view: View) -> Result<(), String> {
        view.validate()?;
        self.latest_view = Some(view);
        Ok(())
    }

    #[must_use]
    pub fn latest_view(&self) -> Option<&View> {
        self.latest_view.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::{MockHost, PluginContext, ViewBuilder, list_view};

    #[test]
    fn sdk_builds_host_owned_list_views_and_checks_capabilities() {
        let context = PluginContext::new("demo", ["http"]);
        assert!(context.require("http").is_ok());
        assert!(context.require("storage").is_err());
        let view = list_view("Demo", [("one".into(), "One".into(), String::new())]);
        assert!(view.validate().is_ok());
    }

    #[test]
    fn builders_cover_the_official_view_contract_and_mock_host_validates_it() {
        let builder = ViewBuilder::new();
        let view = builder
            .detail("Result", "## Done")
            .metadata("source", "fixture")
            .metadata_link("docs", "https://example.com/docs")
            .metadata_tag("format", "json")
            .build()
            .expect("detail view");
        let mut host = MockHost::default();
        host.render(view.clone()).expect("mock host accepts view");
        assert_eq!(host.latest_view(), Some(&view));

        assert!(builder.empty("Empty", "Nothing here").build().is_ok());
        assert!(
            builder
                .list("List")
                .item("one", "One", "First")
                .next_cursor("opaque-page-2")
                .item_with_accessory(
                    "json",
                    "JSON",
                    "Format JSON",
                    super::ItemAccessory::new("Ready", "JSON", "Enter")
                        .with_icon(super::ResourceRef::new("icon.json")),
                )
                .build()
                .is_ok()
        );
        assert!(
            builder
                .grid("Grid")
                .item("one", "One", "First")
                .build()
                .is_ok()
        );
        assert!(
            builder
                .form("Form")
                .field("query", "Query", true)
                .password_field("token", "Token", true, "Stored only for this request")
                .select_field(
                    "language",
                    "Language",
                    true,
                    "en",
                    [
                        ("en".into(), "English".into()),
                        ("zh".into(), "Chinese".into())
                    ],
                )
                .checkbox_field("remember", "Remember selection", false)
                .switch_field("compact", "Compact mode", true)
                .build()
                .is_ok()
        );
        assert!(
            builder
                .action_panel("Actions")
                .default_action("copy", "Copy", false)
                .secondary_action("delete", "Delete", true)
                .build()
                .is_ok()
        );
    }

    #[test]
    fn builders_reuse_protocol_limits_instead_of_allowing_unbounded_views() {
        let mut builder = ViewBuilder::new().list("Results");
        for index in 0..101 {
            builder = builder.item(index.to_string(), "Item", "");
        }
        assert!(builder.build().is_err());
    }
}
