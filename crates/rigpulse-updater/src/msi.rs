//! Inspect the actual MSI database before installation, beyond signed metadata.
use anyhow::ensure;
use std::path::Path;
use windows::{
    Win32::System::ApplicationInstallationAndServicing::*,
    core::{PCWSTR, PWSTR},
};

struct Handle(MSIHANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            MsiCloseHandle(self.0);
        }
    }
}
fn success(code: u32) -> anyhow::Result<()> {
    ensure!(code == 0, "Paket MSI tidak dapat divalidasi (kode {code})");
    Ok(())
}
fn property(database: MSIHANDLE, name: &str) -> anyhow::Result<String> {
    let query: Vec<u16> = format!("SELECT `Value` FROM `Property` WHERE `Property`='{name}'")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    unsafe {
        let mut view = MSIHANDLE(0);
        success(MsiDatabaseOpenViewW(
            database,
            PCWSTR(query.as_ptr()),
            &mut view,
        ))?;
        let view = Handle(view);
        success(MsiViewExecute(view.0, MSIHANDLE(0)))?;
        let mut record = MSIHANDLE(0);
        success(MsiViewFetch(view.0, &mut record))?;
        let record = Handle(record);
        let mut buffer = [0u16; 512];
        let mut len = buffer.len() as u32;
        success(MsiRecordGetStringW(
            record.0,
            1,
            Some(PWSTR(buffer.as_mut_ptr())),
            Some(&mut len),
        ))?;
        Ok(String::from_utf16(&buffer[..len as usize])?)
    }
}
pub fn validate(path: &Path, version: &str) -> anyhow::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let mut database = MSIHANDLE(0);
        success(MsiOpenDatabaseW(
            PCWSTR(path.as_ptr()),
            PCWSTR::null(),
            &mut database,
        ))?;
        let database = Handle(database);
        ensure!(
            property(database.0, "ProductName")? == "RigPulse",
            "MSI bukan produk RigPulse"
        );
        ensure!(
            property(database.0, "ProductVersion")? == version,
            "Versi MSI berbeda dari rilis yang ditandatangani"
        );
        ensure!(
            property(database.0, "UpgradeCode")?
                .eq_ignore_ascii_case("{E10F4D59-89E4-44BC-BB62-C6C8DE55357F}"),
            "MSI memiliki identitas upgrade yang berbeda"
        );
        let mut summary = MSIHANDLE(0);
        success(MsiGetSummaryInformationW(
            database.0,
            PCWSTR::null(),
            0,
            &mut summary,
        ))?;
        let summary = Handle(summary);
        let (mut datatype, mut value, mut len) = (0, 0, 128u32);
        let mut template = [0u16; 128];
        success(MsiSummaryInfoGetPropertyW(
            summary.0,
            7,
            &mut datatype,
            &mut value,
            None,
            Some(PWSTR(template.as_mut_ptr())),
            Some(&mut len),
        ))?;
        let template = String::from_utf16(&template[..len as usize])?;
        ensure!(
            template.split(';').next() == Some("x64"),
            "Arsitektur MSI tidak kompatibel"
        );
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "Requires the packaged release MSI; read-only database validation"]
    fn real_release_msi_identity_and_wrong_version() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../dist")
            .join(format!("RigPulse-{}-x64.msi", env!("CARGO_PKG_VERSION")));
        super::validate(&path, env!("CARGO_PKG_VERSION")).unwrap();
        assert!(super::validate(&path, "999.0.0").is_err());
    }
}
