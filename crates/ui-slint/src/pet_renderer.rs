use std::collections::BTreeMap;

use novahub_core_domain::pet::PetState;

const MAX_FPS: u8 = 30;
const REDUCED_FPS: u8 = 6;

/// Host-owned animation policy. Plugins provide assets, never a render loop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnimationMode {
    Full,
    Reduced,
    Static,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PetFrameState {
    Idle,
    Working,
    Success,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderFrame {
    pub asset: String,
    pub fps: u8,
}

/// Selects bounded host-owned frames and applies the accessibility/resource
/// animation policy before Slint receives a frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PetRenderer {
    frames: BTreeMap<PetFrameState, RenderFrame>,
    mode: AnimationMode,
}

impl PetRenderer {
    /// Creates a renderer with the required idle fallback frame.
    ///
    /// # Panics
    ///
    /// Panics when `idle_asset` is empty; package validation should reject that
    /// input before constructing the renderer.
    #[must_use]
    pub fn new(idle_asset: impl Into<String>) -> Self {
        let idle_asset = idle_asset.into();
        assert!(
            !idle_asset.trim().is_empty(),
            "idle asset must not be empty"
        );
        let mut frames = BTreeMap::new();
        frames.insert(
            PetFrameState::Idle,
            RenderFrame {
                asset: idle_asset,
                fps: REDUCED_FPS,
            },
        );
        Self {
            frames,
            mode: AnimationMode::Full,
        }
    }

    ///
    /// # Errors
    ///
    /// Returns an error when `asset` is empty.
    pub fn set_frame(
        &mut self,
        state: PetFrameState,
        asset: impl Into<String>,
        fps: u8,
    ) -> Result<(), &'static str> {
        let asset = asset.into();
        if asset.trim().is_empty() {
            return Err("pet frame asset must not be empty");
        }
        self.frames.insert(
            state,
            RenderFrame {
                asset,
                fps: fps.min(MAX_FPS),
            },
        );
        Ok(())
    }

    pub fn set_animation_mode(&mut self, mode: AnimationMode) {
        self.mode = mode;
    }

    #[must_use]
    pub const fn animation_mode(&self) -> AnimationMode {
        self.mode
    }

    /// Returns the frame to render, falling back to idle for non-core states.
    #[must_use]
    pub fn frame_for(&self, state: PetState) -> Option<RenderFrame> {
        let frame_state = match state {
            PetState::Hidden => return None,
            PetState::Working => PetFrameState::Working,
            PetState::Success => PetFrameState::Success,
            PetState::Error => PetFrameState::Error,
            PetState::Visible | PetState::Dragging | PetState::Shelf => PetFrameState::Idle,
        };
        let frame = self
            .frames
            .get(&frame_state)
            .or_else(|| self.frames.get(&PetFrameState::Idle))?
            .clone();
        Some(RenderFrame {
            asset: frame.asset,
            fps: self.effective_fps(frame.fps),
        })
    }

    fn effective_fps(&self, fps: u8) -> u8 {
        match self.mode {
            AnimationMode::Full => fps,
            AnimationMode::Reduced => fps.min(REDUCED_FPS),
            AnimationMode::Static => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AnimationMode, PetFrameState, PetRenderer};
    use novahub_core_domain::pet::PetState;

    #[test]
    fn renderer_falls_back_to_idle_and_hides_without_a_visible_state() {
        let renderer = PetRenderer::new("assets/idle.webp");
        assert_eq!(
            renderer.frame_for(PetState::Working),
            Some(super::RenderFrame {
                asset: "assets/idle.webp".into(),
                fps: 6,
            })
        );
        assert_eq!(renderer.frame_for(PetState::Hidden), None);
    }

    #[test]
    fn reduced_and_static_modes_bound_host_animation() {
        let mut renderer = PetRenderer::new("idle");
        renderer
            .set_frame(PetFrameState::Working, "working", 24)
            .expect("working frame");
        renderer.set_animation_mode(AnimationMode::Reduced);
        assert_eq!(renderer.frame_for(PetState::Working).expect("frame").fps, 6);
        renderer.set_animation_mode(AnimationMode::Static);
        assert_eq!(renderer.frame_for(PetState::Working).expect("frame").fps, 0);
    }

    #[test]
    fn renderer_clamps_declared_fps_and_rejects_empty_assets() {
        let mut renderer = PetRenderer::new("idle");
        renderer
            .set_frame(PetFrameState::Success, "success", 120)
            .expect("success frame");
        assert_eq!(
            renderer
                .frame_for(PetState::Success)
                .expect("success frame")
                .fps,
            30
        );
        assert!(renderer.set_frame(PetFrameState::Error, " ", 12).is_err());
    }
}
