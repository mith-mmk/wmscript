use super::*;

fn choice(id: &str) -> UiChoice {
    UiChoice::new(id, id)
}

#[test]
fn direction_keys_map_to_2d_choices() {
    let choices = vec![
        choice("north"),
        choice("south"),
        choice("west"),
        choice("east"),
    ];
    assert_eq!(
        direction_choice_for_key(&choices, egui::Key::ArrowUp)
            .map(|choice| choice.id)
            .as_deref(),
        Some("north")
    );
    assert_eq!(
        direction_choice_for_key(&choices, egui::Key::W)
            .map(|choice| choice.id)
            .as_deref(),
        Some("north")
    );
    assert_eq!(
        direction_choice_for_key(&choices, egui::Key::S)
            .map(|choice| choice.id)
            .as_deref(),
        Some("south")
    );
    assert_eq!(
        direction_choice_for_key(&choices, egui::Key::D)
            .map(|choice| choice.id)
            .as_deref(),
        Some("east")
    );
}

#[test]
fn direction_keys_map_to_grid3d_choices() {
    let choices = vec![
        choice("forward"),
        choice("back"),
        choice("turn_left"),
        choice("turn_right"),
    ];
    assert_eq!(
        direction_choice_for_key(&choices, egui::Key::ArrowUp)
            .map(|choice| choice.id)
            .as_deref(),
        Some("forward")
    );
    assert_eq!(
        direction_choice_for_key(&choices, egui::Key::ArrowLeft)
            .map(|choice| choice.id)
            .as_deref(),
        Some("turn_left")
    );
    assert_eq!(
        direction_choice_for_key(&choices, egui::Key::A)
            .map(|choice| choice.id)
            .as_deref(),
        Some("turn_left")
    );
}

#[test]
fn non_map_choices_do_not_consume_shortcut_keys() {
    let choices = vec![choice("status"), choice("inventory")];
    assert!(direction_choice_for_key(&choices, egui::Key::A).is_none());
    assert!(direction_choice_for_key(&choices, egui::Key::S).is_none());
    assert!(direction_choice_for_key(&choices, egui::Key::ArrowDown).is_none());
}

#[test]
fn movement_only_choices_hide_choice_panel() {
    let choices = vec![
        choice("north"),
        choice("south"),
        choice("east"),
        choice("west"),
    ];
    assert!(should_hide_choice_panel_for_movement(&choices));

    let choices = vec![
        choice("forward"),
        choice("back"),
        choice("turn_left"),
        choice("turn_right"),
    ];
    assert!(should_hide_choice_panel_for_movement(&choices));
}

#[test]
fn explicit_choices_keep_choice_panel_visible() {
    let choices = vec![choice("north"), choice("check")];
    assert!(!should_hide_choice_panel_for_movement(&choices));

    let choices = vec![choice("status"), choice("inventory")];
    assert!(!should_hide_choice_panel_for_movement(&choices));
}

#[test]
fn mixed_choices_do_not_consume_direction_keys() {
    let choices = vec![choice("north"), choice("check"), choice("end_demo")];
    assert!(direction_choice_for_key(&choices, egui::Key::ArrowUp).is_none());
    assert!(direction_choice_for_key(&choices, egui::Key::W).is_none());
}

#[test]
fn adjacent_choice_selection_cycles_all_enabled_choices() {
    let mut disabled = choice("disabled");
    disabled.enabled = false;
    let choices = vec![
        choice("north"),
        choice("south"),
        disabled,
        choice("east"),
        choice("west"),
    ];

    assert_eq!(
        next_enabled_choice_id(&choices, None, 1).as_deref(),
        Some("south")
    );
    assert_eq!(
        next_enabled_choice_id(&choices, Some("south"), 1).as_deref(),
        Some("east")
    );
    assert_eq!(
        next_enabled_choice_id(&choices, Some("west"), 1).as_deref(),
        Some("north")
    );
    assert_eq!(
        next_enabled_choice_id(&choices, Some("north"), -1).as_deref(),
        Some("west")
    );
}

#[test]
fn escape_toggles_runtime_menu_instead_of_closing_window() {
    assert_eq!(escape_action(false), EscapeAction::OpenRuntimeMenu);
    assert_eq!(escape_action(true), EscapeAction::CloseRuntimeMenu);
}

#[test]
fn debug_shortcut_requires_alt_d() {
    assert!(!debug_shortcut_for_key(
        egui::Key::D,
        egui::Modifiers::default()
    ));
    assert!(debug_shortcut_for_key(
        egui::Key::D,
        egui::Modifiers {
            alt: true,
            ..Default::default()
        }
    ));
    assert!(!debug_shortcut_for_key(
        egui::Key::A,
        egui::Modifiers {
            alt: true,
            ..Default::default()
        }
    ));
}
