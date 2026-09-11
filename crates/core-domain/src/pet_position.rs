#![forbid(unsafe_code)]

/// A logical desktop point. Platform adapters convert physical pixels to this
/// representation before applying the host-owned pet placement policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LogicalPoint {
    pub x: i32,
    pub y: i32,
}

/// The usable work area of one display in logical coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisplayWorkArea {
    pub display_id: u64,
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub dpi: u16,
}

impl DisplayWorkArea {
    #[must_use]
    pub const fn right(self) -> i32 {
        self.left + self.width.cast_signed()
    }

    #[must_use]
    pub const fn bottom(self) -> i32 {
        self.top + self.height.cast_signed()
    }

    #[must_use]
    pub const fn contains(self, point: LogicalPoint) -> bool {
        point.x >= self.left
            && point.y >= self.top
            && point.x < self.right()
            && point.y < self.bottom()
    }
}

/// Persisted position for a pet. The display ID is advisory because a display
/// can be disconnected; callers must re-constrain it against current areas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PetPosition {
    pub display_id: u64,
    pub point: LogicalPoint,
}

/// Keeps a pet inside the usable area and snaps it to the nearest edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PetPlacement {
    pub position: PetPosition,
    pub width: u32,
    pub height: u32,
}

impl PetPlacement {
    /// Restores a saved position, using the primary display when its display
    /// is no longer connected.
    #[must_use]
    pub fn restore(
        saved: PetPosition,
        size: (u32, u32),
        displays: &[DisplayWorkArea],
    ) -> Option<Self> {
        let area = displays
            .iter()
            .find(|display| display.display_id == saved.display_id)
            .or_else(|| displays.first())?;
        Some(Self {
            position: PetPosition {
                display_id: area.display_id,
                point: clamp_point(saved.point, *area, size),
            },
            width: size.0,
            height: size.1,
        })
    }

    /// Snaps the current point to the closest edge while preserving the
    /// display identity and keeping the complete window in the work area.
    ///
    /// # Panics
    ///
    /// This method cannot panic because the four edge candidates are fixed and
    /// the candidate array is never empty.
    #[must_use]
    pub fn snap_to_edge(self, area: DisplayWorkArea) -> Self {
        let point = clamp_point(self.position.point, area, (self.width, self.height));
        let distances = [
            (
                point.x - area.left,
                LogicalPoint {
                    x: area.left,
                    y: point.y,
                },
            ),
            (
                area.right() - (point.x + self.width.cast_signed()),
                LogicalPoint {
                    x: area.right() - self.width.cast_signed(),
                    y: point.y,
                },
            ),
            (
                point.y - area.top,
                LogicalPoint {
                    x: point.x,
                    y: area.top,
                },
            ),
            (
                area.bottom() - (point.y + self.height.cast_signed()),
                LogicalPoint {
                    x: point.x,
                    y: area.bottom() - self.height.cast_signed(),
                },
            ),
        ];
        let (_, snapped) = distances
            .into_iter()
            .min_by_key(|(distance, _)| *distance)
            .expect("edge list is never empty");
        Self {
            position: PetPosition {
                display_id: area.display_id,
                point: snapped,
            },
            ..self
        }
    }
}

fn clamp_point(point: LogicalPoint, area: DisplayWorkArea, size: (u32, u32)) -> LogicalPoint {
    let max_x = area.right() - size.0.min(area.width).cast_signed();
    let max_y = area.bottom() - size.1.min(area.height).cast_signed();
    LogicalPoint {
        x: point.x.clamp(area.left, max_x),
        y: point.y.clamp(area.top, max_y),
    }
}

#[cfg(test)]
mod tests {
    use super::{DisplayWorkArea, LogicalPoint, PetPlacement, PetPosition};

    fn area(display_id: u64) -> DisplayWorkArea {
        DisplayWorkArea {
            display_id,
            left: 0,
            top: 0,
            width: 1920,
            height: 1080,
            dpi: 144,
        }
    }

    #[test]
    fn restore_clamps_offscreen_position_and_falls_back_to_primary_display() {
        let restored = PetPlacement::restore(
            PetPosition {
                display_id: 99,
                point: LogicalPoint { x: 5000, y: -20 },
            },
            (160, 120),
            &[area(1)],
        )
        .expect("primary display");
        assert_eq!(restored.position.display_id, 1);
        assert_eq!(restored.position.point, LogicalPoint { x: 1760, y: 0 });
    }

    #[test]
    fn snap_to_edge_uses_logical_coordinates_and_keeps_window_inside_area() {
        let placement = PetPlacement {
            position: PetPosition {
                display_id: 1,
                point: LogicalPoint { x: 900, y: 10 },
            },
            width: 160,
            height: 120,
        };
        let snapped = placement.snap_to_edge(area(1));
        assert_eq!(snapped.position.point, LogicalPoint { x: 900, y: 0 });
        assert!(area(1).contains(snapped.position.point));
        assert!(snapped.position.point.x + snapped.width.cast_signed() <= area(1).right());
    }
}
