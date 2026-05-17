use crate::error::PilotError;
pub struct ContextAssembler;
impl ContextAssembler { pub async fn assemble(&self, _: &str, _: &[String], _: &[String], _: &[String], _: &str) -> Result<AssembledContext, PilotError> { Ok(AssembledContext { prompt: String::new(), total_tokens: 0, tier1_tokens: 0, tier2_tokens: 0, tier3_tokens: 0 }) } }
pub struct AssembledContext { pub prompt: String, pub total_tokens: usize, pub tier1_tokens: usize, pub tier2_tokens: usize, pub tier3_tokens: usize }
