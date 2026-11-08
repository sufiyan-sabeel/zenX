use serde::Serialize;

use crate::fix::Fix;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pass,
    Warn,
    Fail,
    Skip,
}

impl Status {
    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Pass => "pass",
            Status::Warn => "warn",
            Status::Fail => "fail",
            Status::Skip => "skip",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Warn => "WARN",
            Status::Fail => "FAIL",
            Status::Skip => "SKIP",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub id: String,
    pub title: String,
    pub status: Status,
    pub detail: String,
    pub evidence: Vec<String>,
}

impl Check {
    pub fn new(id: &str, title: &str, status: Status, detail: String) -> Self {
        Check {
            id: id.to_string(),
            title: title.to_string(),
            status,
            detail,
            evidence: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Summary {
    pub pass: usize,
    pub warn: usize,
    pub fail: usize,
    pub skip: usize,
}

impl Summary {
    pub fn from_checks(checks: &[Check]) -> Self {
        let mut summary = Summary::default();
        for check in checks {
            match check.status {
                Status::Pass => summary.pass += 1,
                Status::Warn => summary.warn += 1,
                Status::Fail => summary.fail += 1,
                Status::Skip => summary.skip += 1,
            }
        }
        summary
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub root: String,
    pub generated_at: String,
    pub ecosystems: Vec<String>,
    pub checks: Vec<Check>,
    pub summary: Summary,
    pub fixes: Vec<Fix>,
}

impl DoctorReport {
    pub fn is_healthy(&self) -> bool {
        self.summary.fail == 0
    }

    pub fn failures(&self) -> Vec<&Check> {
        self.checks
            .iter()
            .filter(|check| check.status == Status::Fail)
            .collect()
    }

    pub fn render_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Repository health report\n\n");
        out.push_str(&format!("- Repository: {}\n", self.root));
        out.push_str(&format!("- Generated: {}\n", self.generated_at));
        out.push_str(&format!(
            "- Ecosystems: {}\n",
            if self.ecosystems.is_empty() {
                "none detected".to_string()
            } else {
                self.ecosystems.join(", ")
            }
        ));
        out.push_str(&format!(
            "- Result: {} pass, {} warn, {} fail, {} skip\n\n",
            self.summary.pass, self.summary.warn, self.summary.fail, self.summary.skip
        ));

        out.push_str("| Status | Check | Detail |\n|--------|-------|--------|\n");
        for check in &self.checks {
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                check.status.symbol(),
                check.title,
                check.detail.replace('|', "\\|")
            ));
        }

        let problems: Vec<&Check> = self
            .checks
            .iter()
            .filter(|check| matches!(check.status, Status::Fail | Status::Warn))
            .collect();
        if !problems.is_empty() {
            out.push_str("\n## Findings\n\n");
            for check in problems {
                out.push_str(&format!("### {} — {}\n\n", check.status.symbol(), check.title));
                out.push_str(&format!("{}\n\n", check.detail));
                if !check.evidence.is_empty() {
                    for item in &check.evidence {
                        out.push_str(&format!("- {item}\n"));
                    }
                    out.push('\n');
                }
            }
        }

        if !self.fixes.is_empty() {
            out.push_str("\n## Available repairs\n\n");
            out.push_str("Run `zenx doctor --fix` to review these, then `zenx doctor --fix --apply`.\n\n");
            for fix in &self.fixes {
                out.push_str(&format!("- {} → {}\n", fix.title, fix.path));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> DoctorReport {
        DoctorReport {
            root: "/repo".into(),
            generated_at: "2026-01-01T00:00:00Z".into(),
            ecosystems: vec!["rust".into()],
            checks: vec![
                Check::new("docs.readme", "README present", Status::Pass, "ok".into()),
                Check::new("ci.present", "CI present", Status::Fail, "missing".into()),
            ],
            summary: Summary::default(),
            fixes: Vec::new(),
        }
    }

    #[test]
    fn summary_counts_each_status() {
        let report = sample();
        let summary = Summary::from_checks(&report.checks);
        assert_eq!((summary.pass, summary.fail), (1, 1));
        assert_eq!((summary.warn, summary.skip), (0, 0));
    }

    #[test]
    fn health_requires_no_failures() {
        let mut report = sample();
        assert!(!report.is_healthy());
        report.checks[1].status = Status::Warn;
        assert!(report.is_healthy());
    }

    #[test]
    fn markdown_escapes_pipes_and_lists_findings() {
        let mut report = sample();
        report.checks[0].detail = "a|b".into();
        let text = report.render_markdown();
        assert!(text.contains("a\\|b"));
        assert!(text.contains("## Findings"));
        assert!(text.contains("FAIL — CI present"));
    }
}
