#![forbid(unsafe_code)]

const MAX_ACTION_ITEMS: usize = 6;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum View {
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
        items: Vec<ListItem>,
        next_cursor: Option<String>,
    },
    Grid {
        title: String,
        items: Vec<GridItem>,
        next_cursor: Option<String>,
    },
    Detail {
        title: String,
        markdown: String,
        metadata: Vec<Metadata>,
    },
    Form {
        title: String,
        fields: Vec<FormField>,
    },
    ActionPanel {
        title: String,
        actions: Vec<ActionItem>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceRef {
    pub id: String,
}

impl ResourceRef {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemAccessory {
    pub status: String,
    pub badge: String,
    pub shortcut: String,
    pub icon: Option<ResourceRef>,
}

impl ItemAccessory {
    pub fn new(
        status: impl Into<String>,
        badge: impl Into<String>,
        shortcut: impl Into<String>,
    ) -> Self {
        Self {
            status: status.into(),
            badge: badge.into(),
            shortcut: shortcut.into(),
            icon: None,
        }
    }

    #[must_use]
    pub fn with_icon(mut self, icon: ResourceRef) -> Self {
        self.icon = Some(icon);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListItem {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub accessory: Option<ItemAccessory>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GridItem {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub accessory: Option<ItemAccessory>,
}

impl GridItem {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        subtitle: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: subtitle.into(),
            accessory: None,
        }
    }

    #[must_use]
    pub fn with_accessory(mut self, accessory: ItemAccessory) -> Self {
        self.accessory = Some(accessory);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Metadata {
    pub key: String,
    pub value: MetadataValue,
}

impl Metadata {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: MetadataValue::Text(value.into()),
        }
    }

    #[must_use]
    pub fn link(key: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: MetadataValue::Link(url.into()),
        }
    }

    #[must_use]
    pub fn tag(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: MetadataValue::Tag(value.into()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetadataValue {
    Text(String),
    Link(String),
    Tag(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormField {
    pub id: String,
    pub label: String,
    pub control: FormControl,
    pub required: bool,
    pub initial_value: String,
    pub helper: String,
    pub error: String,
    pub options: Vec<FormOption>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FormControl {
    #[default]
    Text,
    Password,
    Select,
    Checkbox,
    Switch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormOption {
    pub id: String,
    pub label: String,
}

impl FormOption {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

impl FormField {
    pub fn new(id: impl Into<String>, label: impl Into<String>, required: bool) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            control: FormControl::Text,
            required,
            initial_value: String::new(),
            helper: String::new(),
            error: String::new(),
            options: Vec::new(),
        }
    }

    #[must_use]
    pub const fn with_control(mut self, control: FormControl) -> Self {
        self.control = control;
        self
    }

    #[must_use]
    pub fn with_initial_value(mut self, value: impl Into<String>) -> Self {
        self.initial_value = value.into();
        self
    }

    #[must_use]
    pub fn with_helper(mut self, helper: impl Into<String>) -> Self {
        self.helper = helper.into();
        self
    }

    #[must_use]
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = error.into();
        self
    }

    #[must_use]
    pub fn with_options(mut self, options: Vec<FormOption>) -> Self {
        self.options = options;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionItem {
    pub id: String,
    pub title: String,
    pub role: ActionRole,
    pub destructive: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionRole {
    Default,
    Secondary,
}

impl ActionItem {
    pub fn new(id: impl Into<String>, title: impl Into<String>, destructive: bool) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            role: ActionRole::Secondary,
            destructive,
        }
    }

    pub fn default_action(
        id: impl Into<String>,
        title: impl Into<String>,
        destructive: bool,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            role: ActionRole::Default,
            destructive,
        }
    }
}

impl ListItem {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        subtitle: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: subtitle.into(),
            accessory: None,
        }
    }

    #[must_use]
    pub fn with_accessory(mut self, accessory: ItemAccessory) -> Self {
        self.accessory = Some(accessory);
        self
    }
}

impl View {
    pub fn list(title: impl Into<String>, items: Vec<ListItem>) -> Self {
        Self::List {
            title: title.into(),
            items,
            next_cursor: None,
        }
    }

    pub fn list_page(
        title: impl Into<String>,
        items: Vec<ListItem>,
        next_cursor: Option<String>,
    ) -> Self {
        Self::List {
            title: title.into(),
            items,
            next_cursor,
        }
    }

    #[must_use]
    pub fn loading(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Loading {
            title: title.into(),
            message: message.into(),
        }
    }

    #[must_use]
    pub fn progress(title: impl Into<String>, completed: u64, total: u64) -> Self {
        Self::Progress {
            title: title.into(),
            completed,
            total,
        }
    }

    #[must_use]
    pub fn error(title: impl Into<String>, message: impl Into<String>, recoverable: bool) -> Self {
        Self::Error {
            title: title.into(),
            message: message.into(),
            recoverable,
        }
    }

    pub fn grid(title: impl Into<String>, items: Vec<GridItem>) -> Self {
        Self::Grid {
            title: title.into(),
            items,
            next_cursor: None,
        }
    }

    pub fn grid_page(
        title: impl Into<String>,
        items: Vec<GridItem>,
        next_cursor: Option<String>,
    ) -> Self {
        Self::Grid {
            title: title.into(),
            items,
            next_cursor,
        }
    }

    pub fn detail(
        title: impl Into<String>,
        markdown: impl Into<String>,
        metadata: Vec<Metadata>,
    ) -> Self {
        Self::Detail {
            title: title.into(),
            markdown: markdown.into(),
            metadata,
        }
    }

    pub fn form(title: impl Into<String>, fields: Vec<FormField>) -> Self {
        Self::Form {
            title: title.into(),
            fields,
        }
    }

    pub fn action_panel(title: impl Into<String>, actions: Vec<ActionItem>) -> Self {
        Self::ActionPanel {
            title: title.into(),
            actions,
        }
    }

    /// Validates host-owned limits and stable IDs before rendering.
    ///
    /// # Errors
    ///
    /// Returns a message when text is blank, the list exceeds 100 items, or
    /// item IDs are empty or duplicated.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Empty { title, description } => {
                if title.trim().is_empty() || description.trim().is_empty() {
                    return Err("empty view text must not be blank".into());
                }
            }
            Self::Loading { title, message } => {
                if title.trim().is_empty() || message.trim().is_empty() {
                    return Err("loading view text must not be blank".into());
                }
            }
            Self::Progress {
                title,
                completed,
                total,
            } => {
                if title.trim().is_empty() || *total == 0 || *completed > *total {
                    return Err("progress view values are invalid".into());
                }
            }
            Self::Error { title, message, .. } => {
                if title.trim().is_empty() || message.trim().is_empty() {
                    return Err("error view text must not be blank".into());
                }
            }
            Self::List {
                title,
                items,
                next_cursor,
            } => {
                if title.trim().is_empty() {
                    return Err("list title must not be blank".into());
                }
                if items.len() > 100 {
                    return Err("list contains more than 100 items".into());
                }
                let mut ids = std::collections::HashSet::with_capacity(items.len());
                for item in items {
                    if item.id.trim().is_empty() || !ids.insert(&item.id) {
                        return Err("list item IDs must be non-empty and unique".into());
                    }
                    validate_item_accessory(item.accessory.as_ref())?;
                }
                validate_next_cursor(next_cursor.as_deref())?;
            }
            Self::Grid {
                title,
                items,
                next_cursor,
            } => {
                if title.trim().is_empty() || items.len() > 100 {
                    return Err("grid title or item limit is invalid".into());
                }
                let mut ids = std::collections::HashSet::with_capacity(items.len());
                if items
                    .iter()
                    .any(|item| item.id.trim().is_empty() || !ids.insert(&item.id))
                {
                    return Err("grid item IDs must be non-empty and unique".into());
                }
                for item in items {
                    validate_item_accessory(item.accessory.as_ref())?;
                }
                validate_next_cursor(next_cursor.as_deref())?;
            }
            Self::Detail {
                title,
                markdown,
                metadata,
            } => validate_detail_view(title, markdown, metadata)?,
            Self::Form { title, fields } => {
                if title.trim().is_empty() || fields.len() > 100 {
                    return Err("form title or field limit is invalid".into());
                }
                let mut ids = std::collections::HashSet::with_capacity(fields.len());
                for field in fields {
                    if field.id.trim().is_empty() || !ids.insert(&field.id) {
                        return Err("form field IDs must be non-empty and unique".into());
                    }
                    validate_form_field(field)?;
                }
            }
            Self::ActionPanel { title, actions } => {
                validate_action_panel(title, actions)?;
            }
        }
        Ok(())
    }
}

fn validate_action_panel(title: &str, actions: &[ActionItem]) -> Result<(), String> {
    if title.trim().is_empty() || actions.is_empty() || actions.len() > MAX_ACTION_ITEMS {
        return Err("action panel title or item limit is invalid".into());
    }
    let mut ids = std::collections::HashSet::with_capacity(actions.len());
    if actions.iter().any(|action| {
        action.id.trim().is_empty()
            || action.title.trim().is_empty()
            || action.title.chars().count() > 160
            || action.title.chars().any(char::is_control)
            || !ids.insert(&action.id)
    }) {
        return Err("action IDs and titles must be bounded, non-empty and unique".into());
    }
    if actions
        .iter()
        .filter(|action| action.role == ActionRole::Default)
        .count()
        != 1
    {
        return Err("action panel must contain exactly one default action".into());
    }
    Ok(())
}

fn validate_detail_view(title: &str, markdown: &str, metadata: &[Metadata]) -> Result<(), String> {
    if title.trim().is_empty() || markdown.trim().is_empty() || markdown.len() > (1 << 20) {
        return Err("detail content is invalid or too large".into());
    }
    if metadata.len() > 100 {
        return Err("detail metadata exceeds 100 entries".into());
    }
    let mut keys = std::collections::HashSet::with_capacity(metadata.len());
    for item in metadata {
        if item.key.trim().is_empty() || item.key.chars().count() > 96 || !keys.insert(&item.key) {
            return Err("detail metadata keys must be non-empty, bounded and unique".into());
        }
        validate_metadata_value(&item.value)?;
    }
    Ok(())
}

fn validate_item_accessory(accessory: Option<&ItemAccessory>) -> Result<(), String> {
    let Some(accessory) = accessory else {
        return Ok(());
    };
    for (value, limit) in [
        (&accessory.status, 96),
        (&accessory.badge, 64),
        (&accessory.shortcut, 32),
    ] {
        if value.chars().count() > limit || value.chars().any(char::is_control) {
            return Err("item accessory text is invalid or too large".into());
        }
    }
    if accessory.status.trim().is_empty()
        && accessory.badge.trim().is_empty()
        && accessory.shortcut.trim().is_empty()
        && accessory.icon.is_none()
    {
        return Err("item accessory must contain visible content".into());
    }
    if let Some(icon) = &accessory.icon
        && (icon.id.trim().is_empty()
            || icon.id.chars().count() > 128
            || icon.id.chars().any(char::is_control))
    {
        return Err("item accessory icon ID is invalid".into());
    }
    Ok(())
}

fn validate_next_cursor(cursor: Option<&str>) -> Result<(), String> {
    if let Some(cursor) = cursor
        && (cursor.trim().is_empty()
            || cursor.chars().count() > 256
            || cursor.chars().any(char::is_control))
    {
        return Err(
            "pagination cursor is blank, contains controls or exceeds 256 characters".into(),
        );
    }
    Ok(())
}

fn validate_metadata_value(value: &MetadataValue) -> Result<(), String> {
    let (text, limit) = match value {
        MetadataValue::Text(text) => (text, 512),
        MetadataValue::Link(url) => {
            if !url.starts_with("https://") {
                return Err("metadata links must use https".into());
            }
            (url, 2_048)
        }
        MetadataValue::Tag(tag) => (tag, 96),
    };
    if text.trim().is_empty() || text.chars().count() > limit || text.chars().any(char::is_control)
    {
        return Err("metadata value is blank, contains controls or exceeds its limit".into());
    }
    Ok(())
}

fn validate_form_field(field: &FormField) -> Result<(), String> {
    if field.label.trim().is_empty()
        || field.initial_value.chars().count() > 4_096
        || field.helper.chars().count() > 512
        || field.error.chars().count() > 512
    {
        return Err("form field text is invalid or too large".into());
    }
    if field.control == FormControl::Select {
        if field.options.is_empty() || field.options.len() > 32 {
            return Err("select field must contain between 1 and 32 options".into());
        }
        let mut option_ids = std::collections::HashSet::with_capacity(field.options.len());
        let mut option_labels = std::collections::HashSet::with_capacity(field.options.len());
        for option in &field.options {
            if option.id.trim().is_empty()
                || option.label.trim().is_empty()
                || !option_ids.insert(&option.id)
                || !option_labels.insert(&option.label)
            {
                return Err("select option IDs and labels must be non-empty and unique".into());
            }
        }
        if !field.initial_value.is_empty()
            && !field
                .options
                .iter()
                .any(|option| option.id == field.initial_value)
        {
            return Err("select initial value must reference an option ID".into());
        }
    } else if !field.options.is_empty() {
        return Err("only select fields may contain options".into());
    }
    if matches!(field.control, FormControl::Checkbox | FormControl::Switch)
        && !matches!(field.initial_value.as_str(), "" | "true" | "false")
    {
        return Err("toggle initial value must be true or false".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ActionItem, FormControl, FormField, FormOption, GridItem, ItemAccessory, ListItem,
        Metadata, ResourceRef, View,
    };

    #[test]
    fn list_view_accepts_unique_stable_ids() {
        let view = View::list(
            "Results",
            vec![ListItem::new("open", "Open", "Open a file")],
        );
        assert!(view.validate().is_ok());
    }

    #[test]
    fn list_accessory_preserves_bounded_host_owned_semantics() {
        let view = View::list(
            "Results",
            vec![
                ListItem::new("json", "JSON", "Format JSON").with_accessory(
                    ItemAccessory::new("Ready", "JSON", "Enter")
                        .with_icon(ResourceRef::new("icon.json")),
                ),
            ],
        );
        assert!(view.validate().is_ok());

        let invalid = View::list(
            "Results",
            vec![
                ListItem::new("json", "JSON", "Format JSON")
                    .with_accessory(ItemAccessory::new("", "", "")),
            ],
        );
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn list_view_rejects_duplicate_ids_and_more_than_one_hundred_items() {
        let duplicate = View::list(
            "Results",
            vec![
                ListItem::new("same", "A", ""),
                ListItem::new("same", "B", ""),
            ],
        );
        assert!(duplicate.validate().is_err());

        let too_many = View::list(
            "Results",
            (0..101)
                .map(|index| ListItem::new(index.to_string(), "Item", ""))
                .collect(),
        );
        assert!(too_many.validate().is_err());
    }

    #[test]
    fn cursor_pagination_accepts_only_bounded_opaque_values() {
        let page = View::list_page(
            "Results",
            vec![ListItem::new("one", "One", "First page")],
            Some("opaque-page-2".into()),
        );
        assert!(page.validate().is_ok());

        let blank = View::grid_page("Grid", Vec::new(), Some("  ".into()));
        assert!(blank.validate().is_err());
        let oversized = View::list_page("Results", Vec::new(), Some("x".repeat(257)));
        assert!(oversized.validate().is_err());
        let control = View::list_page("Results", Vec::new(), Some("page\n2".into()));
        assert!(control.validate().is_err());
    }

    #[test]
    fn official_views_validate_stable_ids_and_form_fields() {
        let grid = View::grid("Tools", vec![GridItem::new("json", "JSON", "Format JSON")]);
        assert!(grid.validate().is_ok());

        let detail = View::detail(
            "JSON",
            "## Result",
            vec![
                Metadata::new("status", "valid"),
                Metadata::link("source", "https://example.com/source"),
                Metadata::tag("format", "json"),
            ],
        );
        assert!(detail.validate().is_ok());

        let form = View::form("Translate", vec![FormField::new("source", "Source", true)]);
        assert!(form.validate().is_ok());

        let actions = View::action_panel(
            "Actions",
            vec![ActionItem::default_action("copy", "Copy", false)],
        );
        assert!(actions.validate().is_ok());
    }

    #[test]
    fn detail_metadata_rejects_duplicate_keys_and_unsafe_links() {
        let duplicate = View::detail(
            "Detail",
            "Content",
            vec![
                Metadata::new("status", "valid"),
                Metadata::tag("status", "ok"),
            ],
        );
        assert!(duplicate.validate().is_err());

        let unsafe_link = View::detail(
            "Detail",
            "Content",
            vec![Metadata::link("source", "http://example.com")],
        );
        assert!(unsafe_link.validate().is_err());
    }

    #[test]
    fn official_views_reject_duplicate_ids() {
        let view = View::action_panel(
            "Actions",
            vec![
                ActionItem::new("same", "A", false),
                ActionItem::new("same", "B", false),
            ],
        );
        assert!(view.validate().is_err());
    }

    #[test]
    fn action_panel_requires_exactly_one_default_action() {
        let valid = View::action_panel(
            "Actions",
            vec![
                ActionItem::default_action("open", "Open", false),
                ActionItem::new("delete", "Delete", true),
            ],
        );
        assert!(valid.validate().is_ok());

        let missing = View::action_panel("Actions", vec![ActionItem::new("open", "Open", false)]);
        assert!(missing.validate().is_err());
        let duplicate = View::action_panel(
            "Actions",
            vec![
                ActionItem::default_action("open", "Open", false),
                ActionItem::default_action("copy", "Copy", false),
            ],
        );
        assert!(duplicate.validate().is_err());

        let too_many = View::action_panel(
            "Actions",
            (0..7)
                .map(|index| {
                    if index == 0 {
                        ActionItem::default_action("0", "Default", false)
                    } else {
                        ActionItem::new(index.to_string(), "Secondary", false)
                    }
                })
                .collect(),
        );
        assert!(too_many.validate().is_err());
    }

    #[test]
    fn form_controls_validate_options_initial_values_and_bounds() {
        let select = FormField::new("language", "Language", true)
            .with_control(FormControl::Select)
            .with_initial_value("en")
            .with_helper("Choose a source language")
            .with_options(vec![
                FormOption::new("en", "English"),
                FormOption::new("zh", "Chinese"),
            ]);
        assert!(View::form("Translate", vec![select]).validate().is_ok());

        let invalid = FormField::new("language", "Language", true)
            .with_control(FormControl::Select)
            .with_initial_value("missing")
            .with_options(vec![FormOption::new("en", "English")]);
        assert!(View::form("Translate", vec![invalid]).validate().is_err());

        let invalid_toggle = FormField::new("enabled", "Enabled", false)
            .with_control(FormControl::Switch)
            .with_initial_value("yes");
        assert!(
            View::form("Settings", vec![invalid_toggle])
                .validate()
                .is_err()
        );
    }

    #[test]
    fn view_states_are_bounded_and_reject_invalid_progress() {
        assert!(
            View::loading("Loading", "Fetching results")
                .validate()
                .is_ok()
        );
        assert!(View::progress("Progress", 2, 4).validate().is_ok());
        assert!(
            View::error("Failed", "Retry the request", true)
                .validate()
                .is_ok()
        );
        assert!(View::progress("Progress", 5, 4).validate().is_err());
        assert!(View::progress("Progress", 0, 0).validate().is_err());
    }
}
