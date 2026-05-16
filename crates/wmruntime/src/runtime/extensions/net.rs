use super::super::*;

impl Runtime {
    pub fn install_net_extension(&mut self) -> Result<NetExtension, RuntimeError> {
        let get_host_id = 120;
        let post_host_id = 121;
        let net_backend = self.net_backend.clone();

        let _ = self.register_host_function(
            HostFunction::new(get_host_id, 1, 1, CAP_NETWORK),
            move |args| {
                let url = expect_string_arg(args, 0, "url")?;
                net_backend.borrow_mut().get(&url).map(Value::String)
            },
        );

        let net_backend = self.net_backend.clone();
        let _ = self.register_host_function(
            HostFunction::new(post_host_id, 2, 2, CAP_NETWORK),
            move |args| {
                let url = expect_string_arg(args, 0, "url")?;
                let body = expect_string_arg(args, 1, "body")?;
                net_backend
                    .borrow_mut()
                    .post(&url, &body)
                    .map(Value::String)
            },
        );

        let ids = self.extensions.register_extension(
            "ext.net",
            &[
                ExtensionFunctionSpec::new("get", get_host_id, 1, 1, CAP_NETWORK)
                    .with_return_type(ExtValueType::String),
                ExtensionFunctionSpec::new("post", post_host_id, 2, 2, CAP_NETWORK)
                    .with_return_type(ExtValueType::String),
            ],
        )?;

        Ok(NetExtension {
            get_ext_id: ids[0],
            post_ext_id: ids[1],
            get_host_id,
            post_host_id,
        })
    }
}
