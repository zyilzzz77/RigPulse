//! Vendor-neutral DXGI inventory and WDDM performance counters. No driver mutation.
use rigpulse_core::{Availability, Measurement};
#[derive(Debug, Clone)]
pub struct Adapter {
    pub name: String,
    pub vendor: String,
    pub dedicated_bytes: u64,
    pub shared_limit_bytes: u64,
    pub luid: String,
}
pub fn vendor_name(id: u32) -> &'static str {
    match id {
        0x10de => "NVIDIA",
        0x1002 | 0x1022 => "AMD",
        0x8086 => "Intel",
        0x5143 => "Qualcomm",
        _ => "Vendor lain",
    }
}
#[cfg(windows)]
pub fn enumerate() -> anyhow::Result<Vec<Adapter>> {
    use windows::Win32::Graphics::Dxgi::*;
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
        let mut adapters = Vec::new();
        for index in 0..64 {
            let adapter = match factory.EnumAdapters1(index) {
                Ok(a) => a,
                Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
                Err(e) => return Err(e.into()),
            };
            let d = adapter.GetDesc1()?;
            if d.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                continue;
            }
            adapters.push(Adapter {
                name: String::from_utf16_lossy(
                    &d.Description[..d
                        .Description
                        .iter()
                        .position(|&v| v == 0)
                        .unwrap_or(d.Description.len())],
                ),
                vendor: vendor_name(d.VendorId).into(),
                dedicated_bytes: d.DedicatedVideoMemory as u64,
                shared_limit_bytes: d.SharedSystemMemory as u64,
                luid: format!(
                    "luid_0x{:08x}_0x{:08x}",
                    d.AdapterLuid.HighPart as u32, d.AdapterLuid.LowPart
                ),
            });
        }
        Ok(adapters)
    }
}
#[cfg(not(windows))]
pub fn enumerate() -> anyhow::Result<Vec<Adapter>> {
    anyhow::bail!("DXGI hanya tersedia pada Windows")
}

/// Sum processes on the same physical engine, then select the busiest engine.
/// Summing every engine would overstate utilization when engines run in parallel.
fn busiest_engine(rows: &[(String, f64)], luid: &str) -> Option<f64> {
    let mut engines = std::collections::BTreeMap::<String, f64>::new();
    for (name, value) in rows {
        if !value.is_finite() || *value < 0.0 {
            continue;
        }
        if let Some(start) = name.find(luid) {
            let key = name[start..].to_owned();
            *engines.entry(key).or_default() += value;
        }
    }
    engines
        .values()
        .copied()
        .reduce(f64::max)
        .map(|v| v.clamp(0.0, 100.0))
}
fn adapter_bytes(rows: &[(String, f64)], luid: &str) -> Option<f64> {
    let values: Vec<f64> = rows
        .iter()
        .filter(|(n, v)| n.starts_with(luid) && v.is_finite() && *v >= 0.0)
        .map(|(_, v)| *v)
        .collect();
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum())
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use windows::{
        Win32::System::Performance::*,
        core::{PCWSTR, w},
    };
    pub struct GpuCounters {
        query: PDH_HQUERY,
        engine: Option<PDH_HCOUNTER>,
        dedicated: Option<PDH_HCOUNTER>,
        shared: Option<PDH_HCOUNTER>,
        pub adapters: Vec<Adapter>,
    }
    impl GpuCounters {
        pub fn new() -> anyhow::Result<Self> {
            let adapters = enumerate()?;
            let mut query = PDH_HQUERY::default();
            let code = unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) };
            anyhow::ensure!(code == 0, "PDH query: 0x{code:08X}");
            let add = |path: PCWSTR| {
                let mut counter = PDH_HCOUNTER::default();
                if unsafe { PdhAddEnglishCounterW(query, path, 0, &mut counter) } == 0 {
                    Some(counter)
                } else {
                    None
                }
            };
            let engine = add(w!("\\GPU Engine(*)\\Utilization Percentage"));
            let dedicated = add(w!("\\GPU Adapter Memory(*)\\Dedicated Usage"));
            let shared = add(w!("\\GPU Adapter Memory(*)\\Shared Usage"));
            let result = Self {
                query,
                engine,
                dedicated,
                shared,
                adapters,
            };
            unsafe {
                PdhCollectQueryData(query);
            }
            Ok(result)
        }
        fn values(counter: Option<PDH_HCOUNTER>) -> anyhow::Result<Vec<(String, f64)>> {
            let counter =
                counter.ok_or_else(|| anyhow::anyhow!("Counter tidak diekspos driver WDDM"))?;
            unsafe {
                let (mut size, mut count) = (0, 0);
                let status = PdhGetFormattedCounterArrayW(
                    counter,
                    PDH_FMT_DOUBLE,
                    &mut size,
                    &mut count,
                    None,
                );
                if status == 0 && size == 0 {
                    return Ok(Vec::new());
                }
                anyhow::ensure!(
                    status == PDH_MORE_DATA && size > 0 && size <= 16 * 1024 * 1024,
                    "Counter belum tersedia: 0x{status:08X}"
                );
                // u64 alignment covers the native item structure and its trailing strings.
                let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
                let status = PdhGetFormattedCounterArrayW(
                    counter,
                    PDH_FMT_DOUBLE,
                    &mut size,
                    &mut count,
                    Some(buffer.as_mut_ptr().cast()),
                );
                anyhow::ensure!(status == 0, "Counter berubah/belum valid: 0x{status:08X}");
                anyhow::ensure!(
                    (count as usize) * std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>()
                        <= buffer.len() * 8,
                    "Ukuran counter tidak valid"
                );
                let items = std::slice::from_raw_parts(
                    buffer.as_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>(),
                    count as usize,
                );
                let mut result = Vec::new();
                for item in items {
                    if matches!(
                        item.FmtValue.CStatus,
                        PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA
                    ) {
                        let addr = item.szName.0 as usize;
                        let start = buffer.as_ptr() as usize;
                        let end = start + buffer.len() * 8;
                        if addr < start || addr >= end || !addr.is_multiple_of(2) {
                            continue;
                        }
                        let string = std::slice::from_raw_parts(item.szName.0, (end - addr) / 2);
                        let Some(len) = string.iter().position(|&c| c == 0) else {
                            continue;
                        };
                        result.push((
                            String::from_utf16_lossy(&string[..len]).to_lowercase(),
                            item.FmtValue.Anonymous.doubleValue,
                        ));
                    }
                }
                Ok(result)
            }
        }
        pub fn sample(&mut self) -> Vec<Measurement> {
            let collected = unsafe { PdhCollectQueryData(self.query) };
            let engine = Self::values(self.engine);
            let dedicated = Self::values(self.dedicated);
            let shared = Self::values(self.shared);
            let mut out = Vec::new();
            for adapter in &self.adapters {
                let name = format!("{} ({})", adapter.name, adapter.vendor);
                let metric = |suffix: &str,
                              unit: &str,
                              result: &anyhow::Result<Vec<(String, f64)>>,
                              usage: bool| {
                    let label = format!("{name} {suffix}");
                    let value = result.as_ref().ok().and_then(|r| {
                        if usage {
                            busiest_engine(r, &adapter.luid)
                        } else {
                            adapter_bytes(r, &adapter.luid).map(|v| v / 1048576.0)
                        }
                    });
                    match value.filter(|_|collected==0) {
                        Some(value)=>Measurement::available(&label,value,unit,"Windows PDH / WDDM"),
                        None=>Measurement::unavailable(&label,unit,"Windows PDH / WDDM",Availability::TemporarilyUnavailable,&result.as_ref().err().map(ToString::to_string).unwrap_or_else(||"Counter GPU belum aktif atau tidak didukung driver; gunakan driver WDDM 2+".into())),
                    }
                };
                out.push(metric("penggunaan (engine tersibuk)", "%", &engine, true));
                out.push(metric(
                    "memori dedicated digunakan",
                    "MiB",
                    &dedicated,
                    false,
                ));
                out.push(metric("memori shared digunakan", "MiB", &shared, false));
                out.push(Measurement::available(
                    &format!("{name} kapasitas dedicated"),
                    adapter.dedicated_bytes as f64 / 1048576.0,
                    "MiB",
                    "DXGI",
                ));
                if adapter.vendor != "NVIDIA" {
                    out.push(Measurement::unavailable(&format!("{name} suhu"),"°C","Windows GPU",Availability::Unsupported,"Windows tidak menyediakan counter suhu universal; provider vendor untuk GPU ini belum tersedia."));
                }
            }
            out
        }
    }
    impl Drop for GpuCounters {
        fn drop(&mut self) {
            unsafe {
                PdhCloseQuery(self.query);
            }
        }
    }
}
#[cfg(windows)]
pub use native::GpuCounters;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sums_processes_per_engine_and_keeps_adapters_separate() {
        let rows = vec![
            (
                "pid_1_luid_0x00000000_0x00000001_phys_0_eng_0_engtype_3d".into(),
                30.0,
            ),
            (
                "pid_2_luid_0x00000000_0x00000001_phys_0_eng_0_engtype_3d".into(),
                40.0,
            ),
            (
                "pid_1_luid_0x00000000_0x00000001_phys_0_eng_1_engtype_copy".into(),
                60.0,
            ),
            (
                "pid_1_luid_0x00000000_0x00000002_phys_0_eng_0_engtype_3d".into(),
                99.0,
            ),
        ];
        assert_eq!(
            busiest_engine(&rows, "luid_0x00000000_0x00000001"),
            Some(70.0)
        );
        assert_eq!(busiest_engine(&rows, "missing"), None);
        assert_eq!(
            busiest_engine(&[("luid_x_engine".into(), f64::NAN)], "luid_x"),
            None
        );
        assert_eq!(
            adapter_bytes(&[("luid_x_phys_0".into(), 1048576.0)], "luid_x"),
            Some(1048576.0)
        );
    }
    #[test]
    fn vendor_ids_cover_rtx_radeon_and_arc() {
        assert_eq!(vendor_name(0x10de), "NVIDIA");
        assert_eq!(vendor_name(0x1002), "AMD");
        assert_eq!(vendor_name(0x8086), "Intel");
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "Requires a physical GPU exposing WDDM counters; run explicitly on hardware"]
    fn actual_dxgi_and_wddm_counters() {
        let mut counters = GpuCounters::new().unwrap();
        assert!(!counters.adapters.is_empty());
        std::thread::sleep(std::time::Duration::from_secs(1));
        let metrics = counters.sample();
        assert!(
            metrics
                .iter()
                .any(|m| m.source == "DXGI" && m.value.is_some())
        );
        assert!(
            metrics
                .iter()
                .any(|m| m.source == "Windows PDH / WDDM" && m.value.is_some()),
            "Driver host must expose at least one real GPU counter"
        );
    }
}
