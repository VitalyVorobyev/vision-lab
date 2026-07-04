//! Shared communication contracts for Vision Lab components.
//!
//! This crate intentionally contains no Tauri, camera, detector, or recorder
//! implementation details. It defines the semantic envelope shared by the
//! in-process prototype and a future IPC transport.

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ComponentId {
    pub component_type: String,
    pub component_name: String,
}

impl ComponentId {
    pub fn new(component_type: impl Into<String>, component_name: impl Into<String>) -> Self {
        Self {
            component_type: component_type.into(),
            component_name: component_name.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuntimeInstanceId(pub String);

impl RuntimeInstanceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl Default for RuntimeInstanceId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentIdentity {
    pub component: ComponentId,
    pub instance_id: RuntimeInstanceId,
    pub version: String,
}

impl ComponentIdentity {
    pub fn new(component_type: &str, component_name: &str, version: impl Into<String>) -> Self {
        Self {
            component: ComponentId::new(component_type, component_name),
            instance_id: RuntimeInstanceId::new(),
            version: version.into(),
        }
    }
}

macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord,
        )]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

uuid_id!(CommandId);
uuid_id!(CorrelationId);
uuid_id!(EventId);
uuid_id!(OperationId);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Versioned<T> {
    pub source: ComponentIdentity,
    #[serde(with = "system_time_serde")]
    pub timestamp: SystemTime,
    pub revision: u64,
    pub value: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope<T> {
    pub event_id: EventId,
    pub source: ComponentIdentity,
    #[serde(with = "system_time_serde")]
    pub timestamp: SystemTime,
    pub sequence: u64,
    pub correlation_id: Option<CorrelationId>,
    pub payload: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandReceipt {
    pub command_id: CommandId,
    pub operation_id: Option<OperationId>,
    pub accepted_revision: u64,
}

#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum ApiError {
    #[error("component is unavailable: {0}")]
    Unavailable(String),
    #[error("command rejected: {0}")]
    Rejected(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("operation failed: {0}")]
    Failed(String),
    #[error("stream closed")]
    StreamClosed,
}

pub type EventStream<T> = tokio::sync::broadcast::Receiver<EventEnvelope<T>>;

pub fn now() -> SystemTime {
    SystemTime::now()
}

pub fn versioned<T>(source: ComponentIdentity, revision: u64, value: T) -> Versioned<T> {
    Versioned {
        source,
        timestamp: now(),
        revision,
        value,
    }
}

pub fn event<T>(
    source: ComponentIdentity,
    sequence: u64,
    correlation_id: Option<CorrelationId>,
    payload: T,
) -> EventEnvelope<T> {
    EventEnvelope {
        event_id: EventId::new(),
        source,
        timestamp: now(),
        sequence,
        correlation_id,
        payload,
    }
}

pub mod system_time_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let millis = time
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        serializer.serialize_u64(millis)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(UNIX_EPOCH + Duration::from_millis(millis))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        assert_ne!(CommandId::new(), CommandId::new());
        assert_ne!(EventId::new(), EventId::new());
    }
}
