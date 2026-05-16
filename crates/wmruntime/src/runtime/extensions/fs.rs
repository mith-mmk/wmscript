use super::super::*;

impl Runtime {
    pub fn install_fs_extension(&mut self) -> Result<FsExtension, RuntimeError> {
        let read_host_id = 100;
        let write_host_id = 101;
        let exists_host_id = 102;

        let _ = self.register_host_function(
            HostFunction::new(read_host_id, 1, 1, CAP_FILE_SYSTEM),
            |args| read_text_file(args),
        );
        let _ = self.register_host_function(
            HostFunction::new(write_host_id, 2, 2, CAP_FILE_SYSTEM),
            |args| write_text_file(args),
        );
        let _ = self.register_host_function(
            HostFunction::new(exists_host_id, 1, 1, CAP_FILE_SYSTEM),
            |args| exists_text_file(args),
        );

        let ids = self.extensions.register_extension(
            "ext.fs",
            &[
                ExtensionFunctionSpec::new("read", read_host_id, 1, 1, CAP_FILE_SYSTEM)
                    .with_return_type(ExtValueType::String),
                ExtensionFunctionSpec::new("write", write_host_id, 2, 2, CAP_FILE_SYSTEM)
                    .with_return_type(ExtValueType::Nil),
                ExtensionFunctionSpec::new("exists", exists_host_id, 1, 1, CAP_FILE_SYSTEM)
                    .with_return_type(ExtValueType::Bool),
            ],
        )?;

        Ok(FsExtension {
            read_ext_id: ids[0],
            write_ext_id: ids[1],
            exists_ext_id: ids[2],
            read_host_id,
            write_host_id,
            exists_host_id,
        })
    }
}
