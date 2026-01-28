//! Credential filtering
//!
//! This module provides filters for excluding credentials based on various criteria.

use crate::models::WifiCredential;

/// Result of applying a filter to a credential
#[derive(Debug, Clone)]
pub enum FilterResult {
    /// Credential passed the filter
    Pass,
    /// Credential was excluded with a reason
    Exclude { reason: String },
}

impl FilterResult {
    /// Returns true if the credential passed the filter
    pub fn passed(&self) -> bool {
        matches!(self, Self::Pass)
    }

    /// Returns the exclusion reason if excluded
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Pass => None,
            Self::Exclude { reason } => Some(reason),
        }
    }
}

/// Trait for credential filters
pub trait CredentialFilter: Send + Sync {
    /// Apply the filter to a credential
    fn filter(&self, credential: &WifiCredential) -> FilterResult;

    /// Get the name of this filter
    fn name(&self) -> &str;
}

/// Filter that excludes enterprise (802.1X) networks
#[derive(Debug, Default)]
pub struct EnterpriseFilter;

impl CredentialFilter for EnterpriseFilter {
    fn filter(&self, credential: &WifiCredential) -> FilterResult {
        if credential.security_type.is_enterprise() {
            FilterResult::Exclude {
                reason: "Enterprise (802.1X) networks cannot be synced".to_string(),
            }
        } else {
            FilterResult::Pass
        }
    }

    fn name(&self) -> &str {
        "enterprise"
    }
}

/// Filter that excludes open (no password) networks
#[derive(Debug, Default)]
pub struct OpenNetworkFilter;

impl CredentialFilter for OpenNetworkFilter {
    fn filter(&self, credential: &WifiCredential) -> FilterResult {
        if credential.security_type.is_open() {
            FilterResult::Exclude {
                reason: "Open networks have no credentials to sync".to_string(),
            }
        } else {
            FilterResult::Pass
        }
    }

    fn name(&self) -> &str {
        "open"
    }
}

/// Filter that excludes networks in a user-defined exclusion list
#[derive(Debug)]
pub struct ExclusionListFilter {
    /// List of excluded SSIDs (exact match)
    excluded_ssids: Vec<String>,
    /// List of excluded patterns (glob-style)
    excluded_patterns: Vec<glob::Pattern>,
}

impl ExclusionListFilter {
    /// Create a new exclusion list filter
    pub fn new() -> Self {
        Self {
            excluded_ssids: Vec::new(),
            excluded_patterns: Vec::new(),
        }
    }

    /// Create from a list of exclusion strings
    ///
    /// Strings containing `*` or `?` are treated as glob patterns.
    pub fn from_list(exclusions: &[String]) -> Self {
        let mut filter = Self::new();
        for exclusion in exclusions {
            filter.add_exclusion(exclusion);
        }
        filter
    }

    /// Add an exclusion (SSID or pattern)
    pub fn add_exclusion(&mut self, exclusion: &str) {
        if exclusion.contains('*') || exclusion.contains('?') {
            if let Ok(pattern) = glob::Pattern::new(exclusion) {
                self.excluded_patterns.push(pattern);
            }
        } else {
            self.excluded_ssids.push(exclusion.to_string());
        }
    }

    /// Remove an exclusion
    pub fn remove_exclusion(&mut self, exclusion: &str) -> bool {
        // Try to remove from exact matches
        if let Some(pos) = self.excluded_ssids.iter().position(|s| s == exclusion) {
            self.excluded_ssids.remove(pos);
            return true;
        }

        // Try to remove from patterns
        if let Some(pos) = self
            .excluded_patterns
            .iter()
            .position(|p| p.as_str() == exclusion)
        {
            self.excluded_patterns.remove(pos);
            return true;
        }

        false
    }

    /// Get all exclusions as strings
    pub fn exclusions(&self) -> Vec<String> {
        let mut result: Vec<String> = self.excluded_ssids.clone();
        result.extend(self.excluded_patterns.iter().map(|p| p.as_str().to_string()));
        result
    }

    /// Check if an SSID matches any exclusion
    fn matches(&self, ssid: &str) -> bool {
        // Check exact matches
        if self.excluded_ssids.iter().any(|s| s == ssid) {
            return true;
        }

        // Check patterns
        self.excluded_patterns.iter().any(|p| p.matches(ssid))
    }
}

impl Default for ExclusionListFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialFilter for ExclusionListFilter {
    fn filter(&self, credential: &WifiCredential) -> FilterResult {
        if self.matches(&credential.ssid) {
            FilterResult::Exclude {
                reason: format!("Network '{}' is in exclusion list", credential.ssid),
            }
        } else {
            FilterResult::Pass
        }
    }

    fn name(&self) -> &str {
        "exclusion_list"
    }
}

/// Filter that includes only credentials with specific tags
#[derive(Debug)]
pub struct TagFilter {
    /// Required tags (credential must have at least one)
    tags: Vec<String>,
    /// Match mode
    mode: TagFilterMode,
}

/// How to match tags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagFilterMode {
    /// Credential must have ANY of the specified tags
    Any,
    /// Credential must have ALL of the specified tags
    All,
}

impl TagFilter {
    /// Create a filter requiring any of the specified tags
    pub fn any(tags: &[impl AsRef<str>]) -> Self {
        Self {
            tags: tags.iter().map(|t| t.as_ref().to_string()).collect(),
            mode: TagFilterMode::Any,
        }
    }

    /// Create a filter requiring all of the specified tags
    pub fn all(tags: &[impl AsRef<str>]) -> Self {
        Self {
            tags: tags.iter().map(|t| t.as_ref().to_string()).collect(),
            mode: TagFilterMode::All,
        }
    }
}

impl CredentialFilter for TagFilter {
    fn filter(&self, credential: &WifiCredential) -> FilterResult {
        if self.tags.is_empty() {
            return FilterResult::Pass;
        }

        let matches = match self.mode {
            TagFilterMode::Any => self.tags.iter().any(|t| credential.has_tag(t)),
            TagFilterMode::All => self.tags.iter().all(|t| credential.has_tag(t)),
        };

        if matches {
            FilterResult::Pass
        } else {
            FilterResult::Exclude {
                reason: format!(
                    "Network '{}' doesn't have required tags: {:?}",
                    credential.ssid, self.tags
                ),
            }
        }
    }

    fn name(&self) -> &str {
        "tag"
    }
}

/// Statistics from a filter operation
#[derive(Debug, Default, Clone)]
pub struct FilterStats {
    /// Total credentials processed
    pub total: usize,
    /// Credentials that passed all filters
    pub passed: usize,
    /// Exclusions by filter name
    pub exclusions: std::collections::HashMap<String, Vec<ExcludedCredential>>,
}

/// Information about an excluded credential
#[derive(Debug, Clone)]
pub struct ExcludedCredential {
    /// SSID of the excluded network
    pub ssid: String,
    /// Reason for exclusion
    pub reason: String,
}

impl FilterStats {
    /// Get the number of excluded credentials
    pub fn excluded(&self) -> usize {
        self.total - self.passed
    }
}

/// A pipeline of filters applied in sequence
#[derive(Default)]
pub struct FilterPipeline {
    filters: Vec<Box<dyn CredentialFilter>>,
}

impl FilterPipeline {
    /// Create a new empty filter pipeline
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a pipeline with the default filters (enterprise + open)
    pub fn default_filters() -> Self {
        Self::new()
            .add(EnterpriseFilter)
            .add(OpenNetworkFilter)
    }

    /// Add a filter to the pipeline
    pub fn add<F: CredentialFilter + 'static>(mut self, filter: F) -> Self {
        self.filters.push(Box::new(filter));
        self
    }

    /// Apply all filters to a list of credentials
    ///
    /// Returns the filtered credentials and statistics
    pub fn apply(&self, credentials: &[WifiCredential]) -> (Vec<WifiCredential>, FilterStats) {
        let mut stats = FilterStats {
            total: credentials.len(),
            ..Default::default()
        };

        let filtered: Vec<WifiCredential> = credentials
            .iter()
            .filter(|cred| {
                for filter in &self.filters {
                    match filter.filter(cred) {
                        FilterResult::Pass => continue,
                        FilterResult::Exclude { reason } => {
                            stats
                                .exclusions
                                .entry(filter.name().to_string())
                                .or_default()
                                .push(ExcludedCredential {
                                    ssid: cred.ssid.clone(),
                                    reason,
                                });
                            return false;
                        }
                    }
                }
                true
            })
            .cloned()
            .collect();

        stats.passed = filtered.len();
        (filtered, stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{SecurityType, SourcePlatform};

    fn make_credential(ssid: &str, security: SecurityType) -> WifiCredential {
        WifiCredential::new(ssid, "password", security, SourcePlatform::Manual)
    }

    #[test]
    fn test_enterprise_filter() {
        let filter = EnterpriseFilter;

        let psk = make_credential("Home", SecurityType::Wpa2Psk);
        let enterprise = make_credential("Corp", SecurityType::Wpa2Enterprise);

        assert!(filter.filter(&psk).passed());
        assert!(!filter.filter(&enterprise).passed());
    }

    #[test]
    fn test_open_filter() {
        let filter = OpenNetworkFilter;

        let psk = make_credential("Home", SecurityType::Wpa2Psk);
        let open = make_credential("FreeWifi", SecurityType::Open);

        assert!(filter.filter(&psk).passed());
        assert!(!filter.filter(&open).passed());
    }

    #[test]
    fn test_exclusion_list_filter() {
        let mut filter = ExclusionListFilter::new();
        filter.add_exclusion("HomeNetwork");
        filter.add_exclusion("HomeNetwork-*");

        let home = make_credential("HomeNetwork", SecurityType::Wpa2Psk);
        let home_5g = make_credential("HomeNetwork-5G", SecurityType::Wpa2Psk);
        let coffee = make_credential("CoffeeShop", SecurityType::Wpa2Psk);

        assert!(!filter.filter(&home).passed());
        assert!(!filter.filter(&home_5g).passed());
        assert!(filter.filter(&coffee).passed());
    }

    #[test]
    fn test_tag_filter() {
        let mut cred = make_credential("CoffeeShop", SecurityType::Wpa2Psk);
        cred.add_tag("coffee");
        cred.add_tag("favorite");

        let filter_any = TagFilter::any(&["coffee", "work"]);
        let filter_all = TagFilter::all(&["coffee", "work"]);

        assert!(filter_any.filter(&cred).passed());
        assert!(!filter_all.filter(&cred).passed());
    }

    #[test]
    fn test_filter_pipeline() {
        let credentials = vec![
            make_credential("Home", SecurityType::Wpa2Psk),
            make_credential("Corp", SecurityType::Wpa2Enterprise),
            make_credential("FreeWifi", SecurityType::Open),
            make_credential("CoffeeShop", SecurityType::Wpa2Psk),
        ];

        let pipeline = FilterPipeline::default_filters();
        let (filtered, stats) = pipeline.apply(&credentials);

        assert_eq!(filtered.len(), 2);
        assert_eq!(stats.total, 4);
        assert_eq!(stats.passed, 2);
        assert_eq!(stats.excluded(), 2);
    }
}
