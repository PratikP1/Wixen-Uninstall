//! Removal plan — pure data describing *what* would be removed.
//!
//! Author: PratikP1
//!
//! The plan is built from static knowledge of where each supported product
//! stores its files, registry keys, services, and scheduled tasks.  The
//! products shipped today happen to be stubborn security suites, but the plan
//! is just data — any application can be described the same way.  Nothing is
//! deleted here; the executor module consumes the plan.
//!
//! File templates use the placeholders defined in [`crate::paths`] rather than
//! a literal `C:\`, and every expanded path is validated before it enters the
//! plan.  A template that cannot be resolved safely is dropped, and the unit
//! tests assert that no shipped template ever is.

use crate::{paths::WindowsLocations, product::Product};

/// The registry path segment that marks an Add/Remove Programs entry.
const UNINSTALL_KEY_MARKER: &str = r"\Uninstall\";

fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    let needle = needle.as_bytes();
    !needle.is_empty()
        && haystack
            .as_bytes()
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle))
}

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
    /// Set for kernel driver images: the service that loads this file.
    ///
    /// Windows refuses to boot when a registered boot-start or system-start
    /// driver's image is missing, so the executor must not delete the file
    /// until that service has been removed.  See [`FilePath::blocking_guard`].
    pub guard_service: Option<String>,
}

impl FilePath {
    pub fn file(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            is_dir: false,
            guard_service: None,
        }
    }

    pub fn dir(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            is_dir: true,
            guard_service: None,
        }
    }

    /// A kernel driver image that is only safe to delete once `service` is gone.
    pub fn driver(path: impl Into<String>, service: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            is_dir: false,
            guard_service: Some(service.into()),
        }
    }

    /// The service that still blocks deletion, given the set of services the
    /// executor has already removed.
    pub fn blocking_guard<'a>(&'a self, removed_services: &[&str]) -> Option<&'a str> {
        self.guard_service
            .as_deref()
            .filter(|service| !removed_services.contains(service))
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
    /// Build the complete removal plan for `product` on this machine.
    pub fn for_product(product: Product) -> Self {
        Self::for_product_at(product, &WindowsLocations::from_env())
    }

    /// Build the plan against explicit locations, so tests can exercise
    /// machines whose Windows does not live on C:.
    pub fn for_product_at(product: Product, locations: &WindowsLocations) -> Self {
        let templates = templates_for(product);

        Self {
            product,
            registry_entries: templates
                .registry_keys
                .iter()
                .copied()
                .map(RegistryEntry::key)
                .collect(),
            file_paths: resolve_file_paths(templates, locations),
            services: templates
                .services
                .iter()
                .copied()
                .map(ServiceEntry::new)
                .collect(),
            scheduled_tasks: templates
                .scheduled_tasks
                .iter()
                .copied()
                .map(ScheduledTask::new)
                .collect(),
        }
    }

    /// The registry keys under `…\Uninstall\…`, whose values carry the
    /// vendor's own uninstall command.
    ///
    /// Probed before deletion so the product can be asked to remove itself —
    /// the one route past self-protection that does not require Safe Mode,
    /// since a product cannot block its own uninstaller.
    pub fn uninstall_keys(&self) -> impl Iterator<Item = &str> {
        self.registry_entries
            .iter()
            .map(|entry| entry.key_path.as_str())
            .filter(|key| contains_ignore_ascii_case(key, UNINSTALL_KEY_MARKER))
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

// ─── Static product knowledge ────────────────────────────────────────────────

/// A kernel driver image paired with the service that loads it.
///
/// The pairing is what makes the guard in [`FilePath::driver`] possible: we can
/// only delete `aswSP.sys` once the `aswSP` service is gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriverService {
    pub image_template: &'static str,
    pub service: &'static str,
}

impl DriverService {
    const fn new(image_template: &'static str, service: &'static str) -> Self {
        Self {
            image_template,
            service,
        }
    }
}

/// The unresolved knowledge base for one product.
struct ProductTemplates {
    registry_keys: &'static [&'static str],
    directories: &'static [&'static str],
    drivers: &'static [DriverService],
    services: &'static [&'static str],
    scheduled_tasks: &'static [&'static str],
}

fn templates_for(product: Product) -> &'static ProductTemplates {
    match product {
        Product::McAfee => &MCAFEE_TEMPLATES,
        Product::Norton => &NORTON_TEMPLATES,
        Product::Avast => &AVAST_TEMPLATES,
        Product::Avg => &AVG_TEMPLATES,
    }
}

/// Expand and validate every path template, dropping any that a machine's
/// layout makes unsafe.  Directories come first so that a failed driver guard
/// never leaves a directory unswept.
fn resolve_file_paths(templates: &ProductTemplates, locations: &WindowsLocations) -> Vec<FilePath> {
    let directories = templates
        .directories
        .iter()
        .filter_map(|template| Some(FilePath::dir(locations.resolve(template).ok()?)));

    let drivers = templates.drivers.iter().filter_map(|driver| {
        Some(FilePath::driver(
            locations.resolve(driver.image_template).ok()?,
            driver.service,
        ))
    });

    directories.chain(drivers).collect()
}

// ─── McAfee knowledge base ───────────────────────────────────────────────────

const MCAFEE_TEMPLATES: ProductTemplates = ProductTemplates {
    registry_keys: &[
        r"HKLM\SOFTWARE\McAfee",
        r"HKLM\SOFTWARE\WOW6432Node\McAfee",
        r"HKLM\SOFTWARE\McAfee.com",
        r"HKLM\SYSTEM\CurrentControlSet\Services\McShield",
        r"HKLM\SYSTEM\CurrentControlSet\Services\McAfee WebAdvisor",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\McAfee Total Protection",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\McAfee LiveSafe",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\McAfee WebAdvisor",
        r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\McAfee WebAdvisor",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\McAfee SiteAdvisor",
        r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\McAfee SiteAdvisor",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\McAfee.WPS",
        r"HKCU\SOFTWARE\McAfee",
    ],
    // Parent directories only: recursive deletion sweeps WebAdvisor and
    // SiteAdvisor sub-folders with them.
    directories: &[
        r"{ProgramFiles}\McAfee",
        r"{ProgramFilesX86}\McAfee",
        r"{ProgramFiles}\McAfee.com",
        r"{ProgramFilesX86}\McAfee.com",
        r"{ProgramFiles}\Common Files\McAfee",
        r"{ProgramFilesX86}\Common Files\McAfee",
        r"{ProgramData}\McAfee",
    ],
    drivers: &[
        DriverService::new(r"{SystemRoot}\System32\drivers\mfehidk.sys", "mfehidk"),
        DriverService::new(r"{SystemRoot}\System32\drivers\mfefirek.sys", "mfefirek"),
        DriverService::new(r"{SystemRoot}\System32\drivers\mfewfpk.sys", "mfewfpk"),
    ],
    services: &[
        "McShield",
        "McAfeeEngineService",
        "McAfee WebAdvisor",
        "mfemms",
        "mfefire",
        "mfehidk",
        "mfefirek",
        "mfewfpk",
    ],
    scheduled_tasks: &[
        r"\McAfee\McAfee Auto Maintenance",
        r"\McAfee\McAfeeLogon",
        r"\McAfee\McAfee WebAdvisor",
    ],
};

// ─── Norton knowledge base ───────────────────────────────────────────────────

const NORTON_TEMPLATES: ProductTemplates = ProductTemplates {
    registry_keys: &[
        r"HKLM\SOFTWARE\Norton",
        r"HKLM\SOFTWARE\WOW6432Node\Norton",
        r"HKLM\SOFTWARE\Symantec",
        r"HKLM\SOFTWARE\WOW6432Node\Symantec",
        r"HKLM\SYSTEM\CurrentControlSet\Services\NortonSecurity",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Norton 360",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Norton Secure VPN",
        r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Norton Secure VPN",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Norton Utilities",
        r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Norton Utilities",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Norton Utilities Ultimate",
        r"HKCU\SOFTWARE\Norton",
        r"HKCU\SOFTWARE\Symantec",
    ],
    directories: &[
        r"{ProgramFiles}\Norton Security",
        r"{ProgramFiles}\Norton 360",
        r"{ProgramFiles}\Norton",
        r"{ProgramFiles}\Norton Secure VPN",
        r"{ProgramFiles}\Norton Utilities",
        r"{ProgramFilesX86}\Norton Security",
        r"{ProgramFilesX86}\Norton 360",
        r"{ProgramFilesX86}\Norton",
        r"{ProgramFilesX86}\Norton Secure VPN",
        r"{ProgramFilesX86}\Norton Utilities",
        r"{ProgramFiles}\Common Files\Symantec Shared",
        r"{ProgramFilesX86}\Common Files\Symantec Shared",
        r"{ProgramData}\Norton",
        r"{ProgramData}\Symantec",
    ],
    drivers: &[DriverService::new(
        r"{SystemRoot}\System32\drivers\NortonSecurity.sys",
        "NortonSecurity",
    )],
    services: &[
        "NortonSecurity",
        "NortonSecurityPlatformIDS",
        "Symantec Event Manager",
        "Symantec Settings Manager",
    ],
    scheduled_tasks: &[
        r"\Norton Security\Norton Error Processor",
        r"\Norton Security\Norton Error Submitter",
        r"\Symantec\Norton Update Manager",
    ],
};

// ─── Avast knowledge base ────────────────────────────────────────────────────

const AVAST_TEMPLATES: ProductTemplates = ProductTemplates {
    registry_keys: &[
        r"HKLM\SOFTWARE\AVAST Software",
        r"HKLM\SOFTWARE\WOW6432Node\AVAST Software",
        r"HKLM\SYSTEM\CurrentControlSet\Services\AvastSvc",
        r"HKLM\SYSTEM\CurrentControlSet\Services\AvastWscReporter",
        r"HKLM\SYSTEM\CurrentControlSet\Services\aswMonFlt",
        r"HKLM\SYSTEM\CurrentControlSet\Services\aswSnx",
        r"HKLM\SYSTEM\CurrentControlSet\Services\aswSP",
        r"HKLM\SYSTEM\CurrentControlSet\Services\aswVmm",
        r"HKLM\SYSTEM\CurrentControlSet\Services\aswRdr2",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Avast Antivirus",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Avast Secure Browser",
        r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Avast Secure Browser",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Avast Cleanup",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Avast Cleanup Premium",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Avast Driver Updater",
        r"HKCU\SOFTWARE\AVAST Software",
    ],
    directories: &[
        r"{ProgramFiles}\AVAST Software",
        r"{ProgramFilesX86}\AVAST Software",
        r"{ProgramFiles}\Common Files\Avast Software",
        r"{ProgramFilesX86}\Common Files\Avast Software",
        r"{ProgramData}\AVAST Software",
    ],
    drivers: &[
        DriverService::new(r"{SystemRoot}\System32\drivers\aswSP.sys", "aswSP"),
        DriverService::new(r"{SystemRoot}\System32\drivers\aswSnx.sys", "aswSnx"),
        DriverService::new(r"{SystemRoot}\System32\drivers\aswMonFlt.sys", "aswMonFlt"),
        DriverService::new(r"{SystemRoot}\System32\drivers\aswVmm.sys", "aswVmm"),
        DriverService::new(r"{SystemRoot}\System32\drivers\aswRdr2.sys", "aswRdr2"),
    ],
    services: &[
        "AvastSvc",
        "AvastWscReporter",
        "aswbIDSAgent",
        "aswMonFlt",
        "aswSnx",
        "aswSP",
        "aswVmm",
        "aswRdr2",
    ],
    scheduled_tasks: &[
        r"\AVAST Software\Avast\Overseer",
        r"\AVAST Software\Avast\AutoRepair",
        r"\AVAST Software\Avast\Periodic Scan",
        r"\AVAST Software\Avast\AvastBrowserUpdate",
    ],
};

// ─── AVG knowledge base ──────────────────────────────────────────────────────

const AVG_TEMPLATES: ProductTemplates = ProductTemplates {
    registry_keys: &[
        r"HKLM\SOFTWARE\AVG",
        r"HKLM\SOFTWARE\WOW6432Node\AVG",
        r"HKLM\SYSTEM\CurrentControlSet\Services\avgSvc",
        r"HKLM\SYSTEM\CurrentControlSet\Services\avgMonFlt",
        r"HKLM\SYSTEM\CurrentControlSet\Services\avgSP",
        r"HKLM\SYSTEM\CurrentControlSet\Services\avgRdr",
        r"HKLM\SYSTEM\CurrentControlSet\Services\AVGIDSDriver",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\AVG Antivirus",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\AVG Secure Browser",
        r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\AVG Secure Browser",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\AVG PC TuneUp",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\AVG TuneUp",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\AVG Driver Updater",
        r"HKCU\SOFTWARE\AVG",
    ],
    directories: &[
        r"{ProgramFiles}\AVG",
        r"{ProgramFilesX86}\AVG",
        r"{ProgramFiles}\Common Files\AVG",
        r"{ProgramData}\AVG",
        r"{ProgramData}\TuneUp Software",
    ],
    drivers: &[
        DriverService::new(r"{SystemRoot}\System32\drivers\avgSP.sys", "avgSP"),
        DriverService::new(r"{SystemRoot}\System32\drivers\avgMonFlt.sys", "avgMonFlt"),
        DriverService::new(r"{SystemRoot}\System32\drivers\avgRdr.sys", "avgRdr"),
    ],
    services: &["avgSvc", "avgMonFlt", "avgSP", "avgRdr", "AVGIDSDriver"],
    scheduled_tasks: &[
        r"\AVG\Antivirus\Overseer",
        r"\AVG\Antivirus\AutoRepair",
        r"\AVG\Antivirus\Periodic Scan",
    ],
};

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::WindowsLocations;

    fn all_templates() -> impl Iterator<Item = &'static ProductTemplates> {
        Product::all().iter().copied().map(templates_for)
    }

    // ── RegistryEntry ────────────────────────────────────────────────────────

    #[test]
    fn registry_key_has_no_value_name() {
        let entry = RegistryEntry::key(r"HKLM\SOFTWARE\McAfee");
        assert_eq!(entry.key_path, r"HKLM\SOFTWARE\McAfee");
        assert!(entry.value_name.is_none());
    }

    #[test]
    fn registry_value_stores_name() {
        let entry = RegistryEntry::value(r"HKLM\SOFTWARE\McAfee", "Version");
        assert_eq!(entry.value_name.as_deref(), Some("Version"));
    }

    // ── FilePath ─────────────────────────────────────────────────────────────

    #[test]
    fn file_path_is_not_dir() {
        assert!(!FilePath::file(r"C:\Windows\System32\mfehidk.sys").is_dir);
    }

    #[test]
    fn dir_path_is_dir() {
        assert!(FilePath::dir(r"C:\Program Files\McAfee").is_dir);
    }

    #[test]
    fn plain_files_are_never_guarded() {
        assert_eq!(FilePath::file(r"C:\x\y.txt").blocking_guard(&[]), None);
        assert_eq!(FilePath::dir(r"C:\x\y").blocking_guard(&[]), None);
    }

    #[test]
    fn driver_is_blocked_until_its_service_is_removed() {
        let driver = FilePath::driver(r"C:\Windows\System32\drivers\aswSP.sys", "aswSP");
        assert_eq!(driver.blocking_guard(&[]), Some("aswSP"));
        assert_eq!(driver.blocking_guard(&["AvastSvc"]), Some("aswSP"));
        assert_eq!(driver.blocking_guard(&["aswSP"]), None);
    }

    // ── Path safety across the whole knowledge base ──────────────────────────

    #[test]
    fn every_shipped_path_template_resolves_safely() {
        for drive in ["C:", "D:"] {
            let locations = WindowsLocations::conventional(drive);
            for templates in all_templates() {
                let paths = templates
                    .directories
                    .iter()
                    .copied()
                    .chain(templates.drivers.iter().map(|driver| driver.image_template));
                for template in paths {
                    assert!(
                        locations.resolve(template).is_ok(),
                        "{template} must resolve safely on {drive}: {:?}",
                        locations.resolve(template)
                    );
                }
            }
        }
    }

    #[test]
    fn every_guarded_driver_has_a_matching_service_in_the_plan() {
        for &product in Product::all() {
            let plan = RemovalPlan::for_product(product);
            let service_names: Vec<&str> = plan
                .services
                .iter()
                .map(|entry| entry.name.as_str())
                .collect();

            for file in &plan.file_paths {
                let Some(guard) = file.guard_service.as_deref() else {
                    continue;
                };
                assert!(
                    service_names.contains(&guard),
                    "{product}: driver {} is guarded by service {guard}, which the plan never removes",
                    file.path
                );
            }
        }
    }

    #[test]
    fn every_driver_image_is_guarded() {
        for &product in Product::all() {
            for file in &RemovalPlan::for_product(product).file_paths {
                if file.path.to_ascii_lowercase().ends_with(".sys") {
                    assert!(
                        file.guard_service.is_some(),
                        "{product}: driver image {} must be guarded",
                        file.path
                    );
                }
            }
        }
    }

    #[test]
    fn no_plan_path_is_nested_inside_another_plan_path() {
        for &product in Product::all() {
            let plan = RemovalPlan::for_product(product);
            for outer in &plan.file_paths {
                if !outer.is_dir {
                    continue;
                }
                let prefix = format!("{}\\", outer.path).to_ascii_lowercase();
                for inner in &plan.file_paths {
                    assert!(
                        !inner.path.to_ascii_lowercase().starts_with(&prefix)
                            || inner.guard_service.is_some(),
                        "{product}: {} is already swept by {}",
                        inner.path,
                        outer.path
                    );
                }
            }
        }
    }

    #[test]
    fn plans_follow_a_relocated_windows_drive() {
        let plan =
            RemovalPlan::for_product_at(Product::Avast, &WindowsLocations::conventional("D:"));
        assert!(
            plan.file_paths
                .iter()
                .all(|file| file.path.starts_with("D:\\")),
            "every path should follow the resolver: {:?}",
            plan.file_paths
        );
    }

    // ── RemovalPlan::action_count ────────────────────────────────────────────

    #[test]
    fn every_product_plan_is_non_empty() {
        for &product in Product::all() {
            let plan = RemovalPlan::for_product(product);
            assert!(plan.is_non_empty(), "{product} plan should not be empty");
            assert!(!plan.registry_entries.is_empty());
            assert!(!plan.file_paths.is_empty());
            assert!(!plan.services.is_empty());
            assert!(!plan.scheduled_tasks.is_empty());
        }
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
                .any(|entry| entry.path.contains("McAfee.com"))
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
            entry.path.contains("Norton Secure VPN") || entry.path.contains("Norton Utilities")
        }));
    }

    #[test]
    fn avast_plan_covers_browser_and_cleanup() {
        let plan = RemovalPlan::for_product(Product::Avast);
        assert!(plan.registry_entries.iter().any(|entry| {
            entry.key_path.contains("Avast Secure Browser")
                || entry.key_path.contains("Avast Cleanup")
        }));
        assert!(
            plan.file_paths
                .iter()
                .any(|entry| entry.path.contains(r"Common Files\Avast Software"))
        );
    }

    #[test]
    fn avg_plan_covers_browser_and_tuneup() {
        let plan = RemovalPlan::for_product(Product::Avg);
        assert!(plan.registry_entries.iter().any(|entry| {
            entry.key_path.contains("AVG Secure Browser")
                || entry.key_path.contains("AVG PC TuneUp")
                || entry.key_path.contains("AVG TuneUp")
        }));
        assert!(
            plan.file_paths
                .iter()
                .any(|entry| entry.path.contains("TuneUp Software"))
        );
    }

    #[test]
    fn an_empty_plan_is_not_non_empty() {
        let empty = RemovalPlan {
            product: Product::McAfee,
            registry_entries: Vec::new(),
            file_paths: Vec::new(),
            services: Vec::new(),
            scheduled_tasks: Vec::new(),
        };
        assert_eq!(empty.action_count(), 0);
        assert!(!empty.is_non_empty());
    }

    #[test]
    fn a_plan_with_a_single_action_is_non_empty() {
        let single = RemovalPlan {
            product: Product::McAfee,
            registry_entries: vec![RegistryEntry::key(r"HKLM\SOFTWARE\McAfee")],
            file_paths: Vec::new(),
            services: Vec::new(),
            scheduled_tasks: Vec::new(),
        };
        assert_eq!(single.action_count(), 1);
        assert!(single.is_non_empty());
    }

    #[test]
    fn uninstall_keys_are_the_add_remove_programs_entries() {
        for &product in Product::all() {
            let plan = RemovalPlan::for_product(product);
            let keys: Vec<&str> = plan.uninstall_keys().collect();

            assert!(
                !keys.is_empty(),
                "{product} should expose at least one uninstall key to probe"
            );
            for key in &keys {
                assert!(
                    key.contains(r"\Uninstall\"),
                    "{product}: {key} is not an Add/Remove Programs entry"
                );
            }
            // A service key or a software root must not be mistaken for one.
            assert!(
                plan.registry_entries
                    .iter()
                    .any(|entry| !entry.key_path.contains(r"\Uninstall\")),
                "{product}: the plan should also carry non-uninstall keys"
            );
        }
    }

    #[test]
    fn a_plan_with_no_uninstall_keys_yields_none() {
        let plan = RemovalPlan {
            product: Product::McAfee,
            registry_entries: vec![RegistryEntry::key(r"HKLM\SOFTWARE\McAfee")],
            file_paths: Vec::new(),
            services: Vec::new(),
            scheduled_tasks: Vec::new(),
        };
        assert_eq!(plan.uninstall_keys().count(), 0);
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
        for &product in Product::all() {
            assert_eq!(RemovalPlan::for_product(product).product, product);
        }
    }

    #[test]
    fn registry_entries_all_target_a_known_hive() {
        for &product in Product::all() {
            for entry in &RemovalPlan::for_product(product).registry_entries {
                assert!(
                    entry.key_path.starts_with("HKLM\\") || entry.key_path.starts_with("HKCU\\"),
                    "{product}: unexpected hive in {}",
                    entry.key_path
                );
            }
        }
    }

    #[test]
    fn scheduled_tasks_are_absolute_task_paths() {
        for &product in Product::all() {
            for task in &RemovalPlan::for_product(product).scheduled_tasks {
                assert!(
                    task.task_path.starts_with('\\'),
                    "{product}: task path should be absolute: {}",
                    task.task_path
                );
            }
        }
    }
}
