use crate::Adapter;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use windows::Win32::{
    NetworkManagement::{IpHelper::*, Ndis::*},
    Networking::WinSock::*,
};

unsafe fn address(a: SOCKET_ADDRESS) -> Option<String> {
    if a.lpSockaddr.is_null() {
        return None;
    }
    unsafe {
        match (*a.lpSockaddr).sa_family {
            AF_INET if a.iSockaddrLength as usize >= size_of::<SOCKADDR_IN>() => {
                let v = &*a.lpSockaddr.cast::<SOCKADDR_IN>();
                Some(Ipv4Addr::from(v.sin_addr.S_un.S_addr.to_ne_bytes()).to_string())
            }
            AF_INET6 if a.iSockaddrLength as usize >= size_of::<SOCKADDR_IN6>() => {
                let v = &*a.lpSockaddr.cast::<SOCKADDR_IN6>();
                Some(Ipv6Addr::from(v.sin6_addr.u.Byte).to_string())
            }
            _ => None,
        }
    }
}
pub fn adapters() -> anyhow::Result<Vec<Adapter>> {
    unsafe {
        let mut bytes = 16_384u32;
        for _ in 0..3 {
            anyhow::ensure!(bytes <= 1_048_576, "Daftar adaptor terlalu besar");
            let mut buffer = vec![0u64; (bytes as usize).div_ceil(8)];
            let head = buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
            let code = GetAdaptersAddresses(
                AF_UNSPEC.0 as u32,
                GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST,
                None,
                Some(head),
                &mut bytes,
            );
            if code == 111 {
                continue;
            }
            anyhow::ensure!(code == 0, "GetAdaptersAddresses: {code}");
            let mut result = Vec::new();
            let mut p = head;
            while !p.is_null() && result.len() < 256 {
                let a = &*p;
                let mut addresses = Vec::new();
                let mut q = a.FirstUnicastAddress;
                while !q.is_null() && addresses.len() < 64 {
                    if let Some(ip) = address((*q).Address) {
                        addresses.push(ip);
                    }
                    q = (*q).Next;
                }
                let mut dns = Vec::new();
                let mut q = a.FirstDnsServerAddress;
                while !q.is_null() && dns.len() < 64 {
                    if let Some(ip) = address((*q).Address) {
                        dns.push(ip);
                    }
                    q = (*q).Next;
                }
                let mut gateways = Vec::new();
                let mut q = a.FirstGatewayAddress;
                while !q.is_null() && gateways.len() < 64 {
                    if let Some(ip) = address((*q).Address) {
                        gateways.push(ip);
                    }
                    q = (*q).Next;
                }
                if a.IfType != 24 {
                    result.push(Adapter {
                        id: a.Luid.Value,
                        index: a.Anonymous1.Anonymous.IfIndex,
                        name: a.FriendlyName.to_string().unwrap_or_default(),
                        description: a.Description.to_string().unwrap_or_default(),
                        kind: match a.IfType {
                            71 => "Wi-Fi",
                            6 => "Ethernet",
                            23 => "PPP / VPN",
                            131 => "Tunnel / VPN",
                            _ => "Adaptor jaringan",
                        }
                        .into(),
                        connected: a.OperStatus == IfOperStatusUp,
                        link_down: a.ReceiveLinkSpeed,
                        link_up: a.TransmitLinkSpeed,
                        addresses,
                        dns,
                        gateways,
                        mac: a.PhysicalAddress[..(a.PhysicalAddressLength as usize).min(8)]
                            .iter()
                            .map(|b| format!("{b:02X}"))
                            .collect::<Vec<_>>()
                            .join(":"),
                    });
                }
                p = a.Next;
            }
            return Ok(result);
        }
    }
    anyhow::bail!("Daftar adaptor berubah saat dibaca; segarkan kembali")
}
pub fn counters(id: u64) -> anyhow::Result<(u64, u64)> {
    unsafe {
        let mut row = MIB_IF_ROW2 {
            InterfaceLuid: NET_LUID_LH { Value: id },
            ..Default::default()
        };
        let status = GetIfEntry2(&mut row);
        anyhow::ensure!(status.0 == 0, "GetIfEntry2: {}", status.0);
        anyhow::ensure!(row.OperStatus == IfOperStatusUp, "Adaptor terputus");
        Ok((row.InOctets, row.OutOctets))
    }
}
pub fn default_index() -> Option<u32> {
    unsafe {
        let mut dest = SOCKADDR_IN {
            sin_family: AF_INET,
            ..Default::default()
        };
        dest.sin_addr.S_un.S_addr = u32::from_ne_bytes([1, 1, 1, 1]);
        let mut index = 0;
        (GetBestInterfaceEx((&dest as *const SOCKADDR_IN).cast(), &mut index) == 0).then_some(index)
    }
}
pub fn connectivity() -> String {
    unsafe {
        let mut hint = NL_NETWORK_CONNECTIVITY_HINT::default();
        if GetNetworkConnectivityHint(&mut hint).0 != 0 {
            return "Status internet belum tersedia".into();
        }
        match hint.ConnectivityLevel.0 {
            3 => "Windows melaporkan akses internet",
            2 => "Akses lokal / terbatas",
            1 => "Tidak terhubung",
            _ => "Status internet belum diketahui",
        }
        .into()
    }
}
struct IcmpHandle(windows::Win32::Foundation::HANDLE);
pub fn wifi(id: u64) -> anyhow::Result<(String, u32)> {
    use windows::Win32::{Foundation::HANDLE, NetworkManagement::WiFi::*};
    struct Client(HANDLE);
    impl Drop for Client {
        fn drop(&mut self) {
            unsafe {
                WlanCloseHandle(self.0, None);
            }
        }
    }
    struct Memory(*mut core::ffi::c_void);
    impl Drop for Memory {
        fn drop(&mut self) {
            unsafe {
                if !self.0.is_null() {
                    WlanFreeMemory(self.0);
                }
            }
        }
    }
    unsafe {
        let mut version = 0;
        let mut handle = HANDLE::default();
        let status = WlanOpenHandle(2, None, &mut version, &mut handle);
        anyhow::ensure!(status == 0, "Native Wi-Fi tidak tersedia: {status}");
        let client = Client(handle);
        let mut guid = windows::core::GUID::zeroed();
        let status = ConvertInterfaceLuidToGuid(&NET_LUID_LH { Value: id }, &mut guid);
        anyhow::ensure!(status.0 == 0, "GUID adaptor tidak tersedia");
        let mut size = 0;
        let mut pointer = std::ptr::null_mut();
        let status = WlanQueryInterface(
            client.0,
            &guid,
            wlan_intf_opcode_current_connection,
            None,
            &mut size,
            &mut pointer,
            None,
        );
        let memory = Memory(pointer);
        anyhow::ensure!(
            status == 0,
            "SSID / sinyal tidak tersedia (Windows {status}). Izin lokasi atau kebijakan Windows dapat membatasi pembacaan."
        );
        anyhow::ensure!(
            !memory.0.is_null() && size as usize >= size_of::<WLAN_CONNECTION_ATTRIBUTES>(),
            "Respons Wi-Fi tidak lengkap"
        );
        let data = &*memory.0.cast::<WLAN_CONNECTION_ATTRIBUTES>();
        let a = &data.wlanAssociationAttributes;
        let ssid = &a.dot11Ssid;
        anyhow::ensure!(
            ssid.uSSIDLength <= 32 && a.wlanSignalQuality <= 100,
            "Data Wi-Fi tidak valid"
        );
        Ok((
            String::from_utf8_lossy(&ssid.ucSSID[..ssid.uSSIDLength as usize]).into_owned(),
            a.wlanSignalQuality,
        ))
    }
}
impl Drop for IcmpHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = IcmpCloseHandle(self.0);
        }
    }
}
pub fn echo(ip: IpAddr) -> anyhow::Result<f64> {
    unsafe {
        let handle = IcmpHandle(match ip {
            IpAddr::V4(_) => IcmpCreateFile()?,
            IpAddr::V6(_) => Icmp6CreateFile()?,
        });
        let request = b"RigPulse ICMP measurement";
        let mut reply = vec![0usize; 128];
        let size = (reply.len() * size_of::<usize>()) as u32;
        let (count, status, ms) = match ip {
            IpAddr::V4(ip) => {
                let count = IcmpSendEcho(
                    handle.0,
                    u32::from_ne_bytes(ip.octets()),
                    request.as_ptr().cast(),
                    request.len() as u16,
                    None,
                    reply.as_mut_ptr().cast(),
                    size,
                    1000,
                );
                let r = &*reply.as_ptr().cast::<ICMP_ECHO_REPLY>();
                (count, r.Status, r.RoundTripTime)
            }
            IpAddr::V6(ip) => {
                let source = SOCKADDR_IN6 {
                    sin6_family: AF_INET6,
                    ..Default::default()
                };
                let mut destination = source;
                destination.sin6_addr.u.Byte = ip.octets();
                let count = Icmp6SendEcho2(
                    handle.0,
                    None,
                    None,
                    None,
                    &source,
                    &destination,
                    request.as_ptr().cast(),
                    request.len() as u16,
                    None,
                    reply.as_mut_ptr().cast(),
                    size,
                    1000,
                );
                let r = &*reply.as_ptr().cast::<ICMPV6_ECHO_REPLY_LH>();
                (count, r.Status, r.RoundTripTime)
            }
        };
        anyhow::ensure!(
            count > 0,
            "ICMP timeout / Windows error {}",
            windows::Win32::Foundation::GetLastError().0
        );
        anyhow::ensure!(status == 0, "ICMP status {status}");
        Ok(ms as f64)
    }
}
#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "Read-only native adapter and loopback ICMP check"]
    fn native_network() {
        let adapters = super::adapters().unwrap();
        assert!(!adapters.is_empty());
        for a in adapters.iter().filter(|a| a.connected) {
            super::counters(a.id).unwrap();
        }
        super::echo("127.0.0.1".parse().unwrap()).unwrap();
        super::echo("::1".parse().unwrap()).unwrap();
    }
}
