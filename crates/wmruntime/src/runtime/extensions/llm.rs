use super::super::*;

impl Runtime {
    pub fn install_llm_extension(&mut self) -> Result<LlmExtension, RuntimeError> {
        let generate_host_id = 130;
        let llm_backend = self.llm_backend.clone();

        let _ = self.register_host_function(
            HostFunction::new(generate_host_id, 1, 1, CAP_ASYNC_IO),
            move |args| {
                let prompt = expect_string_arg(args, 0, "prompt")?;
                llm_backend
                    .borrow_mut()
                    .generate(&prompt)
                    .map(Value::String)
            },
        );

        let ids = self.extensions.register_extension(
            "ext.llm",
            &[
                ExtensionFunctionSpec::new("generate", generate_host_id, 1, 1, CAP_ASYNC_IO)
                    .with_return_type(ExtValueType::String),
            ],
        )?;

        Ok(LlmExtension {
            generate_ext_id: ids[0],
            generate_host_id,
        })
    }
}
