use wmhost::HostId;

/// Stable ids assigned to the built-in file system extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FsExtension {
    pub read_ext_id: u32,
    pub write_ext_id: u32,
    pub exists_ext_id: u32,
    pub read_host_id: HostId,
    pub write_host_id: HostId,
    pub exists_host_id: HostId,
}

/// Stable ids assigned to the built-in debug extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DebugExtension {
    pub log_ext_id: u32,
    pub inspect_ext_id: u32,
    pub log_host_id: HostId,
    pub inspect_host_id: HostId,
}

/// Stable ids assigned to the built-in net extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetExtension {
    pub get_ext_id: u32,
    pub post_ext_id: u32,
    pub get_host_id: HostId,
    pub post_host_id: HostId,
}

/// Stable ids assigned to the built-in llm extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LlmExtension {
    pub generate_ext_id: u32,
    pub generate_host_id: HostId,
}

/// Stable ids assigned to the built-in message extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageExtension {
    pub show_ext_id: u32,
    pub append_ext_id: u32,
    pub choices_ext_id: u32,
    pub choices_named_ext_id: u32,
    pub prompt_ext_id: u32,
    pub hide_ext_id: u32,
    pub speed_ext_id: u32,
    pub auto_ext_id: u32,
    pub skip_ext_id: u32,
    pub log_clear_ext_id: u32,
    pub clear_ext_id: u32,
    pub box_style_ext_id: u32,
    pub text_color_ext_id: u32,
    pub speaker_color_ext_id: u32,
    pub accent_color_ext_id: u32,
    pub font_size_ext_id: u32,
    pub reset_style_ext_id: u32,
    pub frame_ext_id: u32,
    pub content_inset_ext_id: u32,
    pub input_box_style_ext_id: u32,
    pub input_text_color_ext_id: u32,
    pub input_hint_color_ext_id: u32,
    pub input_prompt_color_ext_id: u32,
    pub choice_box_style_ext_id: u32,
    pub choice_text_color_ext_id: u32,
    pub choice_accent_color_ext_id: u32,
    pub choice_selected_style_ext_id: u32,
    pub locale_ext_id: u32,
    pub show_host_id: HostId,
    pub append_host_id: HostId,
    pub choices_host_id: HostId,
    pub choices_named_host_id: HostId,
    pub prompt_host_id: HostId,
    pub hide_host_id: HostId,
    pub speed_host_id: HostId,
    pub auto_host_id: HostId,
    pub skip_host_id: HostId,
    pub log_clear_host_id: HostId,
    pub clear_host_id: HostId,
    pub box_style_host_id: HostId,
    pub text_color_host_id: HostId,
    pub speaker_color_host_id: HostId,
    pub accent_color_host_id: HostId,
    pub font_size_host_id: HostId,
    pub reset_style_host_id: HostId,
    pub frame_host_id: HostId,
    pub content_inset_host_id: HostId,
    pub input_box_style_host_id: HostId,
    pub input_text_color_host_id: HostId,
    pub input_hint_color_host_id: HostId,
    pub input_prompt_color_host_id: HostId,
    pub choice_box_style_host_id: HostId,
    pub choice_text_color_host_id: HostId,
    pub choice_accent_color_host_id: HostId,
    pub choice_selected_style_host_id: HostId,
    pub locale_host_id: HostId,
}

/// Stable ids assigned to the built-in scene extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneExtension {
    pub layout_ext_id: u32,
    pub reset_ext_id: u32,
    pub z_index_ext_id: u32,
    pub opening_ext_id: u32,
    pub ending_ext_id: u32,
    pub background_ext_id: u32,
    pub layout_host_id: HostId,
    pub reset_host_id: HostId,
    pub z_index_host_id: HostId,
    pub opening_host_id: HostId,
    pub ending_host_id: HostId,
    pub background_host_id: HostId,
}

/// Stable ids assigned to the built-in image extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageExtension {
    pub load_ext_id: u32,
    pub info_ext_id: u32,
    pub status_ext_id: u32,
    pub release_ext_id: u32,
    pub draw_ext_id: u32,
    pub draw_part_ext_id: u32,
    pub draw_ext_ext_id: u32,
    pub set_icon_sheet_ext_id: u32,
    pub draw_icon_ext_id: u32,
    pub load_host_id: HostId,
    pub info_host_id: HostId,
    pub status_host_id: HostId,
    pub release_host_id: HostId,
    pub draw_host_id: HostId,
    pub draw_part_host_id: HostId,
    pub draw_ext_host_id: HostId,
    pub set_icon_sheet_host_id: HostId,
    pub draw_icon_host_id: HostId,
}

/// Stable ids assigned to the built-in audio extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioExtension {
    pub load_ext_id: u32,
    pub play_ext_id: u32,
    pub playback_ext_id: u32,
    pub pause_ext_id: u32,
    pub stop_ext_id: u32,
    pub seek_ext_id: u32,
    pub volume_ext_id: u32,
    pub release_ext_id: u32,
    pub status_ext_id: u32,
    pub load_host_id: HostId,
    pub play_host_id: HostId,
    pub playback_host_id: HostId,
    pub pause_host_id: HostId,
    pub stop_host_id: HostId,
    pub seek_host_id: HostId,
    pub volume_host_id: HostId,
    pub release_host_id: HostId,
    pub status_host_id: HostId,
}

/// Stable ids assigned to the built-in VM extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VmExtension {
    pub save_ext_id: u32,
    pub load_ext_id: u32,
    pub save_host_id: HostId,
    pub load_host_id: HostId,
}

/// Stable ids assigned to the built-in persistent state extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateExtension {
    pub save_ext_id: u32,
    pub load_ext_id: u32,
    pub has_ext_id: u32,
    pub get_ext_id: u32,
    pub set_ext_id: u32,
    pub erase_ext_id: u32,
    pub save_host_id: HostId,
    pub load_host_id: HostId,
    pub has_host_id: HostId,
    pub get_host_id: HostId,
    pub set_host_id: HostId,
    pub erase_host_id: HostId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiExtension {
    pub context_menu_ext_id: u32,
    pub shift_fast_ext_id: u32,
    pub context_menu_host_id: HostId,
    pub shift_fast_host_id: HostId,
}

/// Stable ids assigned to the automation-game extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AutomationExtension {
    pub resource_ext_id: u32,
    pub set_resource_ext_id: u32,
    pub add_resource_ext_id: u32,
    pub set_job_ext_id: u32,
    pub enable_job_ext_id: u32,
    pub tick_ext_id: u32,
    pub job_progress_ext_id: u32,
    pub resource_host_id: HostId,
    pub set_resource_host_id: HostId,
    pub add_resource_host_id: HostId,
    pub set_job_host_id: HostId,
    pub enable_job_host_id: HostId,
    pub tick_host_id: HostId,
    pub job_progress_host_id: HostId,
}

/// Stable ids assigned to the RTS state extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RtsExtension {
    pub set_unit_ext_id: u32,
    pub move_unit_ext_id: u32,
    pub unit_x_ext_id: u32,
    pub unit_y_ext_id: u32,
    pub unit_hp_ext_id: u32,
    pub damage_unit_ext_id: u32,
    pub set_unit_host_id: HostId,
    pub move_unit_host_id: HostId,
    pub unit_x_host_id: HostId,
    pub unit_y_host_id: HostId,
    pub unit_hp_host_id: HostId,
    pub damage_unit_host_id: HostId,
}

/// Stable ids for the runtime's standard extension set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StandardExtensions {
    pub fs: FsExtension,
    pub debug: DebugExtension,
    pub net: NetExtension,
    pub llm: LlmExtension,
    pub scene: SceneExtension,
    pub message: MessageExtension,
    pub image: ImageExtension,
    pub audio: AudioExtension,
    pub ui: UiExtension,
    pub vm: VmExtension,
    pub state: StateExtension,
    pub automation: AutomationExtension,
    pub rts: RtsExtension,
}
