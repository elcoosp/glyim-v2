use crate::error::PilotError;
use super::state::{SessionState, StreamStatus};
pub struct TransitionValidator;
impl TransitionValidator { pub fn validate(_: &SessionState, _: StreamStatus) -> Result<(), PilotError> { Ok(()) } pub fn transition(s: &mut SessionState, ns: StreamStatus) -> Result<(), PilotError> { s.status = ns; Ok(()) } }
