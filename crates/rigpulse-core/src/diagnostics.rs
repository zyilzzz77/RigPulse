use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FindingStatus {
    Detected,
    NotDetected,
    Unavailable,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub category: String,
    pub severity: Severity,
    pub status: FindingStatus,
    pub title: String,
    pub explanation: String,
    pub evidence: String,
    pub recommended_action: String,
    pub at: chrono::DateTime<chrono::Utc>,
    pub can_auto_fix: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticRun {
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub finished_at: chrono::DateTime<chrono::Utc>,
    pub days: u32,
    pub findings: Vec<Finding>,
}
impl DiagnosticRun {
    pub fn summary(&self) -> &'static str {
        if self
            .findings
            .iter()
            .any(|f| f.status == FindingStatus::Detected && f.severity == Severity::Critical)
        {
            "Masalah terdeteksi"
        } else if self
            .findings
            .iter()
            .any(|f| f.status == FindingStatus::Detected && f.severity == Severity::Warning)
        {
            "Perlu perhatian"
        } else if self.findings.is_empty()
            || self
                .findings
                .iter()
                .any(|f| f.status == FindingStatus::Unavailable)
        {
            "Pemeriksaan belum lengkap"
        } else {
            "Tidak ditemukan masalah pada pemeriksaan yang berhasil"
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incomplete_never_claims_healthy() {
        let mut d = DiagnosticRun {
            started_at: chrono::Utc::now(),
            finished_at: chrono::Utc::now(),
            days: 7,
            findings: Vec::new(),
        };
        assert_eq!(d.summary(), "Pemeriksaan belum lengkap");
        d.findings.push(Finding {
            category: "CPU".into(),
            severity: Severity::Info,
            status: FindingStatus::Unavailable,
            title: "Tidak tersedia".into(),
            explanation: String::new(),
            evidence: String::new(),
            recommended_action: String::new(),
            at: chrono::Utc::now(),
            can_auto_fix: false,
        });
        assert_eq!(d.summary(), "Pemeriksaan belum lengkap");
    }
}
