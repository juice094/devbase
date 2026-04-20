use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};

/// Computed layout rectangles for all major UI regions.
///
/// Produced by [`AppLayout::compute`] so that every render pass works with
/// stable, named regions instead of re-calculating percentages inline.
#[derive(Debug, Clone, Copy)]
pub struct AppLayout {
    /// Full terminal area
    pub area: Rect,
    /// Left-hand repository list
    pub list: Rect,
    /// Right-hand stack (detail + logs)
    pub right: Rect,
    /// Detail panel (inside `right`)
    pub detail: Rect,
    /// Logs panel (inside `right`)
    pub logs: Rect,
    /// Bottom status / hint bar
    pub bottom: Rect,
    /// Whether the terminal is considered "compact"
    pub compact: bool,
}

impl AppLayout {
    /// Compute layout for the given terminal size.
    ///
    /// # Breakpoints
    ///
    /// * **Compact** – width < 80 or height < 20.  List takes the full width;
    ///   detail/logs are not allocated (the caller should overlay them or hide
    ///   them).  Bottom bar is reduced to a single line.
    /// * **Standard** – width 80-119.  Classic 35/65 horizontal split with
    ///   detail/logs stacked 60/40 vertically.
    /// * **Wide** – width >= 120.  30/70 split with more room for the detail
    ///   panel.
    pub fn compute(area: Rect) -> Self {
        let compact = area.width < 80 || area.height < 20;

        if compact {
            let main = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(1)])
                .split(area);
            return Self {
                area,
                list: main[0],
                right: Rect::default(),
                detail: Rect::default(),
                logs: Rect::default(),
                bottom: main[1],
                compact: true,
            };
        }

        let main = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);

        let h_split = if area.width >= 120 {
            [Constraint::Percentage(30), Constraint::Fill(1)]
        } else {
            [Constraint::Percentage(35), Constraint::Fill(1)]
        };
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(h_split)
            .split(main[0]);

        let v_split = if area.height >= 40 {
            [Constraint::Percentage(55), Constraint::Fill(1)]
        } else {
            [Constraint::Percentage(60), Constraint::Fill(1)]
        };
        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints(v_split)
            .split(horizontal[1]);

        Self {
            area,
            list: horizontal[0],
            right: horizontal[1],
            detail: right[0],
            logs: right[1],
            bottom: main[1],
            compact: false,
        }
    }

    /// Inner rectangle with 1-cell margin on all sides.
    pub fn inner(rect: Rect) -> Rect {
        rect.inner(Margin::new(1, 1))
    }

    /// Create a centered rectangle occupying `pct_x` % width and `pct_y` % height.
    pub fn centered(area: Rect, pct_x: u16, pct_y: u16) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - pct_y) / 2),
                Constraint::Percentage(pct_y),
                Constraint::Percentage((100 - pct_y) / 2),
            ])
            .split(area);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - pct_x) / 2),
                Constraint::Percentage(pct_x),
                Constraint::Percentage((100 - pct_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_compact() {
        let area = Rect::new(0, 0, 60, 15);
        let layout = AppLayout::compute(area);
        assert!(layout.compact);
        assert_eq!(layout.area, area);
        assert_eq!(layout.list.width, 60);
        assert_eq!(layout.bottom.height, 1);
        assert_eq!(layout.detail, Rect::default());
        assert_eq!(layout.logs, Rect::default());
    }

    #[test]
    fn test_layout_standard() {
        let area = Rect::new(0, 0, 100, 30);
        let layout = AppLayout::compute(area);
        assert!(!layout.compact);
        assert_eq!(layout.area, area);
        assert_eq!(layout.list.width, 35); // 35% of 100
        assert_eq!(layout.right.width, 65); // remaining
        assert_eq!(layout.bottom.height, 1);
    }

    #[test]
    fn test_layout_wide() {
        let area = Rect::new(0, 0, 120, 40);
        let layout = AppLayout::compute(area);
        assert!(!layout.compact);
        assert_eq!(layout.list.width + layout.right.width, 120);
        // detail/logs should partition the right panel height
        assert_eq!(layout.detail.height + layout.logs.height, layout.right.height);
    }

    #[test]
    fn test_layout_centered() {
        let area = Rect::new(0, 0, 100, 100);
        let centered = AppLayout::centered(area, 50, 50);
        assert_eq!(centered.width, 50);
        assert_eq!(centered.height, 50);
        assert_eq!(centered.x, 25);
        assert_eq!(centered.y, 25);
    }

    #[test]
    fn test_layout_inner() {
        let rect = Rect::new(10, 10, 20, 10);
        let inner = AppLayout::inner(rect);
        assert_eq!(inner.x, 11);
        assert_eq!(inner.y, 11);
        assert_eq!(inner.width, 18);
        assert_eq!(inner.height, 8);
    }
}
