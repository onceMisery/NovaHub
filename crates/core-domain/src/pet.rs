#![forbid(unsafe_code)]

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PetId(String);

impl PetId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PetState {
    Hidden,
    Visible,
    Dragging,
    Shelf,
    Working,
    Success,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PetEvent {
    Show,
    Hide,
    DragStart,
    DragEnd,
    OpenShelf,
    StartAction,
    ActionSucceeded,
    ActionFailed,
    Dismiss,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PetController {
    active_pet: PetId,
    state: PetState,
}

impl PetController {
    #[must_use]
    pub fn new(active_pet: PetId) -> Self {
        Self {
            active_pet,
            state: PetState::Hidden,
        }
    }

    #[must_use]
    pub fn state(&self) -> PetState {
        self.state
    }

    #[must_use]
    pub fn active_pet(&self) -> &PetId {
        &self.active_pet
    }

    /// Applies a host-owned pet event and rejects illegal transitions.
    ///
    /// # Errors
    ///
    /// Returns `PetEvent` when the event is not valid for the current state.
    pub fn dispatch(&mut self, event: PetEvent) -> Result<PetState, PetEvent> {
        let next = match (self.state, event) {
            (PetState::Hidden, PetEvent::Show)
            | (PetState::Dragging, PetEvent::DragEnd)
            | (PetState::Success | PetState::Error, PetEvent::Dismiss) => PetState::Visible,
            (PetState::Visible, PetEvent::Hide) => PetState::Hidden,
            (PetState::Visible, PetEvent::DragStart) => PetState::Dragging,
            (PetState::Visible, PetEvent::OpenShelf) => PetState::Shelf,
            (PetState::Shelf, PetEvent::StartAction) => PetState::Working,
            (PetState::Working, PetEvent::ActionSucceeded) => PetState::Success,
            (PetState::Working, PetEvent::ActionFailed) => PetState::Error,
            _ => return Err(event),
        };
        self.state = next;
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::{PetController, PetEvent, PetId, PetState};

    #[test]
    fn pet_controller_accepts_host_owned_visible_shelf_action_flow() {
        let mut controller = PetController::new(PetId::new("nova"));
        controller.dispatch(PetEvent::Show).expect("show");
        controller.dispatch(PetEvent::OpenShelf).expect("shelf");
        controller.dispatch(PetEvent::StartAction).expect("working");
        controller
            .dispatch(PetEvent::ActionSucceeded)
            .expect("success");
        assert_eq!(controller.state(), PetState::Success);
    }

    #[test]
    fn pet_controller_rejects_hidden_drag_and_preserves_state() {
        let mut controller = PetController::new(PetId::new("nova"));
        assert_eq!(
            controller.dispatch(PetEvent::DragStart),
            Err(PetEvent::DragStart)
        );
        assert_eq!(controller.state(), PetState::Hidden);
    }
}
