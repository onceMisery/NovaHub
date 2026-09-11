const MAX_RECOMMENDED_ACTIONS: usize = 3;
const MAX_FIXED_ACTIONS: usize = 5;
const MAX_VISIBLE_ACTIONS: usize = 5;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PetAction {
    pub id: String,
    pub title: String,
    pub target: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PetActionState {
    Available,
    Disabled(String),
}

/// Host-owned action shelf. Recommendations never become fixed implicitly.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PetActionShelf {
    recommended: Vec<PetAction>,
    fixed: Vec<PetAction>,
}

impl PetActionShelf {
    pub fn set_recommended(&mut self, mut actions: Vec<PetAction>) {
        actions.truncate(MAX_RECOMMENDED_ACTIONS);
        self.recommended = actions;
    }

    #[must_use]
    pub fn recommended(&self) -> &[PetAction] {
        &self.recommended
    }

    #[must_use]
    pub fn fixed(&self) -> &[PetAction] {
        &self.fixed
    }

    /// Returns the bounded actions displayed by the pet shelf.
    ///
    /// Fixed actions take precedence; recommendations fill remaining slots
    /// without being copied into the fixed list.
    #[must_use]
    pub fn visible_actions(&self) -> Vec<PetAction> {
        let mut actions = self.fixed.clone();
        for recommendation in &self.recommended {
            if actions.len() >= MAX_VISIBLE_ACTIONS {
                break;
            }
            if !actions.iter().any(|action| action.id == recommendation.id) {
                actions.push(recommendation.clone());
            }
        }
        actions
    }

    /// Pins a host-resolved target; duplicate IDs and disabled targets are not
    /// allowed to enter the persisted fixed list.
    ///
    /// # Errors
    ///
    /// Returns an error when the target is unavailable or the shelf already
    /// contains five fixed actions.
    pub fn pin(&mut self, action: PetAction, state: &PetActionState) -> Result<(), &'static str> {
        if !matches!(state, PetActionState::Available) {
            return Err("pet action target is unavailable");
        }
        if self.fixed.iter().any(|item| item.id == action.id) {
            return Ok(());
        }
        if self.fixed.len() >= MAX_FIXED_ACTIONS {
            return Err("pet action shelf is full");
        }
        self.fixed.push(action);
        Ok(())
    }

    pub fn unpin(&mut self, id: &str) -> bool {
        let before = self.fixed.len();
        self.fixed.retain(|action| action.id != id);
        self.fixed.len() != before
    }

    #[must_use]
    pub fn state_for(&self, id: &str, target_available: bool) -> Option<PetActionState> {
        self.fixed
            .iter()
            .chain(self.recommended.iter())
            .find(|action| action.id == id)
            .map(|_| {
                if target_available {
                    PetActionState::Available
                } else {
                    PetActionState::Disabled("目标已禁用或已卸载".into())
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{PetAction, PetActionShelf, PetActionState};

    fn action(id: &str) -> PetAction {
        PetAction {
            id: id.into(),
            title: id.into(),
            target: format!("builtin/{id}"),
        }
    }

    #[test]
    fn recommendations_are_bounded_without_becoming_fixed() {
        let mut shelf = PetActionShelf::default();
        shelf.set_recommended((0..5).map(|index| action(&index.to_string())).collect());
        assert_eq!(shelf.recommended().len(), 3);
        assert!(shelf.fixed().is_empty());
    }

    #[test]
    fn fixed_actions_require_availability_and_are_bounded() {
        let mut shelf = PetActionShelf::default();
        assert!(
            shelf
                .pin(
                    action("missing"),
                    &PetActionState::Disabled("missing".into())
                )
                .is_err()
        );
        for index in 0..5 {
            shelf
                .pin(action(&index.to_string()), &PetActionState::Available)
                .expect("pin action");
        }
        assert!(
            shelf
                .pin(action("overflow"), &PetActionState::Available)
                .is_err()
        );
        assert!(shelf.unpin("2"));
        assert_eq!(shelf.fixed().len(), 4);
    }

    #[test]
    fn unavailable_target_exposes_a_recovery_reason() {
        let mut shelf = PetActionShelf::default();
        shelf
            .pin(action("calculator"), &PetActionState::Available)
            .expect("pin action");
        assert_eq!(
            shelf.state_for("calculator", false),
            Some(PetActionState::Disabled("目标已禁用或已卸载".into()))
        );
    }

    #[test]
    fn visible_actions_prioritize_fixed_items_and_deduplicate_recommendations() {
        let mut shelf = PetActionShelf::default();
        shelf.set_recommended(vec![action("a"), action("b"), action("c")]);
        shelf
            .pin(action("b"), &PetActionState::Available)
            .expect("pin");
        assert_eq!(
            shelf
                .visible_actions()
                .into_iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            vec!["b", "a", "c"]
        );
    }
}
