//! Task list sampler. The platform supplies per-thread CPU-time samples,
//! and the delta since the previous call becomes a percentage. The
//! denominator is wall-clock time, so one busy thread on one core reads
//! 100.

use alloc::string::String;
use alloc::vec::Vec;

use acid_platform::ThreadSample;

/// The task table holds 16 entries; when there are more threads, the
/// first 16 are kept.
pub const MAX_TRACKED_TASKS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskInfo {
    pub name: String,
    pub state: &'static str,
    pub cpu_percent: i32,
}

/// Maps a thread-state letter to a state name, using FreeRTOS's naming.
pub fn state_name(letter: char) -> &'static str {
    match letter {
        'R' => "running",
        'S' | 'D' => "blocked",
        'T' | 't' => "suspended",
        'Z' | 'X' => "deleted",
        _ => "?",
    }
}

#[derive(Default)]
pub struct TaskSampler {
    /// (id, cpu_ms) from the previous refresh.
    prev: Vec<(u64, u64)>,
    prev_at: Option<u64>,
    cur: Vec<TaskInfo>,
}

impl TaskSampler {
    /// Re-reads the task list; returns how many tasks are tracked. A task
    /// not seen last time reads 0%.
    pub fn refresh(&mut self, samples: &[ThreadSample], now_ms: u64) -> usize {
        let samples = &samples[..samples.len().min(MAX_TRACKED_TASKS)];
        let elapsed = self.prev_at.map_or(0, |at| now_ms.saturating_sub(at));
        self.cur = samples
            .iter()
            .map(|s| {
                let prev = self.prev.iter().find(|(id, _)| *id == s.id).map(|&(_, ms)| ms);
                let cpu_percent = match prev {
                    Some(p) if elapsed > 0 => {
                        let pct = s.cpu_ms.saturating_sub(p).saturating_mul(100) / elapsed;
                        pct.min(i32::MAX as u64) as i32
                    }
                    _ => 0,
                };
                TaskInfo { name: s.name.clone(), state: state_name(s.state), cpu_percent }
            })
            .collect();
        self.prev = samples.iter().map(|s| (s.id, s.cpu_ms)).collect();
        self.prev_at = Some(now_ms);
        self.cur.len()
    }

    pub fn count(&self) -> usize {
        self.cur.len()
    }

    pub fn info(&self, index: usize) -> Option<TaskInfo> {
        self.cur.get(index).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use alloc::vec::Vec;

    fn s(id: u64, name: &str, state: char, cpu_ms: u64) -> ThreadSample {
        ThreadSample { id, name: name.into(), state, cpu_ms }
    }

    #[test]
    fn first_refresh_reports_zero_cpu() {
        let mut t = TaskSampler::default();
        assert_eq!(t.refresh(&[s(1, "router", 'R', 500)], 1000), 1);
        assert_eq!(t.info(0), Some(TaskInfo { name: "router".into(), state: "running", cpu_percent: 0 }));
        assert_eq!(t.count(), 1);
    }

    #[test]
    fn cpu_is_the_tasks_time_delta_over_elapsed_time() {
        let mut t = TaskSampler::default();
        t.refresh(&[s(1, "a", 'R', 500), s(2, "b", 'S', 0)], 1000);
        t.refresh(&[s(1, "a", 'R', 750), s(2, "b", 'S', 100), s(3, "new", 'S', 900)], 2000);
        let cpu: Vec<i32> = (0..3).map(|i| t.info(i).unwrap().cpu_percent).collect();
        assert_eq!(cpu, [25, 10, 0], "a task not seen last time reads 0");
    }

    #[test]
    fn no_elapsed_time_reads_zero() {
        let mut t = TaskSampler::default();
        t.refresh(&[s(1, "a", 'R', 0)], 1000);
        t.refresh(&[s(1, "a", 'R', 50)], 1000);
        assert_eq!(t.info(0).unwrap().cpu_percent, 0);
    }

    #[test]
    fn tracks_at_most_sixteen_tasks() {
        let mut t = TaskSampler::default();
        let many: Vec<ThreadSample> = (0..20).map(|i| s(i, &format!("t{i}"), 'S', 0)).collect();
        assert_eq!(t.refresh(&many, 1000), MAX_TRACKED_TASKS);
        assert!(t.info(MAX_TRACKED_TASKS).is_none());
        assert_eq!(t.info(15).unwrap().name, "t15");
    }

    #[test]
    fn state_letters_map_to_freertos_style_names() {
        let got: Vec<&str> = ['R', 'S', 'D', 'T', 't', 'Z', 'X', 'I'].iter().map(|&c| state_name(c)).collect();
        assert_eq!(got, ["running", "blocked", "blocked", "suspended", "suspended", "deleted", "deleted", "?"]);
    }
}
