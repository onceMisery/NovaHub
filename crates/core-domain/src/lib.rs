#![forbid(unsafe_code)]

pub mod pet;
pub mod pet_position;

/// Stable identifier for a searchable command.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CommandId(String);

impl CommandId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Minimal command descriptor shared by providers and the future UI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandDescriptor {
    pub id: CommandId,
    pub title: String,
    pub subtitle: String,
}

/// Stable identifier for a normalized user query.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct QueryId(String);

impl QueryId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// User input after the host applies lightweight normalization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Query {
    id: QueryId,
    text: String,
}

impl Query {
    pub fn new(value: impl AsRef<str>) -> Self {
        let text = value.as_ref().trim().to_owned();
        Self {
            id: QueryId::new(text.clone()),
            text,
        }
    }

    #[must_use]
    pub fn id(&self) -> &QueryId {
        &self.id
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Host-owned command invocation request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Action {
    command_id: CommandId,
    arguments: Vec<String>,
}

impl Action {
    pub fn invoke<I, S>(command_id: impl Into<String>, arguments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            command_id: CommandId::new(command_id),
            arguments: arguments.into_iter().map(Into::into).collect(),
        }
    }

    #[must_use]
    pub fn command_id(&self) -> &CommandId {
        &self.command_id
    }

    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, Query, QueryId};

    #[test]
    fn query_trims_input_and_has_stable_identity() {
        let query = Query::new("  clipboard  ");
        assert_eq!(query.text(), "clipboard");
        assert_eq!(query.id(), &QueryId::new("clipboard"));
    }

    #[test]
    fn action_keeps_command_identity_and_arguments() {
        let action = Action::invoke("system.open", ["https://example.com"]);
        assert_eq!(action.command_id().as_str(), "system.open");
        assert_eq!(action.arguments(), &["https://example.com"]);
    }
}
