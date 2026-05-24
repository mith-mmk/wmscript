use std::collections::BTreeMap;

use wmarchive::Manifest;
use wmresource::ResourceManager;
use wmui::UiSceneLayoutState;

use super::{
    AudioPlaybackState, IconSheetState, ImageDrawState, MessageWindowState, RpgUiState,
    StateManager, UiPolicyState,
};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RuntimeCheckpoint {
    pub(super) scheduler: wmvm::SchedulerSnapshot,
    pub(super) resources: ResourceManager,
    pub(super) loaded_archives: Vec<Manifest>,
    pub(super) image_draws: Vec<ImageDrawState>,
    pub(super) icon_sheets: BTreeMap<u64, IconSheetState>,
    pub(super) scene_layout: UiSceneLayoutState,
    pub(super) message_window: MessageWindowState,
    pub(super) ui_policy: UiPolicyState,
    pub(super) rpg_ui: RpgUiState,
    pub(super) debug_log: Vec<String>,
    pub(super) audio_states: BTreeMap<u64, AudioPlaybackState>,
    pub(super) state_manager: StateManager,
}
