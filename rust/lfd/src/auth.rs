use crate::registration::ConnectionValidator;

/// Auth context for HTTP API.
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub enabled: bool,
    pub registered: bool,
    pub validator: Option<ConnectionValidator>,
}

impl AuthContext {
    pub fn new(enabled: bool, registered: bool, validator: Option<ConnectionValidator>) -> Self {
        Self {
            enabled,
            registered,
            validator,
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            registered: false,
            validator: None,
        }
    }
}
