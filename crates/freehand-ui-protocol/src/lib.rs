//! UI-facing commands, events, and projections for Freehand.

pub use freehand_debug::{
    DebugEvent, DebugScenePosition, DebugSemanticPosition, DebugStateSnapshot, DebugTraceEnvelope,
};

mod dto;
pub use dto::*;

mod adp_wire;
pub use adp_wire::*;

mod adp_descriptor;
pub use adp_descriptor::*;

mod ports;
pub use ports::*;

mod state;
pub use state::*;

mod validate;
pub use validate::*;

mod projection;
pub use projection::*;

#[cfg(test)]
mod tests;
