use crate::dispatch::provider_pool::ProviderPool;
use crate::error::PilotError;
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq)]
pub enum DispatchStrategy {
    MostSlotsFirst,
    RoundRobin,
    LeastLoaded,
}

impl std::str::FromStr for DispatchStrategy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "most_slots_first" => Ok(Self::MostSlotsFirst),
            "round_robin" => Ok(Self::RoundRobin),
            "least_loaded" => Ok(Self::LeastLoaded),
            _ => Err(format!("unknown strategy: {s}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreamAssignment {
    pub stream_id: String,
    pub provider_id: String,
}

pub fn dispatch_wave(
    stream_ids: &[String],
    pool: &mut ProviderPool,
    strategy: &DispatchStrategy,
) -> Result<Vec<StreamAssignment>, PilotError> {
    let mut unassigned: VecDeque<String> = stream_ids.iter().cloned().collect();
    let mut assignments = Vec::new();

    match strategy {
        DispatchStrategy::MostSlotsFirst => {
            while let Some((best_id, _)) = pool.most_slots_available() {
                if pool.allocate(&best_id).is_ok() {
                    if let Some(id) = unassigned.pop_front() {
                        assignments.push(StreamAssignment {
                            stream_id: id,
                            provider_id: best_id,
                        });
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        DispatchStrategy::RoundRobin => {
            let providers = pool.provider_ids();
            if providers.is_empty() {
                return Ok(assignments);
            }
            let mut idx = 0;
            let mut consecutive_fails = 0;
            while let Some(id) = unassigned.pop_front() {
                let pid = &providers[idx % providers.len()];
                if pool.allocate(pid).is_ok() {
                    assignments.push(StreamAssignment {
                        stream_id: id,
                        provider_id: pid.clone(),
                    });
                    consecutive_fails = 0;
                } else {
                    unassigned.push_front(id);
                    consecutive_fails += 1;
                    if consecutive_fails > providers.len() * 2 {
                        break;
                    }
                }
                idx += 1;
            }
        }
        DispatchStrategy::LeastLoaded => {
            while let Some(id) = unassigned.pop_front() {
                let mut providers = pool.provider_ids();
                // Sort by available slots descending (most free first)
                providers.sort_by(|a, b| {
                    pool.available_slots(b).cmp(&pool.available_slots(a))
                });
                let mut assigned = false;
                for pid in providers {
                    if pool.allocate(&pid).is_ok() {
                        assignments.push(StreamAssignment {
                            stream_id: id,
                            provider_id: pid,
                        });
                        assigned = true;
                        break;
                    }
                }
                if !assigned {
                    break;
                }
            }
        }
    }
    Ok(assignments)
}
