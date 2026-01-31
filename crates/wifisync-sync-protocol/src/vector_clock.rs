//! Vector clock implementation for conflict detection
//!
//! Vector clocks allow us to detect concurrent modifications across devices
//! without requiring a centralized clock.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A vector clock for tracking causality across devices
///
/// Each entry maps a device ID to its logical clock value.
/// When comparing clocks:
/// - A > B if A dominates B (all entries in A >= B, at least one >)
/// - A < B if B dominates A
/// - A || B (concurrent) if neither dominates the other
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorClock {
    /// Map of device_id -> logical timestamp
    clocks: BTreeMap<String, u64>,
}

/// Result of comparing two vector clocks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockOrdering {
    /// First clock is strictly before (happens-before) second
    Before,
    /// First clock is strictly after (happens-after) second
    After,
    /// Clocks are identical
    Equal,
    /// Clocks are concurrent (conflict)
    Concurrent,
}

impl VectorClock {
    /// Create a new empty vector clock
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a vector clock from a map
    #[must_use]
    pub fn from_map(clocks: BTreeMap<String, u64>) -> Self {
        Self { clocks }
    }

    /// Increment the clock for a specific device
    pub fn increment(&mut self, device_id: &str) {
        let counter = self.clocks.entry(device_id.to_string()).or_insert(0);
        *counter += 1;
    }

    /// Get the clock value for a specific device
    #[must_use]
    pub fn get(&self, device_id: &str) -> u64 {
        self.clocks.get(device_id).copied().unwrap_or(0)
    }

    /// Check if this clock is empty (no events recorded)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.clocks.is_empty() || self.clocks.values().all(|&v| v == 0)
    }

    /// Merge another clock into this one (take maximum of each entry)
    pub fn merge(&mut self, other: &Self) {
        for (device_id, &value) in &other.clocks {
            let entry = self.clocks.entry(device_id.clone()).or_insert(0);
            *entry = (*entry).max(value);
        }
    }

    /// Create a merged clock without modifying either input
    #[must_use]
    pub fn merged(&self, other: &Self) -> Self {
        let mut result = self.clone();
        result.merge(other);
        result
    }

    /// Compare this clock with another
    ///
    /// Returns the ordering relationship between the clocks.
    #[must_use]
    pub fn compare(&self, other: &Self) -> ClockOrdering {
        let mut self_greater = false;
        let mut other_greater = false;

        // Collect all device IDs from both clocks
        let all_devices: std::collections::BTreeSet<&String> = self
            .clocks
            .keys()
            .chain(other.clocks.keys())
            .collect();

        for device_id in all_devices {
            let self_val = self.get(device_id);
            let other_val = other.get(device_id);

            match self_val.cmp(&other_val) {
                std::cmp::Ordering::Greater => self_greater = true,
                std::cmp::Ordering::Less => other_greater = true,
                std::cmp::Ordering::Equal => {}
            }
        }

        match (self_greater, other_greater) {
            (false, false) => ClockOrdering::Equal,
            (true, false) => ClockOrdering::After,
            (false, true) => ClockOrdering::Before,
            (true, true) => ClockOrdering::Concurrent,
        }
    }

    /// Check if this clock happens-before another
    #[must_use]
    pub fn happens_before(&self, other: &Self) -> bool {
        self.compare(other) == ClockOrdering::Before
    }

    /// Check if this clock happens-after another
    #[must_use]
    pub fn happens_after(&self, other: &Self) -> bool {
        self.compare(other) == ClockOrdering::After
    }

    /// Check if this clock is concurrent with another (conflict)
    #[must_use]
    pub fn is_concurrent_with(&self, other: &Self) -> bool {
        self.compare(other) == ClockOrdering::Concurrent
    }

    /// Get the internal map (for serialization)
    #[must_use]
    pub fn as_map(&self) -> &BTreeMap<String, u64> {
        &self.clocks
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.clocks)
    }

    /// Deserialize from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let clocks: BTreeMap<String, u64> = serde_json::from_str(json)?;
        Ok(Self { clocks })
    }
}

impl std::fmt::Display for VectorClock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let entries: Vec<String> = self
            .clocks
            .iter()
            .map(|(k, v)| format!("{}:{}", k, v))
            .collect();
        write!(f, "[{}]", entries.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_clock_is_empty() {
        let clock = VectorClock::new();
        assert!(clock.is_empty());
        assert_eq!(clock.get("device1"), 0);
    }

    #[test]
    fn test_increment() {
        let mut clock = VectorClock::new();
        clock.increment("device1");
        assert_eq!(clock.get("device1"), 1);
        clock.increment("device1");
        assert_eq!(clock.get("device1"), 2);
        assert_eq!(clock.get("device2"), 0);
    }

    #[test]
    fn test_compare_equal() {
        let mut clock1 = VectorClock::new();
        let mut clock2 = VectorClock::new();

        clock1.increment("a");
        clock2.increment("a");

        assert_eq!(clock1.compare(&clock2), ClockOrdering::Equal);
    }

    #[test]
    fn test_compare_before() {
        let mut clock1 = VectorClock::new();
        let mut clock2 = VectorClock::new();

        clock1.increment("a");
        clock2.increment("a");
        clock2.increment("a");

        assert_eq!(clock1.compare(&clock2), ClockOrdering::Before);
        assert!(clock1.happens_before(&clock2));
    }

    #[test]
    fn test_compare_after() {
        let mut clock1 = VectorClock::new();
        let mut clock2 = VectorClock::new();

        clock1.increment("a");
        clock1.increment("a");
        clock2.increment("a");

        assert_eq!(clock1.compare(&clock2), ClockOrdering::After);
        assert!(clock1.happens_after(&clock2));
    }

    #[test]
    fn test_compare_concurrent() {
        let mut clock1 = VectorClock::new();
        let mut clock2 = VectorClock::new();

        clock1.increment("a");
        clock2.increment("b");

        assert_eq!(clock1.compare(&clock2), ClockOrdering::Concurrent);
        assert!(clock1.is_concurrent_with(&clock2));
    }

    #[test]
    fn test_merge() {
        let mut clock1 = VectorClock::new();
        let mut clock2 = VectorClock::new();

        clock1.increment("a");
        clock1.increment("a");
        clock2.increment("b");
        clock2.increment("b");
        clock2.increment("b");

        clock1.merge(&clock2);

        assert_eq!(clock1.get("a"), 2);
        assert_eq!(clock1.get("b"), 3);
    }

    #[test]
    fn test_merged_creates_new() {
        let mut clock1 = VectorClock::new();
        let mut clock2 = VectorClock::new();

        clock1.increment("a");
        clock2.increment("b");

        let merged = clock1.merged(&clock2);

        // Original clocks unchanged
        assert_eq!(clock1.get("b"), 0);
        assert_eq!(clock2.get("a"), 0);

        // Merged clock has both
        assert_eq!(merged.get("a"), 1);
        assert_eq!(merged.get("b"), 1);
    }

    #[test]
    fn test_serialization() {
        let mut clock = VectorClock::new();
        clock.increment("device1");
        clock.increment("device2");
        clock.increment("device1");

        let json = clock.to_json().unwrap();
        let restored = VectorClock::from_json(&json).unwrap();

        assert_eq!(clock, restored);
    }

    #[test]
    fn test_display() {
        let mut clock = VectorClock::new();
        clock.increment("a");
        clock.increment("b");
        clock.increment("b");

        let display = format!("{}", clock);
        assert!(display.contains("a:1"));
        assert!(display.contains("b:2"));
    }
}
