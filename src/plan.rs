//! Removal plan — pure data describing *what* would be removed.
//!
//! Author: PratikP1
//!
//! The plan is built from static knowledge of where supported security suites
//! store their files, registry keys, services, and scheduled tasks.  Nothing
//! is deleted here; the executor module consumes the plan.

use crate::product::Product;

// ─── Domain types ────────────────────────────────────────────────────────────

/// A Windows registry key (or value) that should be deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryEntry {
    /// E.g. `"HKLM\\SOFTWARE\\McAfee"`.
    pub key_path: String,
    /// `None` means delete the whole key; `Some(name)` deletes a single value.
    pub value_name: Option<String>,
}

impl RegistryEntry {
    /// Convenience constructor for deleting an entire key.
    pub fn key(path: impl Into<String>) -> Self {
        Self {
            key_path: path.into(),
            value_name: None,
        }
    }

    /// Convenience constructor for deleting a single named value.
    pub fn value(path: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            key_path: path.into(),
            value_name: Some(name.into()),
        }
    }
}

/// A filesystem path (file or directory) that should be deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePath {
    /// Absolute Windows path, e.g. `"C:\\Program Files\\McAfee"`.
    pub path: String,
    /// When `true` the entry is a directory; recursive deletion is used.
    pub is_dir: bool,
}

impl FilePath {
    pub fn file(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            is_dir: false,
        }
    }

    pub fn dir(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            is_dir: true,
        }
    }
}

/// A Windows service name that should be stopped and deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceEntry {
    pub name: String,
}

impl ServiceEntry {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// A Windows Scheduled Task path that should be deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledTask {
    /// Task path as shown in Task Scheduler, e.g. `"\\McAfee\\..."`.
    pub task_path: String,
}

impl ScheduledTask {
    pub fn new(task_path: impl Into<String>) -> Self {
        Self {
            task_path: task_path.into(),
        }
    }
}

// ─── Removal plan ────────────────────────────────────────────────────────────

/// Everything that will be removed for a given product.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovalPlan {
    pub product: Product,
    pub registry_entries: Vec<RegistryEntry>,
    pub file_paths: Vec<FilePath>,
    pub services: Vec<ServiceEntry>,
    pub scheduled_tasks: Vec<ScheduledTask>,
}

impl RemovalPlan {
    /// Build the complete removal plan for `product`.
    pub fn for_product(product: Product) -> Self {
        match product {
            Product::McAfee => mcafee_plan(),
            Product::Norton => norton_plan(),
            Product::Avast => avast_plan(),
            Product::Avg => avg_plan(),
        }
    }

    /// Total number of removal actions.
    pub fn action_count(&self) -> usize {
        self.registry_entries.len()
            + self.file_paths.len()
            + self.services.len()
            + self.scheduled_tasks.len()
    }

    /// `true` when the plan has at least one action.
    pub fn is_non_empty(&self) -> bool {
        self.action_count() > 0
    }
}

// ─── McAfee knowledge base ───────────────────────────────────────────────────

fn mcafee_plan() -> RemovalPlan {
    RemovalPlan {
        product: Product::McAfee,
        registry_entries: vec![
            RegistryEntry::key(r"HKLM\SOFTWARE\McAfee"),
            RegistryEntry::key(r"HKLM\SOFTWARE\WOW6432Node\McAfee"),
            RegistryEntry::key(r"HKLM\SYSTEM\CurrentControlSet\Services\McShield"),
            RegistryEntry::key(r"HKLM\SYSTEM\CurrentControlSet\Services\McAfee WebAdvisor"),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\McAfee Total Protection",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\McAfee LiveSafe",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\McAfee WebAdvisor",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\McAfee WebAdvisor",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\McAfee SiteAdvisor",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\McAfee SiteAdvisor",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\McAfee.WPS",
            ),
            RegistryEntry::key(r"HKCU\SOFTWARE\McAfee"),
            RegistryEntry::key(r"HKLM\SOFTWARE\McAfee.com"),
        ],
        file_paths: vec![
            FilePath::dir(r"C:\Program Files\McAfee"),
            FilePath::dir(r"C:\Program Files (x86)\McAfee"),
            FilePath::dir(r"C:\Program Files\McAfee.com"),
            FilePath::dir(r"C:\Program Files (x86)\McAfee.com"),
            FilePath::dir(r"C:\Program Files\McAfee\WebAdvisor"),
            FilePath::dir(r"C:\Program Files (x86)\McAfee\WebAdvisor"),
            FilePath::dir(r"C:\Program Files (x86)\McAfee\SiteAdvisor"),
            FilePath::dir(r"C:\ProgramData\McAfee"),
            FilePath::dir(r"C:\Users\All Users\McAfee"),
            FilePath::dir(r"C:\Program Files\Common Files\McAfee"),
            FilePath::dir(r"C:\Program Files (x86)\Common Files\McAfee"),
            FilePath::file(r"C:\Windows\System32\drivers\mfehidk.sys"),
            FilePath::file(r"C:\Windows\System32\drivers\mfefirek.sys"),
            FilePath::file(r"C:\Windows\System32\drivers\mfewfpk.sys"),
        ],
        services: vec![
            ServiceEntry::new("McShield"),
            ServiceEntry::new("McAfeeEngineService"),
            ServiceEntry::new("McAfee WebAdvisor"),
            ServiceEntry::new("mfemms"),
            ServiceEntry::new("mfefire"),
        ],
        scheduled_tasks: vec![
            ScheduledTask::new(r"\McAfee\McAfee Auto Maintenance"),
            ScheduledTask::new(r"\McAfee\McAfeeLogon"),
            ScheduledTask::new(r"\McAfee\McAfee WebAdvisor"),
        ],
    }
}

// ─── Norton knowledge base ───────────────────────────────────────────────────

fn norton_plan() -> RemovalPlan {
    RemovalPlan {
        product: Product::Norton,
        registry_entries: vec![
            RegistryEntry::key(r"HKLM\SOFTWARE\Norton"),
            RegistryEntry::key(r"HKLM\SOFTWARE\WOW6432Node\Norton"),
            RegistryEntry::key(r"HKLM\SOFTWARE\Symantec"),
            RegistryEntry::key(r"HKLM\SOFTWARE\WOW6432Node\Symantec"),
            RegistryEntry::key(r"HKLM\SYSTEM\CurrentControlSet\Services\NortonSecurity"),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Norton 360",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Norton Secure VPN",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Norton Secure VPN",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Norton Utilities",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Norton Utilities",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Norton Utilities Ultimate",
            ),
            RegistryEntry::key(r"HKCU\SOFTWARE\Norton"),
            RegistryEntry::key(r"HKCU\SOFTWARE\Symantec"),
        ],
        file_paths: vec![
            FilePath::dir(r"C:\Program Files\Norton Security"),
            FilePath::dir(r"C:\Program Files\Norton 360"),
            FilePath::dir(r"C:\Program Files\Norton"),
            FilePath::dir(r"C:\Program Files\Norton Secure VPN"),
            FilePath::dir(r"C:\Program Files\Norton Utilities"),
            FilePath::dir(r"C:\Program Files (x86)\Norton Security"),
            FilePath::dir(r"C:\Program Files (x86)\Norton 360"),
            FilePath::dir(r"C:\Program Files (x86)\Norton"),
            FilePath::dir(r"C:\Program Files (x86)\Norton Secure VPN"),
            FilePath::dir(r"C:\Program Files (x86)\Norton Utilities"),
            FilePath::dir(r"C:\ProgramData\Norton"),
            FilePath::dir(r"C:\ProgramData\Symantec"),
            FilePath::dir(r"C:\Program Files\Common Files\Symantec Shared"),
            FilePath::dir(r"C:\Program Files (x86)\Common Files\Symantec Shared"),
            FilePath::file(r"C:\Windows\System32\drivers\NortonSecurity.sys"),
        ],
        services: vec![
            ServiceEntry::new("NortonSecurity"),
            ServiceEntry::new("NortonSecurityPlatformIDS"),
            ServiceEntry::new("Symantec Event Manager"),
            ServiceEntry::new("Symantec Settings Manager"),
        ],
        scheduled_tasks: vec![
            ScheduledTask::new(r"\Norton Security\Norton Error Processor"),
            ScheduledTask::new(r"\Norton Security\Norton Error Submitter"),
            ScheduledTask::new(r"\Symantec\Norton Update Manager"),
        ],
    }
}

// ─── Avast knowledge base ────────────────────────────────────────────────────

fn avast_plan() -> RemovalPlan {
    RemovalPlan {
        product: Product::Avast,
        registry_entries: vec![
            RegistryEntry::key(r"HKLM\SOFTWARE\AVAST Software"),
            RegistryEntry::key(r"HKLM\SOFTWARE\WOW6432Node\AVAST Software"),
            RegistryEntry::key(r"HKLM\SOFTWARE\AVAST Software\Avast"),
            RegistryEntry::key(r"HKLM\SYSTEM\CurrentControlSet\Services\AvastSvc"),
            RegistryEntry::key(r"HKLM\SYSTEM\CurrentControlSet\Services\AvastWscReporter"),
            RegistryEntry::key(r"HKLM\SYSTEM\CurrentControlSet\Services\aswMonFlt"),
            RegistryEntry::key(r"HKLM\SYSTEM\CurrentControlSet\Services\aswSnx"),
            RegistryEntry::key(r"HKLM\SYSTEM\CurrentControlSet\Services\aswSP"),
            RegistryEntry::key(r"HKLM\SYSTEM\CurrentControlSet\Services\aswVmm"),
            RegistryEntry::key(r"HKLM\SYSTEM\CurrentControlSet\Services\aswRdr2"),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Avast Antivirus",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Avast Secure Browser",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Avast Secure Browser",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Avast Cleanup",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Avast Cleanup Premium",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Avast Driver Updater",
            ),
            RegistryEntry::key(r"HKCU\SOFTWARE\AVAST Software"),
        ],
        file_paths: vec![
            FilePath::dir(r"C:\Program Files\AVAST Software"),
            FilePath::dir(r"C:\Program Files\AVAST Software\Avast"),
            FilePath::dir(r"C:\Program Files (x86)\AVAST Software"),
            FilePath::dir(r"C:\Program Files (x86)\AVAST Software\Avast"),
            FilePath::dir(r"C:\Program Files (x86)\AVAST Software\Browser"),
            FilePath::dir(r"C:\Program Files\Common Files\Avast Software"),
            FilePath::dir(r"C:\ProgramData\AVAST Software"),
            FilePath::dir(r"C:\ProgramData\AVAST Software\Avast\log"),
            FilePath::dir(r"C:\ProgramData\AVAST Software\Avast\setup"),
            FilePath::file(r"C:\Windows\System32\drivers\aswSP.sys"),
            FilePath::file(r"C:\Windows\System32\drivers\aswSnx.sys"),
            FilePath::file(r"C:\Windows\System32\drivers\aswMonFlt.sys"),
            FilePath::file(r"C:\Windows\System32\drivers\aswVmm.sys"),
            FilePath::file(r"C:\Windows\System32\drivers\aswRdr2.sys"),
        ],
        services: vec![
            ServiceEntry::new("AvastSvc"),
            ServiceEntry::new("AvastWscReporter"),
            ServiceEntry::new("aswbIDSAgent"),
            ServiceEntry::new("aswMonFlt"),
            ServiceEntry::new("aswSnx"),
            ServiceEntry::new("aswSP"),
            ServiceEntry::new("aswVmm"),
            ServiceEntry::new("aswRdr2"),
        ],
        scheduled_tasks: vec![
            ScheduledTask::new(r"\AVAST Software\Avast\Overseer"),
            ScheduledTask::new(r"\AVAST Software\Avast\AutoRepair"),
            ScheduledTask::new(r"\AVAST Software\Avast\Periodic Scan"),
            ScheduledTask::new(r"\AVAST Software\Avast\AvastBrowserUpdate"),
        ],
    }
}

// ─── AVG knowledge base ──────────────────────────────────────────────────────

fn avg_plan() -> RemovalPlan {
    RemovalPlan {
        product: Product::Avg,
        registry_entries: vec![
            RegistryEntry::key(r"HKLM\SOFTWARE\AVG"),
            RegistryEntry::key(r"HKLM\SOFTWARE\WOW6432Node\AVG"),
            RegistryEntry::key(r"HKLM\SYSTEM\CurrentControlSet\Services\avgSvc"),
            RegistryEntry::key(r"HKLM\SYSTEM\CurrentControlSet\Services\avgMonFlt"),
            RegistryEntry::key(r"HKLM\SYSTEM\CurrentControlSet\Services\avgSP"),
            RegistryEntry::key(r"HKLM\SYSTEM\CurrentControlSet\Services\avgRdr"),
            RegistryEntry::key(r"HKLM\SYSTEM\CurrentControlSet\Services\AVGIDSDriver"),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\AVG Antivirus",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\AVG Secure Browser",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\AVG Secure Browser",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\AVG PC TuneUp",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\AVG TuneUp",
            ),
            RegistryEntry::key(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\AVG Driver Updater",
            ),
            RegistryEntry::key(r"HKCU\SOFTWARE\AVG"),
        ],
        file_paths: vec![
            FilePath::dir(r"C:\Program Files\AVG"),
            FilePath::dir(r"C:\Program Files\AVG\Antivirus"),
            FilePath::dir(r"C:\Program Files (x86)\AVG"),
            FilePath::dir(r"C:\ProgramData\AVG"),
            FilePath::dir(r"C:\ProgramData\AVG\Antivirus"),
            FilePath::dir(r"C:\ProgramData\TuneUp Software"),
            FilePath::file(r"C:\Windows\System32\drivers\avgSP.sys"),
            FilePath::file(r"C:\Windows\System32\drivers\avgMonFlt.sys"),
            FilePath::file(r"C:\Windows\System32\drivers\avgRdr.sys"),
        ],
        services: vec![
            ServiceEntry::new("avgSvc"),
            ServiceEntry::new("avgMonFlt"),
            ServiceEntry::new("avgSP"),
            ServiceEntry::new("avgRdr"),
            ServiceEntry::new("AVGIDSDriver"),
        ],
        scheduled_tasks: vec![
            ScheduledTask::new(r"\AVG\Antivirus\Overseer"),
            ScheduledTask::new(r"\AVG\Antivirus\AutoRepair"),
            ScheduledTask::new(r"\AVG\Antivirus\Periodic Scan"),
        ],
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── RegistryEntry ────────────────────────────────────────────────────────

    #[test]
    fn registry_key_has_no_value_name() {
        let e = RegistryEntry::key(r"HKLM\SOFTWARE\McAfee");
        assert_eq!(e.key_path, r"HKLM\SOFTWARE\McAfee");
        assert!(e.value_name.is_none());
    }

    #[test]
    fn registry_value_stores_name() {
        let e = RegistryEntry::value(r"HKLM\SOFTWARE\McAfee", "Version");
        assert_eq!(e.value_name.as_deref(), Some("Version"));
    }

    // ── FilePath ─────────────────────────────────────────────────────────────

    #[test]
    fn file_path_is_not_dir() {
        let f = FilePath::file(r"C:\Windows\System32\mfehidk.sys");
        assert!(!f.is_dir);
    }

    #[test]
    fn dir_path_is_dir() {
        let d = FilePath::dir(r"C:\Program Files\McAfee");
        assert!(d.is_dir);
    }

    // ── RemovalPlan::action_count ────────────────────────────────────────────

    #[test]
    fn mcafee_plan_is_non_empty() {
        let plan = RemovalPlan::for_product(Product::McAfee);
        assert!(plan.is_non_empty());
    }

    #[test]
    fn norton_plan_is_non_empty() {
        let plan = RemovalPlan::for_product(Product::Norton);
        assert!(plan.is_non_empty());
    }

    #[test]
    fn avast_plan_is_non_empty() {
        let plan = RemovalPlan::for_product(Product::Avast);
        assert!(plan.is_non_empty());
    }

    #[test]
    fn avg_plan_is_non_empty() {
        let plan = RemovalPlan::for_product(Product::Avg);
        assert!(plan.is_non_empty());
    }

    #[test]
    fn mcafee_plan_has_registry_entries() {
        let plan = RemovalPlan::for_product(Product::McAfee);
        assert!(!plan.registry_entries.is_empty());
    }

    #[test]
    fn mcafee_plan_has_file_paths() {
        let plan = RemovalPlan::for_product(Product::McAfee);
        assert!(!plan.file_paths.is_empty());
    }

    #[test]
    fn mcafee_plan_has_services() {
        let plan = RemovalPlan::for_product(Product::McAfee);
        assert!(!plan.services.is_empty());
    }

    #[test]
    fn norton_plan_has_registry_entries() {
        let plan = RemovalPlan::for_product(Product::Norton);
        assert!(!plan.registry_entries.is_empty());
    }

    #[test]
    fn norton_plan_has_services() {
        let plan = RemovalPlan::for_product(Product::Norton);
        assert!(!plan.services.is_empty());
    }

    #[test]
    fn norton_plan_has_scheduled_tasks() {
        let plan = RemovalPlan::for_product(Product::Norton);
        assert!(!plan.scheduled_tasks.is_empty());
    }

    #[test]
    fn mcafee_plan_covers_livesafe_and_webadvisor() {
        let plan = RemovalPlan::for_product(Product::McAfee);
        assert!(plan.registry_entries.iter().any(|entry| {
            entry.key_path.contains("McAfee LiveSafe")
                || entry.key_path.contains("McAfee WebAdvisor")
        }));
        assert!(
            plan.file_paths
                .iter()
                .any(|entry| entry.path.contains(r"McAfee.com")
                    || entry.path.contains(r"SiteAdvisor"))
        );
    }

    #[test]
    fn norton_plan_covers_vpn_and_utilities() {
        let plan = RemovalPlan::for_product(Product::Norton);
        assert!(plan.registry_entries.iter().any(|entry| {
            entry.key_path.contains("Norton Secure VPN")
                || entry.key_path.contains("Norton Utilities")
        }));
        assert!(plan.file_paths.iter().any(|entry| {
            entry.path.contains(r"Norton Secure VPN") || entry.path.contains(r"Norton Utilities")
        }));
    }

    #[test]
    fn avast_plan_has_scheduled_tasks() {
        let plan = RemovalPlan::for_product(Product::Avast);
        assert!(!plan.scheduled_tasks.is_empty());
    }

    #[test]
    fn avast_plan_covers_browser_and_cleanup() {
        let plan = RemovalPlan::for_product(Product::Avast);
        assert!(plan.registry_entries.iter().any(|entry| {
            entry.key_path.contains("Avast Secure Browser")
                || entry.key_path.contains("Avast Cleanup")
        }));
        assert!(plan.file_paths.iter().any(|entry| {
            entry.path.contains(r"AVAST Software\Browser")
                || entry.path.contains(r"Common Files\Avast Software")
        }));
    }

    #[test]
    fn avg_plan_has_scheduled_tasks() {
        let plan = RemovalPlan::for_product(Product::Avg);
        assert!(!plan.scheduled_tasks.is_empty());
    }

    #[test]
    fn avg_plan_covers_browser_and_tuneup() {
        let plan = RemovalPlan::for_product(Product::Avg);
        assert!(plan.registry_entries.iter().any(|entry| {
            entry.key_path.contains("AVG Secure Browser")
                || entry.key_path.contains("AVG PC TuneUp")
                || entry.key_path.contains("AVG TuneUp")
        }));
        assert!(plan.file_paths.iter().any(|entry| {
            entry.path.contains(r"C:\Program Files\AVG") || entry.path.contains(r"TuneUp Software")
        }));
    }

    #[test]
    fn action_count_equals_sum_of_parts() {
        let plan = RemovalPlan::for_product(Product::McAfee);
        let expected = plan.registry_entries.len()
            + plan.file_paths.len()
            + plan.services.len()
            + plan.scheduled_tasks.len();
        assert_eq!(plan.action_count(), expected);
    }

    #[test]
    fn plan_product_field_matches_requested_product() {
        assert_eq!(
            RemovalPlan::for_product(Product::McAfee).product,
            Product::McAfee
        );
        assert_eq!(
            RemovalPlan::for_product(Product::Norton).product,
            Product::Norton
        );
        assert_eq!(
            RemovalPlan::for_product(Product::Avast).product,
            Product::Avast
        );
        assert_eq!(RemovalPlan::for_product(Product::Avg).product, Product::Avg);
    }

    #[test]
    fn mcafee_registry_entries_all_start_with_hk() {
        for entry in &RemovalPlan::for_product(Product::McAfee).registry_entries {
            assert!(
                entry.key_path.starts_with("HK"),
                "Registry key should start with HK: {}",
                entry.key_path
            );
        }
    }

    #[test]
    fn file_paths_start_with_drive_letter_or_backslash() {
        for plan in [
            RemovalPlan::for_product(Product::McAfee),
            RemovalPlan::for_product(Product::Norton),
            RemovalPlan::for_product(Product::Avast),
            RemovalPlan::for_product(Product::Avg),
        ] {
            for fp in &plan.file_paths {
                let first = fp.path.chars().next().unwrap_or(' ');
                assert!(
                    first == 'C' || first == '\\',
                    "Path should be absolute: {}",
                    fp.path
                );
            }
        }
    }
}
