/// Stable host ids used by WMScript v2 standard modules.
pub mod host_id {
    pub const CORE_LEN: u16 = 90;
    pub const CORE_SET_FIELD: u16 = 91;
    pub const CORE_SET_INDEX: u16 = 92;
    pub const CORE_ASSERT: u16 = 93;
    pub const UI_SAY: u16 = 100;
    pub const UI_CHOICE: u16 = 101;
    pub const INPUT_CHOICE: u16 = 110;
    pub const INPUT_TEXT: u16 = 111;
    pub const TIME_SLEEP: u16 = 120;
    pub const TIME_TICK: u16 = 121;
    pub const RANDOM_INT: u16 = 130;
    pub const WORLD_SPAWN: u16 = 140;
    pub const WORLD_GET: u16 = 141;
    pub const WORLD_SET: u16 = 142;
    pub const WORLD_EMIT: u16 = 143;
    pub const SAVE_STORE: u16 = 150;
    pub const SAVE_LOAD: u16 = 151;
    pub const ASSET_LOAD: u16 = 160;
    pub const AUDIO_PLAY: u16 = 170;
    pub const SCENE_SET: u16 = 180;
}

pub fn resolve_host(path: &str) -> Option<u16> {
    Some(match path {
        "core.len" => host_id::CORE_LEN,
        "core.assert" => host_id::CORE_ASSERT,
        "ui.say" => host_id::UI_SAY,
        "ui.choice" => host_id::UI_CHOICE,
        "input.choice" => host_id::INPUT_CHOICE,
        "input.text" => host_id::INPUT_TEXT,
        "time.sleep" => host_id::TIME_SLEEP,
        "time.tick" => host_id::TIME_TICK,
        "random.int" => host_id::RANDOM_INT,
        "world.spawn" => host_id::WORLD_SPAWN,
        "world.get" => host_id::WORLD_GET,
        "world.set" => host_id::WORLD_SET,
        "world.emit" => host_id::WORLD_EMIT,
        "save.store" => host_id::SAVE_STORE,
        "save.load" => host_id::SAVE_LOAD,
        "asset.load" => host_id::ASSET_LOAD,
        "audio.play" => host_id::AUDIO_PLAY,
        "scene.set" => host_id::SCENE_SET,
        _ => return None,
    })
}
