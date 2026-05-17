use crate::dispatch::provider_pool::ProviderPool;
use crate::error::PilotError;

#[derive(Debug, Clone)]
pub struct RateLimitContext {
    pub stream_id: String,
    pub turn: u32,
    pub commits: u32,
    pub brief_summary: String,
    pub max_reassign_attempts: u32,
}

#[derive(Debug, Clone)]
pub enum RateLimitAction {
    Failover {
        new_provider_id: String,
        failover_prompt: String,
    },
    RetryAfter {
        provider_id: String,
        delay_secs: u64,
    },
    Escalate {
        reason: String,
    },
}

pub fn handle_rate_limit(
    pool: &mut ProviderPool,
    provider_id: &str,
    base_delay_secs: u64,
    max_delay_secs: u64,
    attempt: u32,
    ctx: &RateLimitContext,
) -> Result<RateLimitAction, PilotError> {
    // Apply cooldown to the failing provider
    let cooldown = pool
        .get_config(provider_id)
        .map(|c| c.rate_limit_cooldown)
        .unwrap_or(base_delay_secs);
    pool.cooldown(provider_id, cooldown);
    tracing::warn!(
        provider_id = provider_id,
        cooldown_secs = cooldown,
        attempt = attempt,
        "rate limit detected"
    );

    // Try failover if within reassign limits
    if attempt <= ctx.max_reassign_attempts {
        if let Some((new_id, _)) = pool.most_slots_available() {
            if new_id != provider_id {
                let failover_prompt = format!(
                    "Session {} moved from {} due to rate limit. Turns: {}, Commits: {}. Brief: {}",
                    ctx.stream_id, provider_id, ctx.turn, ctx.commits, ctx.brief_summary
                );
                return Ok(RateLimitAction::Failover {
                    new_provider_id: new_id,
                    failover_prompt,
                });
            }
        }
    }

    // Calculate backoff with jitter
    let exp = base_delay_secs.saturating_mul(2u64.saturating_pow(attempt)).min(max_delay_secs);
    let stagger = (attempt as u64 * 17) % ((exp as f64 * 0.2).max(1.0) as u64);
    let delay = exp.saturating_add(stagger).min(max_delay_secs);

    if attempt < 5 {
        Ok(RateLimitAction::RetryAfter {
            provider_id: provider_id.to_string(),
            delay_secs: delay,
        })
    } else {
        Ok(RateLimitAction::Escalate {
            reason: format!("rate limit on {} after {} attempts", provider_id, attempt),
        })
    }
}
