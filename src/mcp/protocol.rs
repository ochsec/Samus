use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Protocol version information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl Version {
    pub fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self { major, minor, patch }
    }

    pub fn is_compatible_with(&self, other: &Version) -> bool {
        self.major == other.major && self.minor >= other.minor
    }
}

/// Represents a tool's capabilities and requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub version: Version,
    pub schema: serde_json::Value,
}

/// Server state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerState {
    pub id: String,
    pub status: ServerStatus,
    pub tools: Vec<ToolDefinition>,
    pub version: Version,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServerStatus {
    Starting,
    Ready,
    Error,
    Shutdown,
}

/// Protocol error types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProtocolError {
    VersionMismatch { client: Version, server: Version },
    InvalidMessage(String),
    InvalidCommand(String),
    ExecutionError(String),
    StateError(String),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VersionMismatch { client, server } => {
                write!(f, "Protocol version mismatch - client: {:?}, server: {:?}", client, server)
            }
            Self::InvalidMessage(msg) => write!(f, "Invalid message: {}", msg),
            Self::InvalidCommand(cmd) => write!(f, "Invalid command: {}", cmd),
            Self::ExecutionError(err) => write!(f, "Execution error: {}", err),
            Self::StateError(err) => write!(f, "State error: {}", err),
        }
    }
}

/// Message types for client-server communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    HandshakeRequest,
    HandshakeResponse,
    ToolRegistration,
    StateUpdate,
    CommandRequest,
    CommandResponse,
    Error,
}

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageType::HandshakeRequest => write!(f, "handshake_request"),
            MessageType::HandshakeResponse => write!(f, "handshake_response"),
            MessageType::ToolRegistration => write!(f, "tool_registration"),
            MessageType::StateUpdate => write!(f, "state_update"),
            MessageType::CommandRequest => write!(f, "command_request"),
            MessageType::CommandResponse => write!(f, "command_response"),
            MessageType::Error => write!(f, "error"),
        }
    }
}

/// A message in the MCP protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpMessage {
    pub id: String,
    pub message_type: MessageType,
    pub payload: serde_json::Value,
}

/// A request in the MCP protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub id: String,
    pub command: String,
    pub args: HashMap<String, serde_json::Value>,
}

/// A response in the MCP protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub id: String,
    pub success: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<ProtocolError>,
}

impl McpRequest {
    pub fn new(command: &str) -> Self {
        McpRequest {
            id: uuid::Uuid::new_v4().to_string(),
            command: command.to_string(),
            args: HashMap::new(),
        }
    }

    pub fn with_arg<T: Serialize>(mut self, key: &str, value: T) -> Result<Self, serde_json::Error> {
        let json_value = serde_json::to_value(value)?;
        self.args.insert(key.to_string(), json_value);
        Ok(self)
    }

    pub fn to_message(&self) -> Result<McpMessage, serde_json::Error> {
        Ok(McpMessage {
            id: self.id.clone(),
            message_type: MessageType::CommandRequest,
            payload: serde_json::to_value(self)?,
        })
    }
}

impl McpResponse {
    pub fn success(id: &str, result: serde_json::Value) -> Self {
        McpResponse {
            id: id.to_string(),
            success: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: &str, error: ProtocolError) -> Self {
        McpResponse {
            id: id.to_string(),
            success: false,
            result: None,
            error: Some(error),
        }
    }

    pub fn to_message(&self) -> Result<McpMessage, serde_json::Error> {
        Ok(McpMessage {
            id: self.id.clone(),
            message_type: MessageType::CommandResponse,
            payload: serde_json::to_value(self)?,
        })
    }
}

// Protocol traits
pub trait McpProtocol {
    fn handle_message(&mut self, message: McpMessage) -> Result<Option<McpMessage>, ProtocolError>;
    fn get_state(&self) -> ServerState;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_compatibility() {
        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new(1, 1, 0);
        let v3 = Version::new(2, 0, 0);

        assert!(v1.is_compatible_with(&v1));
        assert!(v2.is_compatible_with(&v1));
        assert!(!v1.is_compatible_with(&v2));
        assert!(!v1.is_compatible_with(&v3));
    }

    #[test]
    fn test_message_serialization() {
        let request = McpRequest::new("test_command")
            .with_arg("key", "value")
            .unwrap();
        let message = request.to_message().unwrap();

        let serialized = serde_json::to_string(&message).unwrap();
        let deserialized: McpMessage = serde_json::from_str(&serialized).unwrap();

        assert_eq!(message.id, deserialized.id);
        assert!(matches!(deserialized.message_type, MessageType::CommandRequest));
    }

    #[test]
    fn test_error_handling() {
        let error = ProtocolError::InvalidCommand("unknown_command".to_string());
        let response = McpResponse::error("test_id", error);

        assert!(!response.success);
        assert!(response.result.is_none());
        assert!(response.error.is_some());
    }

    #[test]
    fn test_server_state() {
        let state = ServerState {
            id: "test_server".to_string(),
            status: ServerStatus::Ready,
            tools: vec![
                ToolDefinition {
                    name: "test_tool".to_string(),
                    description: "A test tool".to_string(),
                    version: Version::new(1, 0, 0),
                    schema: serde_json::json!({}),
                }
            ],
            version: Version::new(1, 0, 0),
        };

        let serialized = serde_json::to_string(&state).unwrap();
        let deserialized: ServerState = serde_json::from_str(&serialized).unwrap();

        assert_eq!(state.id, deserialized.id);
        assert_eq!(state.status, deserialized.status);
        assert_eq!(state.tools.len(), deserialized.tools.len());
    }
}
