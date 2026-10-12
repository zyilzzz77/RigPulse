use chrono::Utc;
use rigpulse_core::{HardwareSection, Inventory, Property};

#[cfg(windows)]
pub fn collect() -> anyhow::Result<Inventory> {
    use serde_json::Value;
    use std::collections::BTreeMap;
    let connection = wmi::WMIConnection::new()?;
    let queries = [
        (
            "Perangkat",
            "Win32_ComputerSystem",
            "Manufacturer,Model,SystemType,TotalPhysicalMemory",
        ),
        (
            "CPU",
            "Win32_Processor",
            "Name,Manufacturer,NumberOfCores,NumberOfLogicalProcessors,MaxClockSpeed,CurrentClockSpeed",
        ),
        (
            "GPU & Display",
            "Win32_VideoController",
            "Name,VideoProcessor,DriverVersion,CurrentHorizontalResolution,CurrentVerticalResolution,CurrentRefreshRate",
        ),
        (
            "Modul RAM",
            "Win32_PhysicalMemory",
            "Manufacturer,PartNumber,Capacity,Speed,ConfiguredClockSpeed,DeviceLocator",
        ),
        (
            "Motherboard",
            "Win32_BaseBoard",
            "Manufacturer,Product,Version,SerialNumber",
        ),
        (
            "BIOS",
            "Win32_BIOS",
            "Manufacturer,SMBIOSBIOSVersion,ReleaseDate,SerialNumber",
        ),
        (
            "Chassis",
            "Win32_SystemEnclosure",
            "Manufacturer,ChassisTypes",
        ),
        (
            "Storage fisik",
            "Win32_DiskDrive",
            "Model,Size,InterfaceType,MediaType,FirmwareRevision",
        ),
        (
            "Jaringan",
            "Win32_NetworkAdapter",
            "Name,Manufacturer,NetConnectionStatus,PhysicalAdapter",
        ),
        ("Audio", "Win32_SoundDevice", "Name,Manufacturer,Status"),
        (
            "Windows",
            "Win32_OperatingSystem",
            "Caption,Version,BuildNumber,OSArchitecture,LastBootUpTime",
        ),
    ];
    let mut sections = Vec::new();
    for (name, class, fields) in queries {
        let mut section = HardwareSection {
            name: name.into(),
            source: format!("WMI {class}"),
            properties: Vec::new(),
            error: None,
        };
        match connection
            .raw_query::<BTreeMap<String, Value>>(format!("SELECT {fields} FROM {class}"))
        {
            Ok(rows) => {
                if rows.is_empty() {
                    section.error = Some("Tidak ada data yang dilaporkan provider Windows.".into());
                }
                let multiple = rows.len() > 1;
                for (index, row) in rows.into_iter().enumerate() {
                    let mut properties: Vec<_> = row.into_iter().collect();
                    properties.sort_by_key(|(k, _)| match k.as_str() {
                        "Name" | "Caption" => 0,
                        "Manufacturer" => 1,
                        "Model" | "Product" => 2,
                        "NumberOfCores" => 3,
                        "NumberOfLogicalProcessors" => 4,
                        "Capacity" | "TotalPhysicalMemory" | "Size" => 5,
                        _ => 10,
                    });
                    for (key, value) in properties {
                        let value = match value {
                            Value::Null => "Tidak tersedia".into(),
                            Value::String(s) if s.trim().is_empty() => "Tidak tersedia".into(),
                            Value::String(s) => s.trim().into(),
                            v => v.to_string(),
                        };
                        let sensitive = ["SerialNumber", "UUID", "MACAddress", "PNPDeviceID"]
                            .contains(&key.as_str());
                        let (label, value) = crate::friendly::property(&key, &value);
                        section.properties.push(Property {
                            name: if multiple {
                                format!("{} · {label}", index + 1)
                            } else {
                                label
                            },
                            value,
                            sensitive,
                        });
                    }
                }
            }
            Err(e) => section.error = Some(format!("Provider gagal: {e}")),
        }
        sections.push(section);
    }
    let mut gpu_section = HardwareSection {
        name: "GPU & memori grafis".into(),
        source: "Windows DXGI (semua vendor)".into(),
        properties: Vec::new(),
        error: None,
    };
    match crate::gpu::enumerate() {
        Ok(adapters) if !adapters.is_empty() => {
            for (n, a) in adapters.iter().enumerate() {
                for (label, value) in [
                    ("Model GPU", a.name.clone()),
                    ("Vendor", a.vendor.clone()),
                    (
                        "Memori dedicated",
                        format!("{:.2} GiB", a.dedicated_bytes as f64 / 1073741824.0),
                    ),
                    (
                        "Batas memori shared",
                        format!(
                            "{:.2} GiB (berbagi RAM; bukan VRAM tambahan)",
                            a.shared_limit_bytes as f64 / 1073741824.0
                        ),
                    ),
                ] {
                    gpu_section.properties.push(Property {
                        name: format!("GPU {} · {label}", n + 1),
                        value,
                        sensitive: false,
                    });
                }
            }
        }
        Ok(_) => {
            gpu_section.error = Some("Tidak ada adaptor GPU fisik yang dilaporkan DXGI".into())
        }
        Err(e) => gpu_section.error = Some(e.to_string()),
    }
    sections.push(gpu_section);
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    system.refresh_cpu_all();
    let mut basics = HardwareSection {
        name: "Memori & volume".into(),
        source: "sysinfo / Windows".into(),
        properties: vec![
            Property {
                name: "RAM terpasang (GiB)".into(),
                value: format!("{:.2}", system.total_memory() as f64 / 1073741824.0),
                sensitive: false,
            },
            Property {
                name: "RAM tersedia (GiB)".into(),
                value: format!("{:.2}", system.available_memory() as f64 / 1073741824.0),
                sensitive: false,
            },
            Property {
                name: "Uptime (detik)".into(),
                value: sysinfo::System::uptime().to_string(),
                sensitive: false,
            },
        ],
        error: None,
    };
    for disk in sysinfo::Disks::new_with_refreshed_list().iter() {
        basics.properties.push(Property {
            name: disk.mount_point().display().to_string(),
            value: format!(
                "{:.1} GiB total / {:.1} GiB kosong ({:?})",
                disk.total_space() as f64 / 1073741824.0,
                disk.available_space() as f64 / 1073741824.0,
                disk.kind()
            ),
            sensitive: false,
        });
    }
    sections.push(basics);
    let mut firmware = windows::Win32::System::SystemInformation::FIRMWARE_TYPE::default();
    let mode = unsafe { windows::Win32::System::SystemInformation::GetFirmwareType(&mut firmware) };
    sections.push(HardwareSection {
        name: "Firmware".into(),
        source: "GetFirmwareType".into(),
        properties: vec![Property {
            name: "Mode boot".into(),
            value: if mode.is_err() {
                "Tidak tersedia"
            } else if firmware == windows::Win32::System::SystemInformation::FirmwareTypeUefi {
                "UEFI"
            } else if firmware == windows::Win32::System::SystemInformation::FirmwareTypeBios {
                "BIOS"
            } else {
                "Tidak tersedia"
            }
            .into(),
            sensitive: false,
        }],
        error: mode.err().map(|e| e.to_string()),
    });
    Ok(Inventory {
        at: Utc::now(),
        sections,
    })
}
#[cfg(not(windows))]
pub fn collect() -> anyhow::Result<Inventory> {
    anyhow::bail!("Provider hanya mendukung Windows")
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn real_wmi_inventory() {
        let i = super::collect().expect("WMI connection on Windows");
        assert!(
            i.sections
                .iter()
                .any(|s| s.name == "CPU" && !s.properties.is_empty())
        );
        assert!(!i.specifications(false).contains("SerialNumber"));
    }
}
