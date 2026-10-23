//! The automation wire contract: plain, versioned data that an out-of-process
//! caller (the MCP server, later plugins) exchanges with the running app.
//! It knows nothing of egui, MCP or the native `.odx` schema.

pub mod command_ops;
pub mod convert;
pub mod entity;
mod entity_bounds;
mod entity_leaders;
pub mod envelope;
pub mod error;
pub mod event_ops;
pub mod file_ops;
pub mod frame;
pub mod handshake;
pub mod idempotency;
pub mod ids;
pub mod instance;
pub mod layer_ops;
pub mod limits;
pub mod measure;
pub mod ops;
pub mod paths;
pub mod session_ops;
pub mod version;
pub mod view;
pub mod write;

pub use envelope::{Request, Response, Scope};
pub use error::{ApiError, ErrorCode};
pub use ids::{IdError, WireId};
pub use version::{negotiate, API_VERSION};
