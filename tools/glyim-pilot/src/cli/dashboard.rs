use crate::session::state::SessionState;
pub fn render_status_table(sessions: &[SessionState]) -> String { format!("{} sessions", sessions.len()) }
