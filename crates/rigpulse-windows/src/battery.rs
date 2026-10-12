use rigpulse_core::BatteryReading;
#[cfg(windows)]
pub fn read() -> anyhow::Result<BatteryReading> {
    use windows::Win32::System::Power::*;
    let mut status = SYSTEM_POWER_STATUS::default();
    unsafe {
        GetSystemPowerStatus(&mut status)?;
    }
    let present = if status.BatteryFlag == 255 {
        None
    } else {
        Some(status.BatteryFlag & 128 == 0)
    };
    let (packs, note) = if present == Some(false) {
        (Vec::new(), None)
    } else {
        match packs() {
            Ok(p) => (p, None),
            Err(e) => (Vec::new(), Some(format!("Kapasitas tidak tersedia: {e}"))),
        }
    };
    Ok(BatteryReading {
        at: chrono::Utc::now(),
        present,
        percent: if present == Some(true) && status.BatteryLifePercent <= 100 {
            Some(status.BatteryLifePercent)
        } else {
            None
        },
        ac: match status.ACLineStatus {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        },
        charging: if present == Some(true) {
            Some(status.BatteryFlag & 8 != 0)
        } else {
            None
        },
        remaining_seconds: if present == Some(true) && status.BatteryLifeTime != u32::MAX {
            Some(status.BatteryLifeTime)
        } else {
            None
        },
        packs,
        note,
    })
}

#[cfg(windows)]
fn packs() -> anyhow::Result<Vec<rigpulse_core::BatteryPack>> {
    use windows::{
        Win32::{
            Devices::DeviceAndDriverInstallation::*,
            Foundation::{CloseHandle, ERROR_NO_MORE_ITEMS, GENERIC_READ, HANDLE},
            Storage::FileSystem::*,
            System::{IO::DeviceIoControl, Power::*},
        },
        core::{GUID, PCWSTR},
    };
    const GUID_BATTERY: GUID = GUID::from_u128(0x72631e54_78a4_11d0_bcf7_00aa00b7b32a);
    struct DeviceSet(HDEVINFO);
    impl Drop for DeviceSet {
        fn drop(&mut self) {
            unsafe {
                let _ = SetupDiDestroyDeviceInfoList(self.0);
            }
        }
    }
    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    // Every input/output pointer below refers to a sized, initialized Rust value. The interface
    // detail buffer uses usize storage to provide the alignment Windows requires on x64.
    unsafe {
        let set = DeviceSet(SetupDiGetClassDevsW(
            Some(&GUID_BATTERY),
            PCWSTR::null(),
            None,
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )?);
        let mut result = Vec::new();
        for index in 0..64 {
            let mut interface = SP_DEVICE_INTERFACE_DATA {
                cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                ..Default::default()
            };
            if let Err(e) =
                SetupDiEnumDeviceInterfaces(set.0, None, &GUID_BATTERY, index, &mut interface)
            {
                if e.code() == ERROR_NO_MORE_ITEMS.to_hresult() {
                    break;
                }
                return Err(e.into());
            }
            let mut needed = 0;
            let _ = SetupDiGetDeviceInterfaceDetailW(
                set.0,
                &interface,
                None,
                0,
                Some(&mut needed),
                None,
            );
            anyhow::ensure!(
                (8..=1048576).contains(&needed),
                "Ukuran data baterai tidak valid"
            );
            let mut buffer = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
            let detail = buffer
                .as_mut_ptr()
                .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
            (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            SetupDiGetDeviceInterfaceDetailW(set.0, &interface, Some(detail), needed, None, None)?;
            let handle = Handle(CreateFileW(
                PCWSTR(std::ptr::addr_of!((*detail).DevicePath).cast()),
                GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )?);
            let wait = 0u32;
            let mut tag = 0u32;
            let mut returned = 0;
            DeviceIoControl(
                handle.0,
                IOCTL_BATTERY_QUERY_TAG,
                Some((&wait as *const u32).cast()),
                4,
                Some((&mut tag as *mut u32).cast()),
                4,
                Some(&mut returned),
                None,
            )?;
            if tag == 0 || returned != 4 {
                continue;
            }
            let query = BATTERY_QUERY_INFORMATION {
                BatteryTag: tag,
                InformationLevel: BatteryInformation,
                AtRate: 0,
            };
            let mut info = BATTERY_INFORMATION::default();
            DeviceIoControl(
                handle.0,
                IOCTL_BATTERY_QUERY_INFORMATION,
                Some((&query as *const BATTERY_QUERY_INFORMATION).cast()),
                std::mem::size_of_val(&query) as u32,
                Some((&mut info as *mut BATTERY_INFORMATION).cast()),
                std::mem::size_of_val(&info) as u32,
                Some(&mut returned),
                None,
            )?;
            anyhow::ensure!(
                returned as usize >= std::mem::size_of::<BATTERY_INFORMATION>(),
                "Kapasitas baterai tidak lengkap"
            );
            if info.Capabilities & BATTERY_SYSTEM_BATTERY == 0 {
                continue;
            }
            let valid = |n: u32| {
                if n > 0 && n != BATTERY_UNKNOWN_CAPACITY {
                    Some(n)
                } else {
                    None
                }
            };
            let relative = info.Capabilities & BATTERY_CAPACITY_RELATIVE != 0;
            use std::{
                collections::HashMap,
                sync::{Mutex, OnceLock},
                time::{Duration, Instant},
            };
            static UNSUPPORTED: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
            let path = PCWSTR(std::ptr::addr_of!((*detail).DevicePath).cast())
                .to_string()
                .unwrap_or_default();
            let backoff = UNSUPPORTED.get_or_init(Default::default);
            let retry = backoff
                .lock()
                .map(|mut m| {
                    m.retain(|_, at| at.elapsed() < Duration::from_secs(60));
                    !m.contains_key(&path)
                })
                .unwrap_or(false);
            let wait = BATTERY_WAIT_STATUS {
                BatteryTag: tag,
                Timeout: 0,
                PowerState: 0,
                LowCapacity: 0,
                HighCapacity: u32::MAX,
            };
            let mut status = BATTERY_STATUS::default();
            let power_valid = retry
                && !relative
                && DeviceIoControl(
                    handle.0,
                    IOCTL_BATTERY_QUERY_STATUS,
                    Some((&wait as *const BATTERY_WAIT_STATUS).cast()),
                    std::mem::size_of_val(&wait) as u32,
                    Some((&mut status as *mut BATTERY_STATUS).cast()),
                    std::mem::size_of_val(&status) as u32,
                    Some(&mut returned),
                    None,
                )
                .is_ok()
                && returned as usize >= std::mem::size_of::<BATTERY_STATUS>();
            if (!power_valid || status.Rate == i32::MIN)
                && retry
                && let Ok(mut m) = backoff.lock()
            {
                if m.len() > 64 {
                    m.clear();
                }
                m.insert(path, Instant::now());
            }
            let text = |level| -> Option<String> {
                let query = BATTERY_QUERY_INFORMATION {
                    BatteryTag: tag,
                    InformationLevel: level,
                    AtRate: 0,
                };
                let mut buffer = [0u16; 256];
                let mut returned = 0;
                if DeviceIoControl(
                    handle.0,
                    IOCTL_BATTERY_QUERY_INFORMATION,
                    Some((&query as *const BATTERY_QUERY_INFORMATION).cast()),
                    std::mem::size_of_val(&query) as u32,
                    Some(buffer.as_mut_ptr().cast()),
                    std::mem::size_of_val(&buffer) as u32,
                    Some(&mut returned),
                    None,
                )
                .is_err()
                    || returned < 2
                    || returned as usize > std::mem::size_of_val(&buffer)
                    || returned % 2 != 0
                {
                    return None;
                }
                let count = returned as usize / 2;
                let end = buffer[..count]
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(count);
                String::from_utf16(&buffer[..end])
                    .ok()
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty())
            };
            result.push(rigpulse_core::BatteryPack {
                watts: if power_valid {
                    rigpulse_core::battery_watts(status.Rate, status.PowerState, relative)
                } else {
                    None
                },
                voltage: if power_valid
                    && status.Voltage != BATTERY_UNKNOWN_VOLTAGE
                    && status.Voltage > 0
                {
                    Some(f64::from(status.Voltage) / 1000.0)
                } else {
                    None
                },
                chemistry: String::from_utf8(info.Chemistry.to_vec())
                    .ok()
                    .map(|s| s.trim_matches(char::from(0)).trim().to_string())
                    .filter(|s| !s.is_empty()),
                power_state: power_valid.then_some(status.PowerState),
                manufacturer: text(BatteryManufactureName),
                model: text(BatteryDeviceName),
                design_mwh: if relative {
                    None
                } else {
                    valid(info.DesignedCapacity)
                },
                full_mwh: if relative {
                    None
                } else {
                    valid(info.FullChargedCapacity)
                },
                cycles: (info.CycleCount != u32::MAX).then_some(info.CycleCount),
                relative,
            });
        }
        Ok(result)
    }
}
#[cfg(not(windows))]
pub fn read() -> anyhow::Result<BatteryReading> {
    anyhow::bail!("Provider hanya mendukung Windows")
}
#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn real_power_status() {
        let b = super::read().expect("Windows power status");
        assert!(b.percent.is_none_or(|v| v <= 100));
        if b.present == Some(false) {
            assert!(b.percent.is_none());
            assert!(b.packs.is_empty());
        }
    }
}
