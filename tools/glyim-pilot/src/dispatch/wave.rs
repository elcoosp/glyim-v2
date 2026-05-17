use crate::dispatch::provider_pool::ProviderPool;

#[derive(Debug, Clone, PartialEq)]
pub enum DispatchStrategy { MostSlotsFirst, RoundRobin, LeastLoaded }

pub struct StreamAssignment { pub stream_id: String, pub provider_id: String }

pub fn dispatch_wave(
    stream_ids: &[String], pool: &mut ProviderPool, strategy: &DispatchStrategy,
) -> Vec<StreamAssignment> {
    Vec::new()
}
