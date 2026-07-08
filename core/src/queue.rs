// Print queue management with priority scheduling
// Licensed under Apache 2.0
//
// Manages a queue of print jobs with priority-based scheduling,
// status tracking, and reordering capability.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Priority level for a print job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum Priority {
    Low = 0,
    #[default]
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Priority::Low => write!(f, "low"),
            Priority::Normal => write!(f, "normal"),
            Priority::High => write!(f, "high"),
            Priority::Critical => write!(f, "critical"),
        }
    }
}

/// Current status of a print job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Queued,
    Printing,
    Paused,
    Completed,
    Failed(String),
    Cancelled,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::Queued => write!(f, "queued"),
            JobStatus::Printing => write!(f, "printing"),
            JobStatus::Paused => write!(f, "paused"),
            JobStatus::Completed => write!(f, "completed"),
            JobStatus::Failed(reason) => write!(f, "failed: {}", reason),
            JobStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// A print job ready for the queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintJob {
    /// Unique identifier.
    pub id: String,
    /// Human-readable job name.
    pub name: String,
    /// G-code content to execute.
    pub gcode: String,
    /// Priority for scheduling.
    pub priority: Priority,
    /// Current job status.
    pub status: JobStatus,
    /// Material name (for display / filtering).
    pub material_name: String,
    /// Estimated print time from slicing (seconds).
    pub estimated_time_s: f64,
    /// Timestamp when the job was enqueued (Unix epoch seconds).
    pub enqueued_at: f64,
    /// Timestamp when the job started printing (Unix epoch seconds).
    pub started_at: Option<f64>,
    /// Optional description or notes.
    pub description: String,
}

impl PrintJob {
    /// Create a new print job.
    pub fn new(
        id: String,
        name: String,
        gcode: String,
        material_name: String,
        estimated_time_s: f64,
    ) -> Self {
        Self {
            id,
            name,
            gcode,
            priority: Priority::Normal,
            status: JobStatus::Queued,
            material_name,
            estimated_time_s,
            enqueued_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0),
            started_at: None,
            description: String::new(),
        }
    }

    /// Create a new print job with a specific priority.
    pub fn with_priority(
        id: String,
        name: String,
        gcode: String,
        material_name: String,
        estimated_time_s: f64,
        priority: Priority,
    ) -> Self {
        let mut job = Self::new(id, name, gcode, material_name, estimated_time_s);
        job.priority = priority;
        job
    }

    /// Set job status and return self for chaining.
    pub fn with_status(mut self, status: JobStatus) -> Self {
        self.status = status;
        self
    }

    /// Check if the job is eligible to start printing.
    pub fn can_start(&self) -> bool {
        matches!(self.status, JobStatus::Queued)
    }
}

/// Priority-sorted print queue.
///
/// Jobs are ordered by priority (higher = first), then by enqueue time
/// (FIFO within the same priority level).
#[derive(Debug, Clone)]
pub struct PrintQueue {
    /// Internal list of jobs.
    jobs: Vec<PrintJob>,
    /// Maximum number of completed jobs to retain in history.
    max_history: usize,
    /// Index of the currently printing job, if any.
    current_index: Option<usize>,
}

impl Default for PrintQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl PrintQueue {
    /// Create an empty print queue.
    pub fn new() -> Self {
        Self {
            jobs: Vec::new(),
            max_history: 100,
            current_index: None,
        }
    }

    /// Create a print queue with a specific history limit.
    pub fn with_max_history(max_history: usize) -> Self {
        Self {
            jobs: Vec::new(),
            max_history,
            current_index: None,
        }
    }

    /// Add a job to the queue. Returns the index of the inserted job.
    pub fn enqueue(&mut self, job: PrintJob) -> usize {
        self.jobs.push(job);
        self.sort();
        self.jobs.len() - 1
    }

    /// Remove a job by its ID.
    pub fn remove(&mut self, id: &str) -> Option<PrintJob> {
        let idx = self.jobs.iter().position(|j| j.id == id)?;
        let job = self.jobs.remove(idx);
        self.update_current_index();
        Some(job)
    }

    /// Get the next job that should be printed (highest priority, oldest first).
    pub fn next(&self) -> Option<&PrintJob> {
        self.jobs.iter().find(|j| j.can_start())
    }

    /// Start the next available job. Returns the job that started, or None.
    pub fn start_next(&mut self) -> Option<&PrintJob> {
        let idx = self.jobs.iter().position(|j| j.can_start())?;
        let started_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        self.jobs[idx].status = JobStatus::Printing;
        self.jobs[idx].started_at = Some(started_at);
        self.current_index = Some(idx);
        Some(&self.jobs[idx])
    }

    /// Mark the current job as completed.
    pub fn complete_current(&mut self) -> Option<String> {
        let idx = self.current_index?;
        if idx < self.jobs.len() {
            self.jobs[idx].status = JobStatus::Completed;
            let id = self.jobs[idx].id.clone();
            self.current_index = None;
            self.trim_history();
            Some(id)
        } else {
            self.current_index = None;
            None
        }
    }

    /// Mark the current job as failed.
    pub fn fail_current(&mut self, reason: String) -> Option<String> {
        let idx = self.current_index?;
        if idx < self.jobs.len() {
            self.jobs[idx].status = JobStatus::Failed(reason);
            let id = self.jobs[idx].id.clone();
            self.current_index = None;
            self.trim_history();
            Some(id)
        } else {
            self.current_index = None;
            None
        }
    }

    /// Pause or resume the current job.
    pub fn toggle_pause_current(&mut self) -> Option<bool> {
        let idx = self.current_index?;
        if idx >= self.jobs.len() {
            return None;
        }
        match self.jobs[idx].status {
            JobStatus::Printing => {
                self.jobs[idx].status = JobStatus::Paused;
                Some(true) // now paused
            }
            JobStatus::Paused => {
                self.jobs[idx].status = JobStatus::Printing;
                Some(false) // now resumed
            }
            _ => None,
        }
    }

    /// Cancel a job by ID. Removes it if still queued, marks as cancelled if
    /// it was printing.
    pub fn cancel(&mut self, id: &str) -> bool {
        if let Some(idx) = self.jobs.iter().position(|j| j.id == id) {
            if self.jobs[idx].can_start() {
                self.jobs[idx].status = JobStatus::Cancelled;
                self.jobs.remove(idx);
                self.update_current_index();
                return true;
            }
            if self.current_index == Some(idx) {
                self.jobs[idx].status = JobStatus::Cancelled;
                self.current_index = None;
                return true;
            }
            // Completed / failed jobs: just remove.
            self.jobs.remove(idx);
            self.update_current_index();
            return true;
        }
        false
    }

    /// Change the priority of a job.
    pub fn set_priority(&mut self, id: &str, new_priority: Priority) -> bool {
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
            job.priority = new_priority;
            self.sort();
            true
        } else {
            false
        }
    }

    /// Reorder a job to a specific position (0-indexed).
    pub fn move_to_position(&mut self, id: &str, position: usize) -> bool {
        let current_pos = match self.jobs.iter().position(|j| j.id == id) {
            Some(p) => p,
            None => return false,
        };
        if current_pos == position {
            return true;
        }
        let job = self.jobs.remove(current_pos);
        let target = position.min(self.jobs.len());
        self.jobs.insert(target, job);
        self.update_current_index();
        true
    }

    /// Get a reference to a job by ID.
    pub fn get(&self, id: &str) -> Option<&PrintJob> {
        self.jobs.iter().find(|j| j.id == id)
    }

    /// Get a mutable reference to a job by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut PrintJob> {
        self.jobs.iter_mut().find(|j| j.id == id)
    }

    /// Get all jobs in priority order.
    pub fn all_jobs(&self) -> &[PrintJob] {
        &self.jobs
    }

    /// Get all jobs with a specific status.
    pub fn jobs_by_status(&self, status: JobStatus) -> Vec<&PrintJob> {
        self.jobs.iter().filter(|j| j.status == status).collect()
    }

    /// Number of queued jobs (ready to print).
    pub fn queued_count(&self) -> usize {
        self.jobs.iter().filter(|j| j.can_start()).count()
    }

    /// Total number of jobs in the queue (including completed/failed).
    pub fn total_count(&self) -> usize {
        self.jobs.len()
    }

    /// Clear all completed and failed jobs.
    pub fn clear_history(&mut self) {
        self.jobs.retain(|j| {
            !matches!(
                j.status,
                JobStatus::Completed | JobStatus::Failed(_) | JobStatus::Cancelled
            )
        });
        self.update_current_index();
    }

    /// Get the current printing job.
    pub fn current_job(&self) -> Option<&PrintJob> {
        let idx = self.current_index?;
        self.jobs.get(idx)
    }

    /// Estimated time remaining for the current job.
    pub fn estimated_remaining_seconds(&self) -> f64 {
        self.current_job()
            .map(|j| j.estimated_time_s)
            .unwrap_or(0.0)
    }

    // -- Private helpers --

    fn sort(&mut self) {
        // Stable sort: higher priority first, then earlier enqueue time.
        self.jobs.sort_by(|a, b| {
            b.priority.cmp(&a.priority).then_with(|| {
                a.enqueued_at
                    .partial_cmp(&b.enqueued_at)
                    .unwrap_or(Ordering::Equal)
            })
        });
        self.update_current_index();
    }

    fn update_current_index(&mut self) {
        if let Some(idx) = self.current_index {
            if idx >= self.jobs.len()
                || !matches!(
                    self.jobs[idx].status,
                    JobStatus::Printing | JobStatus::Paused
                )
            {
                self.current_index = None;
            } else {
                // Update index after sort by finding the same job.
                let id = self.jobs[idx].id.clone();
                self.current_index = self.jobs.iter().position(|j| j.id == id);
            }
        }
    }

    fn trim_history(&mut self) {
        let completed_count = self
            .jobs
            .iter()
            .filter(|j| {
                matches!(
                    j.status,
                    JobStatus::Completed | JobStatus::Failed(_) | JobStatus::Cancelled
                )
            })
            .count();

        if completed_count > self.max_history {
            // Remove oldest completed/failed/cancelled entries.
            let to_remove = completed_count - self.max_history;
            let mut removed = 0;
            self.jobs.retain(|j| {
                if removed >= to_remove {
                    return true;
                }
                let is_done = matches!(
                    j.status,
                    JobStatus::Completed | JobStatus::Failed(_) | JobStatus::Cancelled
                );
                if is_done {
                    removed += 1;
                    false
                } else {
                    true
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_job(id: &str, priority: Priority, name: &str) -> PrintJob {
        PrintJob::with_priority(
            id.to_string(),
            name.to_string(),
            "G28\n".to_string(),
            "TestMaterial".to_string(),
            60.0,
            priority,
        )
    }

    #[test]
    fn test_enqueue_and_next() {
        let mut queue = PrintQueue::new();
        queue.enqueue(make_job("1", Priority::Normal, "Job 1"));
        queue.enqueue(make_job("2", Priority::High, "Job 2"));

        let next = queue.next();
        assert!(next.is_some());
        assert_eq!(next.unwrap().id, "2"); // higher priority first
    }

    #[test]
    fn test_priority_ordering() {
        let mut queue = PrintQueue::new();
        queue.enqueue(make_job("low", Priority::Low, "Low"));
        queue.enqueue(make_job("critical", Priority::Critical, "Critical"));
        queue.enqueue(make_job("high", Priority::High, "High"));

        let jobs = queue.all_jobs();
        assert_eq!(jobs[0].id, "critical");
        assert_eq!(jobs[1].id, "high");
        assert_eq!(jobs[2].id, "low");
    }

    #[test]
    fn test_fifo_within_priority() {
        let mut queue = PrintQueue::new();
        queue.enqueue(make_job("a", Priority::Normal, "A"));
        queue.enqueue(make_job("b", Priority::Normal, "B"));
        queue.enqueue(make_job("c", Priority::Normal, "C"));

        assert_eq!(queue.next().unwrap().id, "a");
    }

    #[test]
    fn test_start_next_marks_printing() {
        let mut queue = PrintQueue::new();
        queue.enqueue(make_job("1", Priority::Normal, "Job 1"));
        let started = queue.start_next();
        assert!(started.is_some());
        assert_eq!(started.unwrap().status, JobStatus::Printing);
        assert!(queue.current_job().is_some());
    }

    #[test]
    fn test_complete_current() {
        let mut queue = PrintQueue::new();
        queue.enqueue(make_job("1", Priority::Normal, "Job 1"));
        queue.start_next();
        let completed_id = queue.complete_current();
        assert_eq!(completed_id.unwrap(), "1");
        assert!(queue.current_job().is_none());
    }

    #[test]
    fn test_pause_resume() {
        let mut queue = PrintQueue::new();
        queue.enqueue(make_job("1", Priority::Normal, "Job 1"));
        queue.start_next();
        assert!(queue.toggle_pause_current().unwrap()); // paused
        assert_eq!(queue.current_job().unwrap().status, JobStatus::Paused);
        assert!(!queue.toggle_pause_current().unwrap()); // resumed
    }

    #[test]
    fn test_set_priority() {
        let mut queue = PrintQueue::new();
        queue.enqueue(make_job("1", Priority::Low, "Low"));
        queue.set_priority("1", Priority::High);
        assert_eq!(queue.get("1").unwrap().priority, Priority::High);
        // Should now be first.
        assert_eq!(queue.next().unwrap().id, "1");
    }

    #[test]
    fn test_cancel_queued_job() {
        let mut queue = PrintQueue::new();
        queue.enqueue(make_job("1", Priority::Normal, "Job 1"));
        assert!(queue.cancel("1"));
        assert!(queue.get("1").is_none());
    }

    #[test]
    fn test_cancel_printing_job() {
        let mut queue = PrintQueue::new();
        queue.enqueue(make_job("1", Priority::Normal, "Job 1"));
        queue.start_next();
        assert!(queue.cancel("1"));
        assert!(queue.current_job().is_none());
    }

    #[test]
    fn test_remove_job() {
        let mut queue = PrintQueue::new();
        queue.enqueue(make_job("1", Priority::Normal, "Job 1"));
        queue.enqueue(make_job("2", Priority::Normal, "Job 2"));
        assert!(queue.remove("1").is_some());
        assert_eq!(queue.total_count(), 1);
    }

    #[test]
    fn test_move_to_position() {
        let mut queue = PrintQueue::new();
        queue.enqueue(make_job("a", Priority::Normal, "A"));
        queue.enqueue(make_job("b", Priority::Normal, "B"));
        queue.enqueue(make_job("c", Priority::Normal, "C"));

        queue.move_to_position("c", 0);
        let jobs = queue.all_jobs();
        assert_eq!(jobs[0].id, "c");
        assert_eq!(jobs[1].id, "a");
        assert_eq!(jobs[2].id, "b");
    }

    #[test]
    fn test_clear_history() {
        let mut queue = PrintQueue::new();
        queue.enqueue(make_job("1", Priority::Normal, "Job 1"));
        queue.enqueue(make_job("2", Priority::Normal, "Job 2"));
        queue.start_next();
        queue.complete_current();
        queue.clear_history();
        assert_eq!(queue.total_count(), 1); // Job 2 still queued
    }

    #[test]
    fn test_job_status_display() {
        assert_eq!(format!("{}", JobStatus::Queued), "queued");
        assert_eq!(format!("{}", JobStatus::Completed), "completed");
        assert_eq!(
            format!("{}", JobStatus::Failed("oops".to_string())),
            "failed: oops"
        );
    }

    #[test]
    fn test_job_can_start() {
        let queued = PrintJob::new("1".into(), "Test".into(), "".into(), "Mat".into(), 10.0);
        assert!(queued.can_start());
        let printing = queued.with_status(JobStatus::Printing);
        assert!(!printing.can_start());
    }

    #[test]
    fn test_trims_history() {
        let mut queue = PrintQueue::with_max_history(2);
        for i in 0..5 {
            queue.enqueue(make_job(
                &format!("{}", i),
                Priority::Normal,
                &format!("Job {}", i),
            ));
        }
        // Complete 3 jobs.
        queue.start_next();
        queue.complete_current();
        queue.start_next();
        queue.complete_current();
        queue.start_next();
        queue.complete_current();

        // History should be trimmed to 2 completed + 2 remaining = 4 total.
        assert!(queue.total_count() <= 4);
    }
}
