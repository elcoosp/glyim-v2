use crate::dispatch::provider_pool::ProviderPool;

pub struct RateLimitContext {
    pub stream_id: String, pub turn: u32, pub commits: u32,
    pub brief_summary: String, pub max_reassign_attempts: u32,
}

pub fn handle_rate_limit(
    pool: &mut ProviderPool, provider_id: &str, base_delay_secs: u64,
    max_delay_secs: u64, attempt: u32, ctx: &RateLimitContext,
) -> Result<(), String> {
    Ok(())
}
