/// Convergence tracking for batch COBOL program output verification.
///
/// Detects when a running batch processor has "converged" — i.e., the output
/// accuracy has plateaued and further processing will not improve results.
///
/// Algorithm:
/// 1. Track accuracy (match_rate) in windows of `window_size` batches
/// 2. A plateau is detected when the change in accuracy over the window
///    is less than `threshold`
/// 3. Mark as converged once `min_windows` consecutive plateaus are seen

/// Tracks accuracy convergence over a sliding window of batch results.
#[derive(Debug)]
pub struct ConvergenceTracker {
    /// Number of records per batch
    pub batch_size: usize,
    /// Number of batches in the sliding window
    pub window: usize,
    /// Minimum change to NOT be considered converged
    pub threshold: f64,
    /// Accuracy measurements per batch
    history: Vec<f64>,
    /// Total records processed
    total_records: u64,
    /// Total matching records
    matching_records: u64,
}

impl ConvergenceTracker {
    /// Create a new tracker with the given parameters.
    ///
    /// # Parameters
    /// - `batch_size`: records per batch (e.g., 10_000)
    /// - `window`: number of recent batches to compare (e.g., 5)
    /// - `threshold`: minimum accuracy delta to avoid declaring convergence (e.g., 0.0001)
    pub fn new(batch_size: usize, window: usize, threshold: f64) -> Self {
        Self {
            batch_size,
            window,
            threshold,
            history: Vec::new(),
            total_records: 0,
            matching_records: 0,
        }
    }

    /// Record results for a completed batch.
    ///
    /// # Parameters
    /// - `batch_matches`: number of records in this batch that matched expected output
    /// - `batch_total`: total records in this batch (may differ from `batch_size` for last batch)
    pub fn record_batch(&mut self, batch_matches: u64, batch_total: u64) {
        self.total_records += batch_total;
        self.matching_records += batch_matches;

        let batch_accuracy = if batch_total > 0 {
            batch_matches as f64 / batch_total as f64
        } else {
            0.0
        };
        self.history.push(batch_accuracy);
    }

    /// Overall accuracy across all processed records.
    pub fn overall_accuracy(&self) -> f64 {
        if self.total_records == 0 {
            return 0.0;
        }
        self.matching_records as f64 / self.total_records as f64
    }

    /// Check if the accuracy has converged (plateaued within `threshold`).
    ///
    /// Returns `true` when the most recent `window` batches all have accuracy
    /// values that differ from the window average by less than `threshold`.
    pub fn has_converged(&self) -> bool {
        if self.history.len() < self.window {
            return false;
        }

        let recent = &self.history[self.history.len() - self.window..];
        let avg: f64 = recent.iter().sum::<f64>() / recent.len() as f64;

        // Converged if all values are within threshold of the window average
        recent.iter().all(|&v| (v - avg).abs() < self.threshold)
    }

    /// Returns the accuracy delta over the last `window` batches.
    ///
    /// A value near 0.0 indicates convergence.
    pub fn window_delta(&self) -> Option<f64> {
        if self.history.len() < self.window {
            return None;
        }
        let recent = &self.history[self.history.len() - self.window..];
        let max = recent.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let min = recent.iter().cloned().fold(f64::INFINITY, f64::min);
        Some(max - min)
    }

    /// Total number of records processed.
    pub fn total_records(&self) -> u64 {
        self.total_records
    }

    /// Number of batches completed.
    pub fn batches_completed(&self) -> usize {
        self.history.len()
    }

    /// Reset the tracker to initial state.
    pub fn reset(&mut self) {
        self.history.clear();
        self.total_records = 0;
        self.matching_records = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracker() -> ConvergenceTracker {
        ConvergenceTracker::new(10_000, 5, 0.0001)
    }

    #[test]
    fn test_default_parameters() {
        let t = tracker();
        assert_eq!(t.batch_size, 10_000);
        assert_eq!(t.window, 5);
        assert!((t.threshold - 0.0001).abs() < f64::EPSILON);
    }

    #[test]
    fn test_not_converged_initially() {
        let t = tracker();
        assert!(!t.has_converged(), "should not converge with 0 batches");
    }

    #[test]
    fn test_not_converged_before_window_full() {
        let mut t = tracker();
        for _ in 0..4 {
            t.record_batch(9_800, 10_000);
        }
        assert!(!t.has_converged(), "need at least window=5 batches");
    }

    #[test]
    fn test_converged_with_stable_accuracy() {
        let mut t = tracker();
        // Feed 6 batches with identical accuracy (98.0%)
        for _ in 0..6 {
            t.record_batch(9_800, 10_000);
        }
        assert!(
            t.has_converged(),
            "identical accuracy should converge: delta={:?}",
            t.window_delta()
        );
    }

    #[test]
    fn test_not_converged_with_improving_accuracy() {
        let mut t = tracker();
        // Feed batches with steadily improving accuracy (not plateaued)
        for i in 0..6u64 {
            let matches = 9_000 + i * 100; // 90.0%, 91.0%, ..., 95.0%
            t.record_batch(matches, 10_000);
        }
        assert!(
            !t.has_converged(),
            "improving accuracy should not converge yet"
        );
    }

    #[test]
    fn test_overall_accuracy() {
        let mut t = tracker();
        t.record_batch(8_000, 10_000); // 80%
        t.record_batch(9_000, 10_000); // 90%
                                       // Overall: 17000/20000 = 85%
        let acc = t.overall_accuracy();
        assert!(
            (acc - 0.85).abs() < 0.001,
            "overall accuracy should be 85%, got {}",
            acc
        );
    }

    #[test]
    fn test_window_delta_returns_none_before_window_full() {
        let mut t = tracker();
        t.record_batch(9_000, 10_000);
        assert!(t.window_delta().is_none());
    }

    #[test]
    fn test_window_delta_near_zero_when_stable() {
        let mut t = tracker();
        for _ in 0..5 {
            t.record_batch(9_750, 10_000);
        }
        let delta = t.window_delta().unwrap();
        assert!(
            delta < 0.0001,
            "delta should be ~0 with stable accuracy: {}",
            delta
        );
    }

    #[test]
    fn test_plateau_detection_30000_records() {
        // Simulate 30,000-record batch: 3 batches of 10,000 (exactly the window-1)
        // then 3 more identical batches → convergence
        let mut t = tracker();

        // Phase 1: improvement
        t.record_batch(7_000, 10_000); // 70%
        t.record_batch(8_500, 10_000); // 85%
        t.record_batch(9_200, 10_000); // 92%

        // Phase 2: plateau at ~97%
        for _ in 0..5 {
            t.record_batch(9_700, 10_000); // 97%
        }

        assert!(
            t.has_converged(),
            "should detect plateau after 5 stable batches"
        );
        assert!(
            t.total_records() >= 30_000,
            "should have processed at least 30,000 records, got {}",
            t.total_records()
        );
    }

    #[test]
    fn test_reset_clears_state() {
        let mut t = tracker();
        for _ in 0..6 {
            t.record_batch(9_800, 10_000);
        }
        assert!(t.has_converged());
        t.reset();
        assert!(!t.has_converged(), "after reset, should not be converged");
        assert_eq!(t.total_records(), 0);
        assert_eq!(t.batches_completed(), 0);
    }
}
