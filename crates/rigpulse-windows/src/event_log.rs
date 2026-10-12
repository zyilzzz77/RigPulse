#[derive(Debug, Clone)]
pub struct SystemEvent {
    pub provider: String,
    pub id: String,
    pub level: String,
    pub time: String,
}
#[cfg(windows)]
pub fn recent_errors(days: u32) -> anyhow::Result<Vec<SystemEvent>> {
    use windows::{
        Win32::{
            Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS},
            System::EventLog::*,
        },
        core::PCWSTR,
    };
    struct EventHandle(EVT_HANDLE);
    impl Drop for EventHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = EvtClose(self.0);
            }
        }
    }
    let days = days.clamp(1, 30);
    let query = format!(
        "*[System[(Level=1 or Level=2) and TimeCreated[timediff(@SystemTime) <= {}]]]",
        u64::from(days) * 86400000
    );
    let wide: Vec<u16> = query.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let query = EventHandle(EvtQuery(
            None,
            windows::core::w!("System"),
            PCWSTR(wide.as_ptr()),
            EvtQueryChannelPath.0 | EvtQueryReverseDirection.0,
        )?);
        let mut result = Vec::new();
        while result.len() < 100 {
            let mut handles = [0isize; 10];
            let mut count = 0;
            if let Err(e) = EvtNext(query.0, &mut handles, 1000, 0, &mut count) {
                if e.code() == ERROR_NO_MORE_ITEMS.to_hresult() {
                    break;
                }
                return Err(e.into());
            }
            // Own every event before parsing, so an error closes all handles in this batch.
            let owned: Vec<_> = handles[..count as usize]
                .iter()
                .map(|h| EventHandle(EVT_HANDLE(*h)))
                .collect();
            for event in owned {
                let mut bytes = 0;
                let mut properties = 0;
                let first = EvtRender(
                    None,
                    event.0,
                    EvtRenderEventXml.0,
                    0,
                    None,
                    &mut bytes,
                    &mut properties,
                );
                if let Err(e) = first
                    && e.code() != ERROR_INSUFFICIENT_BUFFER.to_hresult()
                {
                    return Err(e.into());
                }
                anyhow::ensure!(bytes > 0 && bytes < 1048576, "Ukuran Event Log tidak valid");
                let mut buf = vec![0u16; (bytes as usize).div_ceil(2)];
                EvtRender(
                    None,
                    event.0,
                    EvtRenderEventXml.0,
                    bytes,
                    Some(buf.as_mut_ptr().cast()),
                    &mut bytes,
                    &mut properties,
                )?;
                let xml = String::from_utf16_lossy(&buf)
                    .trim_end_matches('\0')
                    .to_owned();
                let document = roxmltree::Document::parse(&xml)?;
                let system = document
                    .descendants()
                    .find(|n| n.has_tag_name("System"))
                    .ok_or_else(|| anyhow::anyhow!("Event System metadata tidak ditemukan"))?;
                let field = |name: &str| system.children().find(|n| n.has_tag_name(name));
                result.push(SystemEvent {
                    provider: field("Provider")
                        .and_then(|n| n.attribute("Name"))
                        .unwrap_or("Tidak tersedia")
                        .into(),
                    id: field("EventID")
                        .and_then(|n| n.text())
                        .unwrap_or("Tidak tersedia")
                        .into(),
                    level: field("Level")
                        .and_then(|n| n.text())
                        .unwrap_or("Tidak tersedia")
                        .into(),
                    time: field("TimeCreated")
                        .and_then(|n| n.attribute("SystemTime"))
                        .unwrap_or("Tidak tersedia")
                        .into(),
                });
            }
        }
        Ok(result)
    }
}
#[cfg(not(windows))]
pub fn recent_errors(_days: u32) -> anyhow::Result<Vec<SystemEvent>> {
    anyhow::bail!("Windows Event Log tidak tersedia")
}
