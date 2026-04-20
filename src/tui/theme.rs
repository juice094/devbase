use ratatui::style::{Color, Modifier, Style};

/// Semantic color tokens for the application theme.
///
/// All colors are named by their *role* in the interface, not by their
/// hue. This allows the same render code to work across different themes
/// (dark, light, high-contrast) without change.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    /// Primary accent (brand color, active elements)
    pub primary: Color,
    /// Secondary accent (less prominent interactive elements)
    pub secondary: Color,
    /// Success / healthy / up-to-date states
    pub success: Color,
    /// Warning / attention-required states
    pub warning: Color,
    /// Danger / error / blocked states
    pub danger: Color,
    /// Informational highlights (stars, insights)
    pub info: Color,
    /// Muted / disabled / secondary text
    pub muted: Color,
    /// Terminal background (for contrast checks)
    pub bg: Color,
    /// Elevated surface color (panels, popups)
    pub surface: Color,
    /// Border color for blocks
    pub border: Color,
    /// Text color on normal background
    pub text: Color,
    /// Highlighted / selected background
    pub highlight_bg: Color,
    /// Highlighted / selected foreground
    pub highlight_fg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// Dark theme optimized for terminal environments.
    pub const fn dark() -> Self {
        Self {
            primary: Color::Cyan,
            secondary: Color::Blue,
            success: Color::Green,
            warning: Color::Yellow,
            danger: Color::Red,
            info: Color::Rgb(255, 215, 0), // gold for stars
            muted: Color::DarkGray,
            bg: Color::Black,
            surface: Color::Rgb(30, 30, 30),
            border: Color::DarkGray,
            text: Color::White,
            highlight_bg: Color::Rgb(40, 40, 80),
            highlight_fg: Color::White,
        }
    }

    /// Light theme (placeholder for future configuration support).
    #[allow(dead_code)]
    pub const fn light() -> Self {
        Self {
            primary: Color::Blue,
            secondary: Color::Cyan,
            success: Color::Green,
            warning: Color::Rgb(200, 150, 0),
            danger: Color::Red,
            info: Color::Rgb(180, 140, 0),
            muted: Color::Gray,
            bg: Color::White,
            surface: Color::Rgb(245, 245, 245),
            border: Color::Gray,
            text: Color::Black,
            highlight_bg: Color::Rgb(200, 220, 255),
            highlight_fg: Color::Black,
        }
    }

    /// Derive all pre-composed styles from the theme.
    pub fn styles(&self) -> Styles {
        Styles::new(*self)
    }
}

/// Pre-composed styles used throughout the TUI.
///
/// Each field is a complete `Style` ready to be applied to a `Span`, `Line`,
/// or widget.  This centralises stylistic decisions and guarantees consistency.
#[derive(Debug, Clone, Copy)]
pub struct Styles {
    pub theme: Theme,
    // Titles & labels
    pub title: Style,
    pub label: Style,
    pub value: Style,
    pub dim: Style,
    // Status indicators
    pub status_ok: Style,
    pub status_warn: Style,
    pub status_danger: Style,
    pub status_loading: Style,
    // Interactive
    pub highlight: Style,
    pub selected: Style,
    pub border: Style,
    // Logs
    pub log_info: Style,
    pub log_warn: Style,
    pub log_error: Style,
    // Special
    pub tag: Style,
    pub star: Style,
    pub hint: Style,
    pub dry_run: Style,
    pub link: Style,
}

impl Styles {
    fn new(theme: Theme) -> Self {
        Self {
            theme,
            title: Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            label: Style::default().fg(theme.primary).add_modifier(Modifier::BOLD),
            value: Style::default().fg(theme.text),
            dim: Style::default().fg(theme.muted),
            status_ok: Style::default().fg(theme.success),
            status_warn: Style::default().fg(theme.warning),
            status_danger: Style::default().fg(theme.danger).add_modifier(Modifier::BOLD),
            status_loading: Style::default().fg(theme.primary),
            highlight: Style::default()
                .bg(theme.highlight_bg)
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD),
            selected: Style::default().fg(theme.primary).add_modifier(Modifier::BOLD),
            border: Style::default().fg(theme.border),
            log_info: Style::default().fg(theme.success),
            log_warn: Style::default().fg(theme.warning).add_modifier(Modifier::BOLD),
            log_error: Style::default().fg(theme.danger).add_modifier(Modifier::BOLD),
            tag: Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD),
            star: Style::default().fg(theme.info),
            hint: Style::default().fg(theme.muted),
            dry_run: Style::default().fg(theme.warning).add_modifier(Modifier::BOLD),
            link: Style::default().fg(theme.success),
        }
    }
}
