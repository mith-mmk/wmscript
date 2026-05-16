use super::super::*;

impl Runtime {
    pub fn install_vm_extension(&mut self) -> Result<VmExtension, RuntimeError> {
        let save_host_id = 160;
        let load_host_id = 161;
        let scheduler = self.scheduler.clone();
        let resources = self.resources.clone();
        let debug_log = self.debug_log.clone();
        let loaded_archives = self.loaded_archives.clone();
        let image_draws = self.image_draws.clone();
        let icon_sheets = self.icon_sheets.clone();
        let scene_layout = self.scene_layout.clone();
        let message_window = self.message_window.clone();
        let ui_policy = self.ui_policy.clone();
        let audio_states = self.audio_states.clone();
        let state_manager = self.state_manager.clone();
        let checkpoints = self.checkpoints.clone();
        let pending_vm_saves = self.pending_vm_saves.clone();
        let host = self.host.clone();

        let _ =
            self.register_host_function(HostFunction::new(save_host_id, 1, 1, 0), move |args| {
                let slot = expect_integer_arg(args, 0, "slot")? as u32;
                if let Ok(scheduler) = scheduler.try_borrow() {
                    checkpoints.borrow_mut().insert(
                        slot,
                        RuntimeCheckpoint {
                            scheduler: scheduler.snapshot(),
                            resources: resources.borrow().clone(),
                            loaded_archives: loaded_archives.borrow().clone(),
                            image_draws: image_draws.borrow().clone(),
                            icon_sheets: icon_sheets.borrow().clone(),
                            scene_layout: scene_layout.borrow().clone(),
                            message_window: message_window.borrow().clone(),
                            ui_policy: ui_policy.borrow().clone(),
                            debug_log: debug_log.borrow().clone(),
                            audio_states: audio_states.borrow().clone(),
                            state_manager: state_manager.borrow().clone(),
                        },
                    );
                } else {
                    pending_vm_saves.borrow_mut().push(slot);
                }
                Ok(Value::Bool(true))
            });

        let scheduler = self.scheduler.clone();
        let resources = self.resources.clone();
        let debug_log = self.debug_log.clone();
        let loaded_archives = self.loaded_archives.clone();
        let image_draws = self.image_draws.clone();
        let icon_sheets = self.icon_sheets.clone();
        let scene_layout = self.scene_layout.clone();
        let message_window = self.message_window.clone();
        let ui_policy = self.ui_policy.clone();
        let audio_states = self.audio_states.clone();
        let audio_backend = self.audio_backend.clone();
        let state_manager = self.state_manager.clone();
        let checkpoints = self.checkpoints.clone();
        let host_for_load = host.clone();
        let _ =
            self.register_host_function(HostFunction::new(load_host_id, 1, 1, 0), move |args| {
                let slot = expect_integer_arg(args, 0, "slot")? as u32;
                let Some(checkpoint) = checkpoints.borrow().get(&slot).cloned() else {
                    return Ok(Value::Bool(false));
                };
                *scheduler.borrow_mut() = Scheduler::from_snapshot(checkpoint.scheduler, |_| {
                    Box::new(SharedHostApi::new(host_for_load.clone()))
                });
                *resources.borrow_mut() = checkpoint.resources;
                *loaded_archives.borrow_mut() = checkpoint.loaded_archives;
                *image_draws.borrow_mut() = checkpoint.image_draws;
                *icon_sheets.borrow_mut() = checkpoint.icon_sheets;
                *scene_layout.borrow_mut() = checkpoint.scene_layout;
                *message_window.borrow_mut() = checkpoint.message_window;
                *ui_policy.borrow_mut() = checkpoint.ui_policy;
                *debug_log.borrow_mut() = checkpoint.debug_log;
                *audio_states.borrow_mut() = checkpoint.audio_states;
                *state_manager.borrow_mut() = checkpoint.state_manager;
                {
                    let backend = audio_backend.clone();
                    backend.clear()?;
                    let replay_states = audio_states
                        .borrow()
                        .iter()
                        .filter(|(_, state)| state.playing)
                        .map(|(handle, state)| (*handle, state.clone()))
                        .collect::<Vec<_>>();
                    for (handle, state) in replay_states {
                        let bytes = audio_bytes_for_resource_id(&resources, state.resource_id)?;
                        backend.play(
                            handle,
                            state.resource_id,
                            &bytes,
                            state.looped,
                            state.position_ms,
                            state.volume,
                        )?;
                    }
                }
                Ok(Value::Bool(true))
            });

        let ids = self.extensions.register_extension(
            "ext.vm",
            &[
                ExtensionFunctionSpec::new("save", save_host_id, 1, 1, 0)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("load", load_host_id, 1, 1, 0)
                    .with_return_type(ExtValueType::Bool),
            ],
        )?;

        Ok(VmExtension {
            save_ext_id: ids[0],
            load_ext_id: ids[1],
            save_host_id,
            load_host_id,
        })
    }
}
