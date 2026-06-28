//! The UI theme: a single source of truth for every chrome color NyxVim draws.
//!
//! Before this module, ~70 `Color::` literals were scattered across eight files,
//! and the "blue if focused, dark-gray if not" selection rule was copy-pasted in
//! several places. The [`Theme`] gathers those into **semantic tokens** named for
//! their role (text, border, selection, …) rather than their concrete color, so
//! the whole editor can be reskinned by changing one place — see [`Theme::default`].
//!
//! Scope: this owns *UI chrome* only. Syntax-highlight colors (`syntax.rs`) and
//! terminal content colors (vt100 conversion in `terminal_pane.rs`) are separate
//! color spaces and are intentionally left out.
//!
//! `Theme` is `Copy` (every field is a `Copy` `Color`/`BorderType`), so render
//! code takes it by value or by a cheap `&Theme` without borrow-checker friction.

use ratatui::style::{Color, Style};
use ratatui::widgets::BorderType;

/// Semantic UI color tokens. Each field names a *role*; the concrete colors live
/// only in [`Theme::default`] (and any future palette constructor).
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    /// Primary foreground text (buffer text, focused line number).
    pub text: Color,
    /// Muted foreground: borders-as-text, hints, placeholders, unfocused gutter.
    pub text_muted: Color,
    /// Accent for titles, group headers, and active markers.
    pub accent: Color,
    /// Block borders and inter-region dividers.
    pub border: Color,
    /// Background of the line the cursor is on.
    pub cursor_line: Color,
    /// Background of selected *editor text* (distinct from a selected list row).
    pub selection: Color,
    /// Foreground of a selected row in a *focused* region.
    pub focus_fg: Color,
    /// Background of a selected row in a *focused* region (the primary highlight;
    /// also used for the focused modal/help border).
    pub focus_bg: Color,
    /// Foreground of a selected row / muted list item in an *unfocused* region.
    pub inactive_fg: Color,
    /// Background of a selected row in an *unfocused* region, and of chrome
    /// surfaces such as the footer.
    pub inactive_bg: Color,
    /// Background of the minibuffer prompt row.
    pub prompt_bg: Color,
    /// Added line: foreground / background.
    pub diff_add_fg: Color,
    pub diff_add_bg: Color,
    /// Deleted line: foreground / background.
    pub diff_del_fg: Color,
    pub diff_del_bg: Color,
    /// The empty opposite side of a diff change.
    pub diff_gap_bg: Color,
}

impl Default for Theme {
    /// The default theme: a Charm / Bubble Tea-inspired palette — hot-magenta
    /// accent, purple focus, soft-slate borders and muted text — over a
    /// Dracula-adjacent set of greens/reds for diffs. Truecolor `Rgb` values;
    /// the whole look is determined here (and in [`Theme::border_type`]), so it
    /// can be reverted as one isolated change without touching any call site.
    fn default() -> Self {
        Self {
            text: Color::Rgb(248, 248, 242),       // off-white
            text_muted: Color::Rgb(98, 114, 164),  // soft slate
            accent: Color::Rgb(255, 6, 183),       // hot magenta (Charm)
            border: Color::Rgb(98, 114, 164),       // soft slate
            cursor_line: Color::Rgb(40, 42, 54),    // subtle dark tint
            selection: Color::Rgb(68, 71, 90),      // muted slate-blue
            focus_fg: Color::Rgb(248, 248, 242),    // off-white
            focus_bg: Color::Rgb(125, 86, 244),     // purple (Charm)
            inactive_fg: Color::Rgb(98, 114, 164),  // soft slate
            inactive_bg: Color::Rgb(68, 71, 90),    // muted slate-blue
            prompt_bg: Color::Rgb(30, 31, 42),       // near-black slate
            diff_add_fg: Color::Rgb(80, 250, 123),  // green
            diff_add_bg: Color::Rgb(25, 55, 35),     // dark green
            diff_del_fg: Color::Rgb(255, 85, 85),    // red
            diff_del_bg: Color::Rgb(60, 25, 30),     // dark red
            diff_gap_bg: Color::Rgb(24, 25, 34),     // darker gap fill
        }
    }
}

impl Theme {
    /// Border style for bordered blocks. A single themed choice so the whole
    /// editor's border look lives in one place — rounded, for the Charm feel.
    pub fn border_type(&self) -> BorderType {
        BorderType::Rounded
    }

    /// Style for a selected row whose region's focus state is `focused`. Collapses
    /// the focused-vs-inactive selection rule into one place. (Sites whose
    /// unfocused branch keeps a different foreground build the style from the
    /// tokens directly instead.)
    pub fn list_row(&self, focused: bool) -> Style {
        if focused {
            Style::new().bg(self.focus_bg).fg(self.focus_fg)
        } else {
            Style::new().bg(self.inactive_bg).fg(self.inactive_fg)
        }
    }
}
