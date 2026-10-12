use rigpulse_core::DeviceDriverInfo;
#[cfg(windows)]
pub fn enumerate() -> anyhow::Result<Vec<DeviceDriverInfo>> {
    use windows::{
        Win32::{
            Devices::{DeviceAndDriverInstallation::*, Properties::*},
            Foundation::{DEVPROPKEY, ERROR_NO_MORE_ITEMS},
        },
        core::PCWSTR,
    };
    struct DeviceSet(HDEVINFO);
    impl Drop for DeviceSet {
        fn drop(&mut self) {
            unsafe {
                let _ = SetupDiDestroyDeviceInfoList(self.0);
            }
        }
    }
    fn decode(buf: &[u8]) -> String {
        let wide: Vec<u16> = buf
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        String::from_utf16_lossy(&wide)
            .trim_end_matches('\0')
            .to_owned()
    }
    unsafe fn property(
        set: HDEVINFO,
        info: &SP_DEVINFO_DATA,
        key: SETUP_DI_REGISTRY_PROPERTY,
    ) -> Option<String> {
        let mut size = 0;
        unsafe {
            let _ = SetupDiGetDeviceRegistryPropertyW(set, info, key, None, None, Some(&mut size));
        }
        if size == 0 || size > 1048576 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        unsafe {
            SetupDiGetDeviceRegistryPropertyW(set, info, key, None, Some(&mut buf), None).ok()?;
        }
        Some(decode(&buf))
    }
    unsafe fn driver_property(
        set: HDEVINFO,
        info: &SP_DEVINFO_DATA,
        key: &DEVPROPKEY,
    ) -> Option<String> {
        let mut kind = DEVPROPTYPE::default();
        let mut size = 0;
        unsafe {
            let _ = SetupDiGetDevicePropertyW(set, info, key, &mut kind, None, Some(&mut size), 0);
        }
        if size == 0 || size > 1048576 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        unsafe {
            SetupDiGetDevicePropertyW(set, info, key, &mut kind, Some(&mut buf), None, 0).ok()?;
        }
        if kind == DEVPROP_TYPE_STRING {
            Some(decode(&buf))
        } else if kind == DEVPROP_TYPE_FILETIME && buf.len() == 8 {
            let ticks = u64::from_le_bytes(buf.try_into().ok()?);
            let seconds = i64::try_from(ticks / 10000000)
                .ok()?
                .checked_sub(11644473600)?;
            chrono::DateTime::from_timestamp(seconds, 0).map(|d| d.format("%Y-%m-%d").to_string())
        } else {
            None
        }
    }
    unsafe {
        let set = DeviceSet(SetupDiGetClassDevsW(
            None,
            PCWSTR::null(),
            None,
            DIGCF_ALLCLASSES | DIGCF_PRESENT,
        )?);
        let mut devices = Vec::new();
        for n in 0..65536 {
            let mut info = SP_DEVINFO_DATA {
                cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
                ..Default::default()
            };
            if let Err(e) = SetupDiEnumDeviceInfo(set.0, n, &mut info) {
                if e.code() == ERROR_NO_MORE_ITEMS.to_hresult() {
                    break;
                }
                return Err(e.into());
            }
            let mut flags = CM_DEVNODE_STATUS_FLAGS::default();
            let mut problem = CM_PROB::default();
            let status_result = CM_Get_DevNode_Status(&mut flags, &mut problem, info.DevInst, 0);
            let problem = if status_result == CR_SUCCESS {
                Some(problem.0)
            } else {
                None
            };
            let mut id_size = 0;
            let _ = SetupDiGetDeviceInstanceIdW(set.0, &info, None, Some(&mut id_size));
            let mut id = vec![0u16; (id_size as usize).min(65536)];
            let instance_id = if !id.is_empty()
                && SetupDiGetDeviceInstanceIdW(set.0, &info, Some(&mut id), None).is_ok()
            {
                String::from_utf16_lossy(&id).trim_end_matches('\0').into()
            } else {
                String::new()
            };
            devices.push(DeviceDriverInfo {
                name: property(set.0, &info, SPDRP_FRIENDLYNAME)
                    .or_else(|| property(set.0, &info, SPDRP_DEVICEDESC))
                    .unwrap_or_else(|| "Perangkat tanpa nama".into()),
                class: property(set.0, &info, SPDRP_CLASS).unwrap_or_else(|| "Unknown".into()),
                manufacturer: property(set.0, &info, SPDRP_MFG),
                provider: driver_property(set.0, &info, &DEVPKEY_Device_DriverProvider),
                version: driver_property(set.0, &info, &DEVPKEY_Device_DriverVersion),
                date: driver_property(set.0, &info, &DEVPKEY_Device_DriverDate),
                status: rigpulse_core::device_status(problem, flags.0 & DN_STARTED.0 != 0),
                problem,
                instance_id,
                hardware_ids: property(set.0, &info, SPDRP_HARDWAREID)
                    .map(|v| {
                        v.split('\0')
                            .filter(|s| !s.is_empty())
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default(),
            });
        }
        devices.sort_by(|a, b| a.class.cmp(&b.class).then(a.name.cmp(&b.name)));
        Ok(devices)
    }
}
#[cfg(not(windows))]
pub fn enumerate() -> anyhow::Result<Vec<DeviceDriverInfo>> {
    anyhow::bail!("SetupAPI hanya tersedia pada Windows")
}
#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn real_setup_api() {
        let d = super::enumerate().expect("SetupAPI");
        assert!(!d.is_empty());
        assert!(d.iter().all(|d| !d.name.is_empty()));
        assert!(d.iter().all(|d| d.private(false).instance_id.is_empty()));
    }
}
