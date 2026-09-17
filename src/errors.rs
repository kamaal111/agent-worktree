use std::fmt;

#[derive(Debug)]
pub struct AgentWorktreeError {
    pub message: String,
    pub is_usage: bool,
}

impl AgentWorktreeError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            is_usage: false,
        }
    }

    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            is_usage: true,
        }
    }
}

impl fmt::Display for AgentWorktreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AgentWorktreeError {}

impl From<std::io::Error> for AgentWorktreeError {
    fn from(error: std::io::Error) -> Self {
        AgentWorktreeError::new(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AgentWorktreeError>;

pub fn fail<T>(message: impl Into<String>) -> Result<T> {
    Err(AgentWorktreeError::new(message))
}
