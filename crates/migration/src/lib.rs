#![forbid(unsafe_code)]

use serde::Deserialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceKind {
    UTools,
    Raycast,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationItem {
    pub source: SourceKind,
    pub kind: String,
    pub title: String,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RejectedItem {
    pub title: String,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationReport {
    pub accepted: Vec<MigrationItem>,
    pub rejected: Vec<RejectedItem>,
}

#[derive(Deserialize)]
struct RawExport {
    #[serde(default)]
    items: Vec<RawItem>,
}

#[derive(Deserialize)]
struct RawItem {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    script: Option<String>,
    #[serde(default)]
    credential: Option<String>,
}

/// Parses a user-selected JSON export without executing imported content.
///
/// # Errors
///
/// Returns a JSON syntax error when the input cannot be decoded.
pub fn parse_export(input: &str, source: SourceKind) -> Result<MigrationReport, serde_json::Error> {
    let export: RawExport = serde_json::from_str(input)?;
    let mut report = MigrationReport {
        accepted: Vec::new(),
        rejected: Vec::new(),
    };
    for item in export.items {
        let kind = item.kind.to_ascii_lowercase();
        let forbidden = matches!(kind.as_str(), "script" | "credential")
            || item.script.is_some()
            || item.credential.is_some();
        if forbidden {
            report.rejected.push(RejectedItem {
                title: item.title,
                reason: "item is not imported because scripts and credentials are forbidden".into(),
            });
        } else if matches!(kind.as_str(), "quicklink" | "snippet") {
            report.accepted.push(MigrationItem {
                source,
                kind,
                title: item.title,
                content: item.content,
            });
        } else {
            report.rejected.push(RejectedItem {
                title: item.title,
                reason: "item kind is not supported by the MVP importer".into(),
            });
        }
    }
    Ok(report)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationStep {
    pub version: u32,
    pub sql: String,
}

impl MigrationStep {
    pub fn new(version: u32, sql: impl Into<String>) -> Self {
        Self {
            version,
            sql: sql.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationPlan {
    steps: Vec<MigrationStep>,
}

impl MigrationPlan {
    #[must_use]
    pub fn new(steps: Vec<MigrationStep>) -> Self {
        Self { steps }
    }

    /// Checks that migration versions are contiguous and start at one.
    ///
    /// # Errors
    ///
    /// Returns an error if a version is missing or out of order.
    pub fn validate(&self) -> Result<(), String> {
        for (index, step) in self.steps.iter().enumerate() {
            let expected = u32::try_from(index + 1).map_err(|_| "too many migrations")?;
            if step.version != expected {
                return Err(format!(
                    "migration version {} is not expected version {expected}",
                    step.version
                ));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn up_sql(&self, from_version: u32) -> Vec<&str> {
        let latest = u32::try_from(self.steps.len()).unwrap_or(u32::MAX);
        if from_version >= latest {
            return Vec::new();
        }
        self.steps
            .iter()
            .filter(|step| step.version > from_version)
            .map(|step| step.sql.as_str())
            .collect()
    }

    #[must_use]
    pub fn down_sql(&self, to_version: u32) -> Vec<&str> {
        self.steps
            .iter()
            .rev()
            .filter(|step| step.version <= to_version)
            .map(|step| step.sql.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{MigrationPlan, MigrationStep, SourceKind, parse_export};

    #[test]
    fn migrations_apply_in_order_and_rollback_in_reverse() {
        let plan = MigrationPlan::new(vec![
            MigrationStep::new(1, "create settings"),
            MigrationStep::new(2, "create commands"),
        ]);
        assert_eq!(plan.up_sql(0), ["create settings", "create commands"]);
        assert_eq!(plan.down_sql(2), ["create commands", "create settings"]);
    }

    #[test]
    fn migration_plan_rejects_gaps_and_unknown_versions() {
        let plan = MigrationPlan::new(vec![MigrationStep::new(2, "bad gap")]);
        assert!(plan.validate().is_err());
        assert!(plan.up_sql(3).is_empty());
    }

    #[test]
    fn parser_keeps_safe_items_and_rejects_scripts_and_credentials() {
        let input = r#"{
          "items": [
            {"kind":"quicklink","title":"Docs","content":"https://example.com"},
            {"kind":"snippet","title":"Greeting","content":"Hello"},
            {"kind":"script","title":"Run","content":"danger"},
            {"kind":"credential","title":"Token","content":"secret"}
          ]
        }"#;
        let report = parse_export(input, SourceKind::Raycast).expect("valid export");
        assert_eq!(report.accepted.len(), 2);
        assert_eq!(report.rejected.len(), 2);
        assert!(
            report
                .rejected
                .iter()
                .all(|item| item.reason.contains("not"))
        );
    }

    #[test]
    fn parser_rejects_malformed_json() {
        assert!(parse_export("not json", SourceKind::UTools).is_err());
    }
}
