#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemCommand {
    OpenSettings,
    Lock,
    Sleep,
    Logout,
    Restart,
}

impl SystemCommand {
    #[must_use]
    pub fn parse(id: &str) -> Option<Self> {
        match id {
            "system.settings" => Some(Self::OpenSettings),
            "system.lock" => Some(Self::Lock),
            "system.sleep" => Some(Self::Sleep),
            "system.logout" => Some(Self::Logout),
            "system.restart" => Some(Self::Restart),
            _ => None,
        }
    }

    #[must_use]
    pub const fn requires_confirmation(self) -> bool {
        matches!(self, Self::Logout | Self::Restart)
    }
}

#[cfg(test)]
mod tests {
    use super::SystemCommand;

    #[test]
    fn destructive_system_commands_require_host_confirmation() {
        assert!(
            !SystemCommand::parse("system.settings")
                .expect("settings command")
                .requires_confirmation()
        );
        assert!(
            SystemCommand::parse("system.restart")
                .expect("restart command")
                .requires_confirmation()
        );
        assert!(SystemCommand::parse("system.unknown").is_none());
    }
}
