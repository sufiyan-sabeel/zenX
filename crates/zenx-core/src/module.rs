use serde::Serialize;

/// How complete a module is. Advertised honestly in `zenx status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ModuleStatus {
    /// Implemented and covered by tests in this repository.
    Implemented,
    /// Implemented but with documented gaps.
    Partial,
    /// Not built yet. The CLI has no command for it.
    Planned,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ModuleInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
    pub status: ModuleStatus,
}

/// The module registry. One entry per product module, in product order.
pub const MODULES: &[ModuleInfo] = &[
    ModuleInfo {
        id: "trace",
        name: "zenX Trace",
        summary: "Record and inspect AI agent execution sessions",
        status: ModuleStatus::Implemented,
    },
    ModuleInfo {
        id: "skills",
        name: "zenX Skills",
        summary: "Install, verify, update and roll back agent skills",
        status: ModuleStatus::Implemented,
    },
    ModuleInfo {
        id: "doctor",
        name: "zenX Doctor",
        summary: "Repository health diagnostics with a reviewable repair mode",
        status: ModuleStatus::Implemented,
    },
    ModuleInfo {
        id: "mobile",
        name: "zenX Mobile",
        summary: "Android and Termux environment diagnostics, backup and restore",
        status: ModuleStatus::Implemented,
    },
    ModuleInfo {
        id: "bench",
        name: "zenX Bench",
        summary: "Reproducible, measured benchmarking of agent tasks",
        status: ModuleStatus::Implemented,
    },
    ModuleInfo {
        id: "contract",
        name: "zenX Contract",
        summary: "OpenAPI breaking-change detection",
        status: ModuleStatus::Implemented,
    },
    ModuleInfo {
        id: "docs",
        name: "zenX Docs",
        summary: "Offline documentation import and full-text search",
        status: ModuleStatus::Implemented,
    },
    ModuleInfo {
        id: "dashboard",
        name: "zenX Dashboard",
        summary: "Local React dashboard over the same SQLite data",
        status: ModuleStatus::Planned,
    },
];

pub fn find(id: &str) -> Option<&'static ModuleInfo> {
    MODULES.iter().find(|module| module.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_unique_ids() {
        let mut ids: Vec<&str> = MODULES.iter().map(|m| m.id).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total);
    }

    #[test]
    fn dashboard_is_not_claimed_as_implemented() {
        assert_eq!(find("dashboard").unwrap().status, ModuleStatus::Planned);
    }
}
