use openmm_data::evt::EvtStep;

/// Log steps skipped by a forward jump. `from_pc`..`to_pc` are step *indices* (not step numbers).
pub(crate) fn log_skipped(steps: &[EvtStep], from_pc: usize, to_pc: usize, reason: &str) {
    match to_pc.cmp(&from_pc) {
        std::cmp::Ordering::Equal => {}
        std::cmp::Ordering::Less => {
            bevy::log::debug!("  ↺ backward jump ({})", reason);
        }
        std::cmp::Ordering::Greater => {
            for s in &steps[from_pc..to_pc.min(steps.len())] {
                bevy::log::info!("  ↷ [step {}] skip({}): {}", s.step, reason, s.event);
            }
        }
    }
}

/// Log all remaining steps in the sequence as unreachable (sequence ended early).
pub(crate) fn log_tail_unreachable(steps: &[EvtStep], from_pc: usize) {
    for s in steps.get(from_pc..).unwrap_or(&[]) {
        bevy::log::info!("  ⊘ [step {}] unreachable: {}", s.step, s.event);
    }
}

/// Helper to handle conditional jumps in event scripts.
/// Finds the target step index, logs skipped steps, and updates the program counter.
/// Returns true if execution should continue, false if the sequence was terminated.
pub(crate) fn execute_conditional_jump(steps: &[EvtStep], pc: &mut usize, jump_step: u8, reason: &str) -> bool {
    if let Some(target_idx) = steps.iter().position(|s| s.step >= jump_step) {
        log_skipped(steps, *pc, target_idx, reason);
        *pc = target_idx;
        true
    } else {
        log_tail_unreachable(steps, *pc);
        false
    }
}
