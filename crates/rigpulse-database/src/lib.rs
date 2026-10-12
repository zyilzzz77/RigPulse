use rigpulse_core::DiagnosticRun;
use rusqlite::{Connection, params};
pub struct Database {
    connection: Connection,
}
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub id: i64,
    pub at: String,
    pub kind: String,
    pub status: String,
}
impl Database {
    pub fn open(path: &std::path::Path) -> anyhow::Result<Self> {
        let connection = Connection::open(path)?;
        Self::from_connection(connection)
    }
    fn from_connection(mut connection: Connection) -> anyhow::Result<Self> {
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.busy_timeout(std::time::Duration::from_secs(2))?;
        let version: u32 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
        anyhow::ensure!(version <= 1, "Versi database lebih baru daripada aplikasi");
        if version == 0 {
            let tx = connection.transaction()?;
            tx.execute_batch(include_str!("../../../migrations/0001_initial.sql"))?;
            tx.pragma_update(None, "user_version", 1)?;
            tx.commit()?;
        }
        Ok(Self { connection })
    }
    pub fn save_scan(&mut self, run: &DiagnosticRun) -> anyhow::Result<i64> {
        let tx = self.connection.transaction()?;
        tx.execute("INSERT INTO scan_runs(scan_type,started_at,finished_at,status,summary_json) VALUES(?1,?2,?3,?4,?5)",params!["Diagnostics",run.started_at.to_rfc3339(),run.finished_at.to_rfc3339(),run.summary(),serde_json::to_string(run)?])?;
        let id = tx.last_insert_rowid();
        for f in &run.findings {
            tx.execute("INSERT INTO scan_findings(scan_run_id,category,severity,status,title,explanation,evidence_json,recommended_action) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![id,f.category,format!("{:?}",f.severity),format!("{:?}",f.status),f.title,f.explanation,f.evidence,f.recommended_action])?;
        }
        // Retain a bounded number of scans; cascade removes associated findings.
        tx.execute("DELETE FROM scan_runs WHERE id NOT IN (SELECT id FROM scan_runs ORDER BY id DESC LIMIT 200)",[])?;
        tx.commit()?;
        Ok(id)
    }
    pub fn history(&self) -> anyhow::Result<Vec<HistoryEntry>> {
        let mut q = self.connection.prepare(
            "SELECT id,started_at,scan_type,status FROM scan_runs ORDER BY id DESC LIMIT 200",
        )?;
        Ok(q.query_map([], |r| {
            Ok(HistoryEntry {
                id: r.get(0)?,
                at: r.get(1)?,
                kind: r.get(2)?,
                status: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?)
    }
    pub fn scan(&self, id: i64) -> anyhow::Result<DiagnosticRun> {
        let raw: String = self.connection.query_row(
            "SELECT summary_json FROM scan_runs WHERE id=?1",
            [id],
            |r| r.get(0),
        )?;
        Ok(serde_json::from_str(&raw)?)
    }
    pub fn save_battery(&mut self, b: &rigpulse_core::BatteryReading) -> anyhow::Result<()> {
        let tx = self.connection.transaction()?;
        for p in &b.packs {
            tx.execute("INSERT INTO battery_health_history(collected_at,design_capacity_mwh,full_charge_capacity_mwh,health_percent,source) VALUES(?1,?2,?3,?4,?5)",params![b.at.to_rfc3339(),p.design_mwh,p.full_mwh,p.health(),"Windows Battery IOCTL"])?;
        }
        tx.execute("DELETE FROM battery_health_history WHERE id NOT IN (SELECT id FROM battery_health_history ORDER BY id DESC LIMIT 365)",[])?;
        tx.commit()?;
        Ok(())
    }
    pub fn delete_history(&mut self) -> anyhow::Result<()> {
        let tx = self.connection.transaction()?;
        tx.execute("DELETE FROM scan_runs", [])?;
        tx.execute("DELETE FROM driver_operations", [])?;
        tx.execute("DELETE FROM battery_health_history", [])?;
        tx.commit()?;
        self.connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;")?;
        Ok(())
    }
    pub fn begin_driver_operation(&self, packages: usize) -> anyhow::Result<i64> {
        self.connection.execute("INSERT INTO driver_operations(device_label,source,action,state,started_at) VALUES(?1,?2,?3,?4,?5)",params![format!("{packages} paket dipilih pengguna"),"Windows Update Agent","Download/install","AwaitingConfirmation",chrono::Utc::now().to_rfc3339()])?;
        Ok(self.connection.last_insert_rowid())
    }
    pub fn finish_driver_operation(
        &self,
        id: i64,
        state: &str,
        error: Option<&str>,
        restart: bool,
    ) -> anyhow::Result<()> {
        self.connection.execute("UPDATE driver_operations SET state=?1,finished_at=?2,error_message=?3,restart_required=?4 WHERE id=?5",params![state,chrono::Utc::now().to_rfc3339(),error,restart,id])?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migration_roundtrip_delete_and_cascade() {
        let mut db = Database::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        let run = DiagnosticRun {
            started_at: chrono::Utc::now(),
            finished_at: chrono::Utc::now(),
            days: 7,
            findings: Vec::new(),
        };
        let id = db.save_scan(&run).unwrap();
        assert_eq!(db.scan(id).unwrap().days, 7);
        assert_eq!(db.history().unwrap().len(), 1);
        db.delete_history().unwrap();
        assert!(db.history().unwrap().is_empty());
    }
    #[test]
    fn migration_rejects_newer_schema() {
        let c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "user_version", 99).unwrap();
        assert!(Database::from_connection(c).is_err());
    }
    #[test]
    fn failed_scan_insert_rolls_back_and_retention_cascades() {
        let mut db = Database::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        let run = DiagnosticRun {
            started_at: chrono::Utc::now(),
            finished_at: chrono::Utc::now(),
            days: 1,
            findings: vec![rigpulse_core::Finding {
                category: "Test".into(),
                severity: rigpulse_core::Severity::Info,
                status: rigpulse_core::FindingStatus::NotDetected,
                title: "Test".into(),
                explanation: "Test".into(),
                evidence: "Test".into(),
                recommended_action: "Test".into(),
                at: chrono::Utc::now(),
                can_auto_fix: false,
            }],
        };
        db.connection.execute_batch("CREATE TRIGGER reject_finding BEFORE INSERT ON scan_findings BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
        assert!(db.save_scan(&run).is_err());
        assert!(db.history().unwrap().is_empty());
        db.connection
            .execute_batch("DROP TRIGGER reject_finding;")
            .unwrap();
        for _ in 0..202 {
            db.save_scan(&run).unwrap();
        }
        assert_eq!(db.history().unwrap().len(), 200);
        let count: i64 = db
            .connection
            .query_row("SELECT COUNT(*) FROM scan_findings", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 200);
        db.delete_history().unwrap();
        let count: i64 = db
            .connection
            .query_row("SELECT COUNT(*) FROM scan_findings", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
    #[test]
    fn records_driver_failure_without_sensitive_identifiers() {
        let db = Database::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        let id = db.begin_driver_operation(2).unwrap();
        db.finish_driver_operation(id, "Failed", Some("Access denied"), false)
            .unwrap();
        let state: String = db
            .connection
            .query_row(
                "SELECT state FROM driver_operations WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(state, "Failed");
    }
}
