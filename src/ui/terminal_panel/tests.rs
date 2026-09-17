// SPDX-License-Identifier: MIT

use std::collections::HashSet;

use super::{SourcePalette, ThemeTokens, ansi_palette};

fn tokens() -> ThemeTokens {
    ThemeTokens {
        name: "Probe".to_string(),
        background: "#101014".to_string(),
        surface: "#17171d".to_string(),
        text: "#e6e6ea".to_string(),
        accent: "#4f8cff".to_string(),
        danger: "#ff5f5f".to_string(),
        muted: "#6a6a76".to_string(),
        highlight: "#2a2a35".to_string(),
        border: "#31313c".to_string(),
        dim_text: "#9a9aa6".to_string(),
    }
}

fn palette() -> SourcePalette {
    SourcePalette {
        statement: "#c77dff".to_string(),
        string: "#5fd97f".to_string(),
        constant: "#ffc861".to_string(),
        type_color: "#4fd6d6".to_string(),
        preprocessor: "#ff9f6b".to_string(),
    }
}

#[test]
fn theme_colors_fill_sixteen_distinguishable_ansi_slots() {
    let tokens = tokens();
    let colors = ansi_palette(&tokens, &palette());

    assert_eq!(colors.len(), 16);
    assert_eq!(colors[0], tokens.background);
    assert_eq!(colors[1], tokens.danger);
    assert_eq!(colors[4], tokens.accent);
    assert_eq!(colors[7], tokens.text);

    // Collapsing the chromatic slots would make diffs and ls output unreadable.
    let chromatic: HashSet<&String> = colors[1..7].iter().collect();
    assert_eq!(chromatic.len(), 6, "{colors:?}");

    for slot in 1..7 {
        assert_ne!(colors[slot], colors[slot + 8], "bright slot {slot} matches");
    }
}

#[test]
fn the_panel_releases_its_state_when_dropped() {
    crate::test_support::gtk_test(
        "ui::terminal_panel::tests::the_panel_releases_its_state_when_dropped",
        || {
            let preferences = super::ThemeManager::shared();
            let panel = super::TerminalPanel::new(&preferences, std::rc::Rc::new(|| None));
            let state = std::rc::Rc::downgrade(&panel.state);

            drop(panel);

            // Callbacks the panel installs on its own widgets must not own it,
            // or a closed window keeps its terminal and child alive.
            assert!(
                state.upgrade().is_none(),
                "the panel is still held by {} reference(s)",
                state.strong_count()
            );
        },
    );
}
