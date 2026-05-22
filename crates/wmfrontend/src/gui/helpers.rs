use super::*;

pub(super) fn resolve_source_rect(draw: &UiImageDrawCall, natural: egui::Vec2) -> egui::Rect {
    if let Some(source) = draw.source {
        return egui::Rect::from_min_size(
            egui::pos2(source.x, source.y),
            egui::vec2(source.width, source.height),
        );
    }
    if let Some(icon_sheet) = draw.icon_sheet {
        let cell_w = icon_sheet.cell_width as f32;
        let cell_h = icon_sheet.cell_height as f32;
        if cell_w > 0.0 && cell_h > 0.0 {
            let cols = (natural.x / cell_w).floor().max(1.0) as u32;
            let col = icon_sheet.index % cols;
            let row = icon_sheet.index / cols;
            return egui::Rect::from_min_size(
                egui::pos2(col as f32 * cell_w, row as f32 * cell_h),
                egui::vec2(cell_w, cell_h),
            );
        }
    }
    egui::Rect::from_min_size(egui::Pos2::ZERO, natural)
}

pub(super) fn direction_choice_for_pressed_key(
    choices: &[UiChoice],
    ctx: &egui::Context,
) -> Option<UiChoice> {
    for key in [
        egui::Key::ArrowUp,
        egui::Key::W,
        egui::Key::ArrowDown,
        egui::Key::S,
        egui::Key::ArrowLeft,
        egui::Key::A,
        egui::Key::ArrowRight,
        egui::Key::D,
    ] {
        if ctx.input(|input| input.key_pressed(key))
            && let Some(choice) = direction_choice_for_key(choices, key)
        {
            return Some(choice);
        }
    }
    None
}

pub(super) fn direction_choice_for_key(choices: &[UiChoice], key: egui::Key) -> Option<UiChoice> {
    if !should_hide_choice_panel_for_movement(choices) {
        return None;
    }

    let ids: &[&str] = match key {
        egui::Key::ArrowUp | egui::Key::W => &["north", "forward"],
        egui::Key::ArrowDown | egui::Key::S => &["south", "back"],
        egui::Key::ArrowLeft | egui::Key::A => &["west", "turn_left"],
        egui::Key::ArrowRight | egui::Key::D => &["east", "turn_right"],
        _ => return None,
    };
    ids.iter().find_map(|id| {
        choices
            .iter()
            .find(|choice| choice.enabled && choice.id == *id)
            .cloned()
    })
}

pub(super) fn should_hide_choice_panel_for_movement(choices: &[UiChoice]) -> bool {
    !choices.is_empty()
        && choices
            .iter()
            .all(|choice| choice.enabled && is_movement_choice_id(&choice.id))
}

pub(super) fn is_movement_choice_id(id: &str) -> bool {
    matches!(
        id,
        "north" | "south" | "east" | "west" | "forward" | "back" | "turn_left" | "turn_right"
    )
}

pub(super) fn next_enabled_choice_id(
    choices: &[UiChoice],
    selected: Option<&str>,
    delta: i32,
) -> Option<String> {
    let enabled = choices
        .iter()
        .filter(|choice| choice.enabled)
        .collect::<Vec<_>>();
    if enabled.is_empty() {
        return None;
    }
    let current_index = selected
        .and_then(|selected| enabled.iter().position(|choice| choice.id == selected))
        .unwrap_or(0) as i32;
    let next_index = (current_index + delta).rem_euclid(enabled.len() as i32) as usize;
    Some(enabled[next_index].id.clone())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EscapeAction {
    CloseRuntimeMenu,
    OpenRuntimeMenu,
}

pub(super) fn escape_action(runtime_menu_open: bool) -> EscapeAction {
    if runtime_menu_open {
        EscapeAction::CloseRuntimeMenu
    } else {
        EscapeAction::OpenRuntimeMenu
    }
}

pub(super) fn debug_shortcut_pressed(ctx: &egui::Context) -> bool {
    ctx.input(|input| {
        debug_shortcut_for_key(egui::Key::D, input.modifiers) && input.key_pressed(egui::Key::D)
    })
}

pub(super) fn debug_shortcut_for_key(key: egui::Key, modifiers: egui::Modifiers) -> bool {
    key == egui::Key::D && modifiers.alt
}

pub(super) fn paint_textured_rect(
    painter: &egui::Painter,
    texture_id: egui::TextureId,
    rect: egui::Rect,
    uv: egui::Rect,
    tint: egui::Color32,
    rotation_degrees: f32,
) {
    if rotation_degrees.abs() <= f32::EPSILON {
        painter.image(texture_id, rect, uv, tint);
        return;
    }

    let center = rect.center();
    let rotation = egui::emath::Rot2::from_angle(rotation_degrees.to_radians());
    let mut mesh = egui::Mesh::with_texture(texture_id);
    let corners = [
        (rect.left_top(), uv.left_top()),
        (rect.right_top(), uv.right_top()),
        (rect.right_bottom(), uv.right_bottom()),
        (rect.left_bottom(), uv.left_bottom()),
    ];
    for (pos, uv) in corners {
        let rotated = center + rotation * (pos - center);
        mesh.vertices.push(egui::epaint::Vertex {
            pos: rotated,
            uv,
            color: tint,
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(egui::Shape::mesh(mesh));
}

pub(super) fn font_definitions(preset: GuiFontPreset) -> egui::FontDefinitions {
    match preset {
        GuiFontPreset::NotoSans => {
            let mut fonts = egui::FontDefinitions::default();
            if let Some(bytes) = load_noto_sans_bytes() {
                fonts.font_data.insert(
                    "noto_sans_jp".to_owned(),
                    Arc::new(egui::FontData::from_owned(bytes)),
                );
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, "noto_sans_jp".to_owned());
            }
            fonts
        }
        GuiFontPreset::EguiDefault => egui::FontDefinitions::default(),
        GuiFontPreset::Monospace => {
            let mut fonts = egui::FontDefinitions::default();
            if let Some(monospace) = fonts.families.get(&egui::FontFamily::Monospace).cloned() {
                fonts
                    .families
                    .insert(egui::FontFamily::Proportional, monospace);
            }
            fonts
        }
    }
}

pub(super) fn load_noto_sans_bytes() -> Option<Vec<u8>> {
    let mut candidates = Vec::new();
    if let Ok(path) = std::env::var("WML_FRONTEND_FONT_PATH") {
        candidates.push(path);
    }
    candidates.extend([
        r"C:\Windows\Fonts\NotoSansJP-VF.ttf".to_owned(),
        r"C:\Windows\Fonts\NotoSansCJKjp-Regular.otf".to_owned(),
        r"C:\Windows\Fonts\NotoSansJP-Regular.otf".to_owned(),
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".to_owned(),
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc".to_owned(),
        "/usr/share/fonts/truetype/noto/NotoSansJP-Regular.ttf".to_owned(),
        "/System/Library/Fonts/Supplemental/NotoSansCJK.ttc".to_owned(),
        "/Library/Fonts/NotoSansJP-Regular.otf".to_owned(),
    ]);
    for path in candidates {
        if let Ok(bytes) = std::fs::read(&path) {
            return Some(bytes);
        }
    }
    None
}

pub(super) fn decode_texture(
    ctx: &egui::Context,
    image: &UiImageSource,
) -> Result<egui::TextureHandle, DecodeTextureError> {
    let decoded = image::load_from_memory(&image.bytes)
        .map_err(|error| DecodeTextureError::new(format!("{}: {}", image.label, error)))?;
    let rgba = decoded.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Ok(ctx.load_texture(
        image.label.clone(),
        color_image,
        egui::TextureOptions::LINEAR,
    ))
}

pub(super) fn slot_name(slot: &UiImageSlot) -> String {
    match slot {
        UiImageSlot::Background => "background".to_owned(),
        UiImageSlot::Portrait => "portrait".to_owned(),
        UiImageSlot::Foreground => "foreground".to_owned(),
        UiImageSlot::Overlay => "overlay".to_owned(),
        UiImageSlot::Named(name) => name.clone(),
    }
}

pub(super) fn map_modifiers(modifiers: egui::Modifiers) -> wmui::UiModifiers {
    wmui::UiModifiers {
        shift: modifiers.shift,
        ctrl: modifiers.ctrl,
        alt: modifiers.alt,
        logo: modifiers.mac_cmd || modifiers.command,
    }
}

pub(super) fn map_pressed_buttons(
    pointer: &egui::PointerState,
) -> std::collections::BTreeSet<UiMouseButton> {
    let mut buttons = std::collections::BTreeSet::new();
    if pointer.button_down(egui::PointerButton::Primary) {
        buttons.insert(UiMouseButton::Primary);
    }
    if pointer.button_down(egui::PointerButton::Secondary) {
        buttons.insert(UiMouseButton::Secondary);
    }
    if pointer.button_down(egui::PointerButton::Middle) {
        buttons.insert(UiMouseButton::Middle);
    }
    if pointer.button_down(egui::PointerButton::Extra1) {
        buttons.insert(UiMouseButton::Back);
    }
    if pointer.button_down(egui::PointerButton::Extra2) {
        buttons.insert(UiMouseButton::Forward);
    }
    buttons
}

pub(super) fn map_key_to_ui(key: egui::Key) -> Option<UiKey> {
    Some(match key {
        egui::Key::Enter => UiKey::Enter,
        egui::Key::Escape => UiKey::Escape,
        egui::Key::Backspace => UiKey::Backspace,
        egui::Key::Tab => UiKey::Tab,
        egui::Key::Space => UiKey::Space,
        egui::Key::ArrowUp => UiKey::ArrowUp,
        egui::Key::ArrowDown => UiKey::ArrowDown,
        egui::Key::ArrowLeft => UiKey::ArrowLeft,
        egui::Key::ArrowRight => UiKey::ArrowRight,
        egui::Key::A => UiKey::Character('a'),
        egui::Key::B => UiKey::Character('b'),
        egui::Key::C => UiKey::Character('c'),
        egui::Key::D => UiKey::Character('d'),
        egui::Key::E => UiKey::Character('e'),
        egui::Key::F => UiKey::Character('f'),
        egui::Key::G => UiKey::Character('g'),
        egui::Key::H => UiKey::Character('h'),
        egui::Key::I => UiKey::Character('i'),
        egui::Key::J => UiKey::Character('j'),
        egui::Key::K => UiKey::Character('k'),
        egui::Key::L => UiKey::Character('l'),
        egui::Key::M => UiKey::Character('m'),
        egui::Key::N => UiKey::Character('n'),
        egui::Key::O => UiKey::Character('o'),
        egui::Key::P => UiKey::Character('p'),
        egui::Key::Q => UiKey::Character('q'),
        egui::Key::R => UiKey::Character('r'),
        egui::Key::S => UiKey::Character('s'),
        egui::Key::T => UiKey::Character('t'),
        egui::Key::U => UiKey::Character('u'),
        egui::Key::V => UiKey::Character('v'),
        egui::Key::W => UiKey::Character('w'),
        egui::Key::X => UiKey::Character('x'),
        egui::Key::Y => UiKey::Character('y'),
        egui::Key::Z => UiKey::Character('z'),
        _ => return None,
    })
}

#[derive(Debug)]
pub(super) struct DecodeTextureError(String);

#[derive(Default)]
pub(super) struct InputSnapshot {
    pub(super) pointer_position: Option<egui::Pos2>,
    pub(super) raw_scroll_delta: egui::Vec2,
    pub(super) modifiers: egui::Modifiers,
    pub(super) pressed_keys: Vec<egui::Key>,
    pub(super) text_input: String,
    pub(super) recent_events: Vec<String>,
}

impl DecodeTextureError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for DecodeTextureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DecodeTextureError {}
