pub const ALIGN_MAX: u64 = 16;
pub const ALIGN_MIN: u64 = 1;
pub const DEFAULT_STACK_SIZE: usize = 8 * 1024 * 1024;

pub fn validate_alignment(align: u64) -> Result<(), String> {
    if align == 0 { return Err("alignment must be non-zero".into()); }
    if !align.is_power_of_two() { return Err(format!("alignment {} is not a power of two", align)); }
    if align > ALIGN_MAX { return Err(format!("alignment {} exceeds ALIGN_MAX ({})", align, ALIGN_MAX)); }
    Ok(())
}
