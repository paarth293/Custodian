//! Pure geometry for the Cubby window: where it starts, and how it grows when
//! the chat panel opens without leaving the screen. All values are physical
//! pixels. Kept free of Tauri types so it can be unit-tested.

/// A rectangle in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn right(&self) -> i32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.height
    }

    fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
}

/// Which side of Cubby the chat panel opens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelSide {
    Left,
    Right,
}

/// Keeps a window of `width`×`height` fully inside `area`.
pub fn clamp_into(x: i32, y: i32, width: i32, height: i32, area: Rect) -> (i32, i32) {
    let max_x = (area.right() - width).max(area.x);
    let max_y = (area.bottom() - height).max(area.y);
    (x.clamp(area.x, max_x), y.clamp(area.y, max_y))
}

/// Initial position: the saved one if its centre is still on some monitor,
/// otherwise the bottom-right corner of the primary work area.
pub fn initial_position(
    saved: Option<(i32, i32)>,
    size: (i32, i32),
    work_areas: &[Rect],
    primary: Rect,
) -> (i32, i32) {
    const MARGIN: i32 = 24;
    if let Some((x, y)) = saved {
        let (cx, cy) = (x + size.0 / 2, y + size.1 / 2);
        if let Some(area) = work_areas.iter().find(|a| a.contains_point(cx, cy)) {
            return clamp_into(x, y, size.0, size.1, *area);
        }
    }
    clamp_into(
        primary.right() - size.0 - MARGIN,
        primary.bottom() - size.1 - MARGIN,
        size.0,
        size.1,
        primary,
    )
}

/// Expanded window placement for an open chat panel.
///
/// Cubby stays exactly where it is on screen: the window grows upward (bottom
/// edge fixed) and sideways toward whichever side has room, preferring the
/// side facing the centre of the screen.
pub fn expand(compact: Rect, expanded_size: (i32, i32), area: Rect) -> (Rect, PanelSide) {
    let extra = expanded_size.0 - compact.width;
    let centre_x = compact.x + compact.width / 2;
    let area_centre = area.x + area.width / 2;
    let room_left = compact.x - area.x;
    let room_right = area.right() - compact.right();

    let side = if centre_x >= area_centre && room_left >= extra {
        PanelSide::Left
    } else if room_right >= extra {
        PanelSide::Right
    } else if room_left >= room_right {
        PanelSide::Left
    } else {
        PanelSide::Right
    };

    let x = match side {
        PanelSide::Left => compact.x - extra,
        PanelSide::Right => compact.x,
    };
    let y = compact.bottom() - expanded_size.1;
    let (x, y) = clamp_into(x, y, expanded_size.0, expanded_size.1, area);
    (
        Rect {
            x,
            y,
            width: expanded_size.0,
            height: expanded_size.1,
        },
        side,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1040,
    };

    #[test]
    fn defaults_to_bottom_right_of_primary() {
        let (x, y) = initial_position(None, (240, 280), &[SCREEN], SCREEN);
        assert_eq!((x, y), (1920 - 240 - 24, 1040 - 280 - 24));
    }

    #[test]
    fn restores_saved_position_on_a_visible_monitor() {
        let (x, y) = initial_position(Some((300, 400)), (240, 280), &[SCREEN], SCREEN);
        assert_eq!((x, y), (300, 400));
    }

    #[test]
    fn ignores_saved_position_on_a_disconnected_monitor() {
        let (x, y) = initial_position(Some((3000, 400)), (240, 280), &[SCREEN], SCREEN);
        assert_eq!((x, y), (1656, 736));
    }

    #[test]
    fn panel_opens_toward_screen_centre_and_keeps_cubby_in_place() {
        let compact = Rect {
            x: 1656,
            y: 736,
            width: 240,
            height: 280,
        };
        let (window, side) = expand(compact, (620, 580), SCREEN);
        assert_eq!(side, PanelSide::Left);
        // Cubby's column (the right 240 px) stays where it was.
        assert_eq!(window.right(), compact.right());
        assert_eq!(window.bottom(), compact.bottom());
    }

    #[test]
    fn panel_opens_right_when_cubby_is_on_the_left_edge() {
        let compact = Rect {
            x: 10,
            y: 600,
            width: 240,
            height: 280,
        };
        let (window, side) = expand(compact, (620, 580), SCREEN);
        assert_eq!(side, PanelSide::Right);
        assert_eq!(window.x, 10);
    }

    #[test]
    fn expanded_window_never_leaves_the_work_area() {
        let compact = Rect {
            x: 900,
            y: 0,
            width: 240,
            height: 280,
        };
        let (window, _) = expand(compact, (620, 580), SCREEN);
        assert!(window.y >= 0 && window.bottom() <= SCREEN.bottom());
        assert!(window.x >= 0 && window.right() <= SCREEN.right());
    }
}
