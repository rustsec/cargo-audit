//! Vulnerability report generator
//!
//! These types map directly to the JSON report generated by `cargo-audit`,
//! but also provide the core reporting functionality used in general.

use crate::{
    advisory,
    database::{Database, Query},
    lockfile::Lockfile,
    package,
    platforms::target::{Arch, OS},
    vulnerability::Vulnerability,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Vulnerability report for a given lockfile
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Report {
    /// Information about the advisory database
    pub database: DatabaseInfo,

    /// Information about the audited lockfile
    pub lockfile: LockfileInfo,

    /// Settings used when generating report
    pub settings: Settings,

    /// Vulnerabilities detected in project
    pub vulnerabilities: VulnerabilityInfo,

    /// Warnings about dependencies (from e.g. informational advisories)
    pub warnings: Vec<Warning>,
}

impl Report {
    /// Generate a report for the given advisory database and lockfile
    pub fn generate(db: &Database, lockfile: &Lockfile, settings: &Settings) -> Self {
        let vulnerabilities = lockfile
            .query_vulnerabilities(db, &settings.query())
            .into_iter()
            .filter(|vuln| !settings.ignore.contains(&vuln.advisory.metadata.id))
            .collect();

        Self {
            database: DatabaseInfo::new(db),
            lockfile: LockfileInfo::new(lockfile),
            settings: settings.clone(),
            vulnerabilities: VulnerabilityInfo::new(vulnerabilities),
            warnings: Warning::generate(db, lockfile, settings),
        }
    }
}

/// Options to use when generating the report
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Settings {
    /// CPU architecture
    pub target_arch: Option<Arch>,

    /// Operating system
    pub target_os: Option<OS>,

    /// Severity threshold to alert at
    pub severity: Option<advisory::Severity>,

    /// List of advisory IDs to ignore
    pub ignore: Vec<advisory::Id>,

    /// Types of informational advisories to generate warnings for
    pub informational_warnings: Vec<advisory::Informational>,
}

impl Settings {
    /// Get a query which corresponds to the configured report settings.
    /// Note that queries can't filter ignored advisories, so this happens in
    /// a separate pass
    pub fn query(&self) -> Query {
        let mut query = Query::crate_scope();

        if let Some(target_arch) = self.target_arch {
            query = query.target_arch(target_arch);
        }

        if let Some(target_os) = self.target_os {
            query = query.target_os(target_os);
        }

        if let Some(severity) = self.severity {
            query = query.severity(severity);
        }

        query
    }
}

/// Information about the advisory database
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DatabaseInfo {
    /// Number of advisories in the database
    #[serde(rename = "advisory-count")]
    pub advisory_count: usize,

    /// Git commit hash for the last commit to the database
    #[serde(rename = "last-commit")]
    pub last_commit: String,

    /// Date when the advisory database was last committed to
    #[serde(rename = "last-updated")]
    pub last_updated: DateTime<Utc>,
}

impl DatabaseInfo {
    /// Create database information from the advisory db
    pub fn new(db: &Database) -> Self {
        Self {
            advisory_count: db.iter().count(),
            last_commit: db.latest_commit().commit_id.clone(),
            last_updated: db.latest_commit().time,
        }
    }
}

/// Information about `Cargo.lock`
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LockfileInfo {
    /// Number of dependencies in the lock file
    #[serde(rename = "dependency-count")]
    dependency_count: usize,
}

impl LockfileInfo {
    /// Create lockfile information from the given lockfile
    pub fn new(lockfile: &Lockfile) -> Self {
        Self {
            dependency_count: lockfile.packages.len(),
        }
    }
}

/// Information about detected vulnerabilities
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct VulnerabilityInfo {
    /// Were any vulnerabilities found?
    pub found: bool,

    /// Number of vulnerabilities found
    pub count: usize,

    /// List of detected vulnerabilities
    pub list: Vec<Vulnerability>,
}

impl VulnerabilityInfo {
    /// Create new vulnerability info
    pub fn new(list: Vec<Vulnerability>) -> Self {
        Self {
            found: !list.is_empty(),
            count: list.len(),
            list,
        }
    }
}

/// Warnings about particular dependencies
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Warning {
    /// Name of the dependent package
    pub package: package::Name,

    /// Text of the warning
    pub message: String,

    /// URL with additional information (if available)
    pub url: Option<String>,
}

impl Warning {
    /// Generate a report for the given advisory database and lockfile
    pub fn generate(db: &Database, lockfile: &Lockfile, settings: &Settings) -> Vec<Self> {
        let query = settings.query().informational(true);
        let mut result = vec![];

        for vuln in lockfile.query_vulnerabilities(db, &query) {
            let advisory = &vuln.advisory;

            if settings.ignore.contains(&advisory.metadata.id) {
                continue;
            }

            if settings
                .informational_warnings
                .iter()
                .any(|info| Some(info) == advisory.metadata.informational.as_ref())
            {
                result.push(Self {
                    package: advisory.metadata.package.clone(),
                    message: advisory.metadata.title.clone(),
                    url: advisory.metadata.id.url(),
                })
            }
        }

        result
    }
}
