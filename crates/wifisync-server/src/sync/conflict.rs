//! Conflict detection for sync operations

use wifisync_sync_protocol::{ClockOrdering, VectorClock};

/// Result of checking a change against existing state
#[derive(Debug)]
pub enum ChangeCheckResult {
    /// Change can be applied (no conflict)
    Accept,
    /// Change is outdated (existing is newer)
    Outdated,
    /// Conflict detected (concurrent changes)
    Conflict,
}

/// Conflict detector for sync operations
pub struct ConflictDetector;

impl ConflictDetector {
    /// Check if a change can be applied to the current state
    ///
    /// # Arguments
    /// * `incoming` - The incoming change's vector clock
    /// * `existing` - The existing record's vector clock (None if no existing record)
    ///
    /// # Returns
    /// The check result indicating whether to accept, reject, or flag as conflict
    pub fn check_change(
        incoming: &VectorClock,
        existing: Option<&VectorClock>,
    ) -> ChangeCheckResult {
        match existing {
            None => {
                // No existing record, accept the change
                ChangeCheckResult::Accept
            }
            Some(existing_clock) => {
                match incoming.compare(existing_clock) {
                    ClockOrdering::After => {
                        // Incoming is strictly newer, accept
                        ChangeCheckResult::Accept
                    }
                    ClockOrdering::Before | ClockOrdering::Equal => {
                        // Incoming is older or same, outdated
                        ChangeCheckResult::Outdated
                    }
                    ClockOrdering::Concurrent => {
                        // Concurrent modification, conflict!
                        ChangeCheckResult::Conflict
                    }
                }
            }
        }
    }

    /// Check if two changes are in conflict
    pub fn are_concurrent(clock1: &VectorClock, clock2: &VectorClock) -> bool {
        clock1.is_concurrent_with(clock2)
    }

    /// Merge two clocks (for conflict resolution)
    pub fn merge_clocks(clock1: &VectorClock, clock2: &VectorClock) -> VectorClock {
        clock1.merged(clock2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accept_new_change() {
        let mut incoming = VectorClock::new();
        incoming.increment("device1");

        let result = ConflictDetector::check_change(&incoming, None);
        assert!(matches!(result, ChangeCheckResult::Accept));
    }

    #[test]
    fn test_accept_newer_change() {
        let mut existing = VectorClock::new();
        existing.increment("device1");

        let mut incoming = VectorClock::new();
        incoming.increment("device1");
        incoming.increment("device1");

        let result = ConflictDetector::check_change(&incoming, Some(&existing));
        assert!(matches!(result, ChangeCheckResult::Accept));
    }

    #[test]
    fn test_reject_outdated_change() {
        let mut existing = VectorClock::new();
        existing.increment("device1");
        existing.increment("device1");

        let mut incoming = VectorClock::new();
        incoming.increment("device1");

        let result = ConflictDetector::check_change(&incoming, Some(&existing));
        assert!(matches!(result, ChangeCheckResult::Outdated));
    }

    #[test]
    fn test_detect_concurrent_conflict() {
        let mut existing = VectorClock::new();
        existing.increment("device1");

        let mut incoming = VectorClock::new();
        incoming.increment("device2");

        let result = ConflictDetector::check_change(&incoming, Some(&existing));
        assert!(matches!(result, ChangeCheckResult::Conflict));
    }
}
