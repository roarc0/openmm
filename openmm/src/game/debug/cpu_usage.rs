use bevy::prelude::*;
use std::fs;
use std::time::Duration;

/// Resource to track CPU usage statistics via manual `/proc` parsing as a fallback.
#[derive(Resource)]
pub struct CpuStats {
    /// Percentage of one core consumed by this process (e.g., 100.0 = one core saturated).
    pub process_usage: f32,
    /// Percentage of the total system consumed across all cores.
    pub system_usage: f32,
    
    /// Smoothing factor for exponential moving average (0.0 to 1.0, lower is smoother)
    pub smoothing: f32,
    
    // Internal state for delta calculation
    last_update: Duration,
    prev_proc_ticks: u64,
    prev_sys_ticks: u64,
    prev_idle_ticks: u64,
}

impl Default for CpuStats {
    fn default() -> Self {
        Self {
            process_usage: 0.0,
            system_usage: 0.0,
            smoothing: 0.05, // Much smoother (5% new, 95% old)
            last_update: Duration::ZERO,
            prev_proc_ticks: 0,
            prev_sys_ticks: 0,
            prev_idle_ticks: 0,
        }
    }
}

/// System to update CPU statistics.
/// Throttled to ~500ms to avoid excessive I/O and give a stable delta.
pub fn update_cpu_stats_system(time: Res<Time>, mut stats: ResMut<CpuStats>) {
    let now = time.elapsed();
    if now.as_millis() - stats.last_update.as_millis() < 500 {
        return;
    }
    
    #[cfg(target_os = "linux")]
    {
        if let (Some(proc_ticks), Some((total_ticks, idle_ticks))) = (get_linux_process_ticks(), get_linux_system_ticks()) {
            if stats.prev_sys_ticks > 0 {
                let proc_delta = proc_ticks.saturating_sub(stats.prev_proc_ticks);
                let sys_delta = total_ticks.saturating_sub(stats.prev_sys_ticks);
                let idle_delta = idle_ticks.saturating_sub(stats.prev_idle_ticks);
                
                if sys_delta > 0 {
                    // Process usage: (proc_ticks / total_system_ticks_on_one_core)
                    // Since /proc/stat's 'cpu' line is aggregate across all cores, 
                    // and /proc/self/stat's 'utime+stime' is per-process, 
                    // we need to be careful with the scaling.
                    // A simpler way for 'per-core' process usage is using the time delta.
                    
                    let elapsed = (now - stats.last_update).as_secs_f32();
                    let alpha = stats.smoothing;
                    if elapsed > 0.0 {
                        // CLK_TCK is usually 100 on Linux.
                        // proc_usage_percent = (delta_ticks / CLK_TCK) / elapsed_seconds * 100
                        let raw_proc = (proc_delta as f32 / 100.0) / elapsed * 100.0;
                        stats.process_usage = stats.process_usage * (1.0 - alpha) + raw_proc * alpha;
                    }
                    
                    // System usage: (total - idle) / total
                    let busy_delta = sys_delta.saturating_sub(idle_delta);
                    let raw_sys = (busy_delta as f32 / sys_delta as f32) * 100.0;
                    stats.system_usage = stats.system_usage * (1.0 - alpha) + raw_sys * alpha;
                }
            }
            
            stats.prev_proc_ticks = proc_ticks;
            stats.prev_sys_ticks = total_ticks;
            stats.prev_idle_ticks = idle_ticks;
            stats.last_update = now;
        }
    }
}

#[cfg(target_os = "linux")]
fn get_linux_process_ticks() -> Option<u64> {
    let data = fs::read_to_string("/proc/self/stat").ok()?;
    // The process name is in parentheses and may contain spaces.
    // Find the last ')' to skip it safely.
    let start_idx = data.rfind(')')?;
    let fields: Vec<&str> = data[start_idx+1..].split_whitespace().collect();
    
    // Indices relative to the field after the ')'
    // utime is field 14, stime is 15 in the full file.
    // field[0] is state, field[1] is ppid, etc.
    // utime is field[11], stime is field[12]
    if fields.len() > 12 {
        let utime: u64 = fields[11].parse().ok()?;
        let stime: u64 = fields[12].parse().ok()?;
        return Some(utime + stime);
    }
    None
}

#[cfg(target_os = "linux")]
fn get_linux_system_ticks() -> Option<(u64, u64)> {
    let data = fs::read_to_string("/proc/stat").ok()?;
    let line = data.lines().next()?;
    let fields: Vec<&str> = line.split_whitespace().skip(1).collect();
    
    // /proc/stat 'cpu' line: user nice system idle iowait irq softirq steal guest guest_nice
    // idle is the 4th field (index 3)
    let mut total: u64 = 0;
    let mut idle: u64 = 0;
    for (i, f) in fields.iter().enumerate() {
        if let Ok(v) = f.parse::<u64>() {
            total += v;
            if i == 3 {
                idle = v;
            }
        }
    }
    Some((total, idle))
}
