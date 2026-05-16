use wmvm::HostError;

/// Backend interface for `ext.net`.
pub trait NetBackend {
    fn get(&mut self, url: &str) -> Result<String, HostError>;

    fn post(&mut self, url: &str, body: &str) -> Result<String, HostError>;
}

/// Backend interface for `ext.llm`.
pub trait LlmBackend {
    fn generate(&mut self, prompt: &str) -> Result<String, HostError>;
}

pub(super) struct DisabledNetBackend;

impl NetBackend for DisabledNetBackend {
    fn get(&mut self, url: &str) -> Result<String, HostError> {
        Err(HostError::Failed(format!(
            "network backend disabled for GET {url}"
        )))
    }

    fn post(&mut self, url: &str, body: &str) -> Result<String, HostError> {
        Err(HostError::Failed(format!(
            "network backend disabled for POST {url} with {} bytes",
            body.len()
        )))
    }
}

pub(super) struct DisabledLlmBackend;

impl LlmBackend for DisabledLlmBackend {
    fn generate(&mut self, prompt: &str) -> Result<String, HostError> {
        Err(HostError::Failed(format!(
            "llm backend disabled for prompt with {} bytes",
            prompt.len()
        )))
    }
}
