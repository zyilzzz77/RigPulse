use crate::{Cancellation, check_cancel};
use std::{
    io::Read,
    net::{IpAddr, ToSocketAddrs},
    time::Duration,
};

#[derive(Default, Clone, Debug)]
pub struct PingResult {
    pub target: String,
    pub address: String,
    pub sent: usize,
    pub received: usize,
    pub min: Option<f64>,
    pub average: Option<f64>,
    pub max: Option<f64>,
    pub jitter: Option<f64>,
    pub loss: Option<f64>,
    pub samples: Vec<Option<f64>>,
    pub at: Option<chrono::DateTime<chrono::Utc>>,
    pub errors: Vec<String>,
    pub cancelled: bool,
}
pub fn ping_statistics(target: String, address: String, samples: &[Option<f64>]) -> PingResult {
    let values: Vec<f64> = samples.iter().flatten().copied().collect();
    let differences: Vec<f64> = samples
        .windows(2)
        .filter_map(|w| Some((w[1]? - w[0]?).abs()))
        .collect();
    PingResult {
        target,
        address,
        sent: samples.len(),
        received: values.len(),
        min: values.iter().copied().reduce(f64::min),
        max: values.iter().copied().reduce(f64::max),
        average: (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64),
        jitter: (!differences.is_empty())
            .then(|| differences.iter().sum::<f64>() / differences.len() as f64),
        loss: (!samples.is_empty())
            .then(|| 100.0 * (samples.len() - values.len()) as f64 / samples.len() as f64),
        samples: samples.to_vec(),
        at: Some(chrono::Utc::now()),
        ..Default::default()
    }
}
pub fn valid_target(target: &str) -> bool {
    target.parse::<IpAddr>().is_ok()
        || (!target.is_empty()
            && target.len() <= 253
            && target.split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && label
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    && !label.starts_with('-')
                    && !label.ends_with('-')
            }))
}
pub fn ping(
    target: &str,
    cancel: &Cancellation,
    progress: impl Fn(String),
) -> anyhow::Result<PingResult> {
    anyhow::ensure!(
        valid_target(target),
        "Masukkan alamat IP atau nama host tanpa URL/port"
    );
    check_cancel(cancel)?;
    let address = if let Ok(ip) = target.parse::<IpAddr>() {
        ip
    } else {
        resolve(target, cancel)?
    };
    let mut samples = Vec::new();
    let mut errors = Vec::new();
    for n in 0..10 {
        if check_cancel(cancel).is_err() {
            break;
        }
        progress(format!("ICMP {}/10 → {address}", n + 1));
        match crate::native::echo(address) {
            Ok(ms) => samples.push(Some(ms)),
            Err(e) => {
                samples.push(None);
                errors.push(e.to_string());
            }
        }
        for _ in 0..5 {
            if check_cancel(cancel).is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    let mut result = ping_statistics(target.into(), address.to_string(), &samples);
    result.errors = errors;
    result.cancelled = check_cancel(cancel).is_err();
    Ok(result)
}
fn resolve(target: &str, cancel: &Cancellation) -> anyhow::Result<IpAddr> {
    use std::sync::atomic::{AtomicBool, Ordering};
    static RESOLVING: AtomicBool = AtomicBool::new(false);
    anyhow::ensure!(
        !RESOLVING.swap(true, Ordering::AcqRel),
        "DNS Windows masih menyelesaikan permintaan sebelumnya; coba kembali sebentar lagi"
    );
    let target = target.to_owned();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = (target.as_str(), 0)
            .to_socket_addrs()
            .map_err(|e| e.to_string())
            .and_then(|mut a| {
                a.next()
                    .map(|a| a.ip())
                    .ok_or_else(|| "DNS tidak menemukan alamat".into())
            });
        RESOLVING.store(false, Ordering::Release);
        let _ = tx.send(result);
    });
    let start = std::time::Instant::now();
    loop {
        check_cancel(cancel)?;
        anyhow::ensure!(
            start.elapsed().as_secs() < 5,
            "Resolusi DNS melewati batas 5 detik"
        );
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(r) => return r.map_err(anyhow::Error::msg),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => anyhow::bail!("Worker DNS berhenti"),
        }
    }
}
fn client() -> anyhow::Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("RigPulse/", env!("CARGO_PKG_VERSION")))
        .build()?)
}
fn limited_json(response: reqwest::blocking::Response) -> anyhow::Result<serde_json::Value> {
    let response = response.error_for_status()?;
    let mut bytes = Vec::new();
    response.take(65_537).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= 65_536, "Respons penyedia terlalu besar");
    Ok(serde_json::from_slice(&bytes)?)
}
#[derive(Clone, Debug)]
pub struct PublicInfo {
    pub ip: String,
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
    pub organization: Option<String>,
    pub isp: Option<String>,
    pub asn: Option<String>,
    pub asn_org: Option<String>,
    pub prefix: Option<String>,
    pub network_name: Option<String>,
    pub registry: Option<String>,
    pub location: Option<String>,
    pub at: chrono::DateTime<chrono::Utc>,
    pub warning: Option<String>,
}

/// Autonomous System information sourced from RIPEstat.
#[derive(Clone, Debug, Default)]
pub struct AsnInfo {
    pub asns: Vec<String>,
    pub holder: Option<String>,
    pub prefix: Option<String>,
    pub source: String,
    pub at: chrono::DateTime<chrono::Utc>,
}

/// Normalized registration (WHOIS/RDAP) data. All fields are optional:
/// missing registry data is reported as unavailable, never invented.
#[derive(Clone, Debug, Default)]
pub struct WhoisBundle {
    pub query: String,
    pub ip: Option<String>,
    pub network_name: Option<String>,
    pub organization: Option<String>,
    pub range: Option<String>,
    pub cidr: Option<String>,
    pub registry: Option<String>,
    pub country: Option<String>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub asn: Option<String>,
    pub as_org: Option<String>,
    pub prefix: Option<String>,
    pub whois_source: Option<String>,
    pub rdap_source: Option<String>,
    pub at: chrono::DateTime<chrono::Utc>,
    pub warning: Option<String>,
}

/// Bytes/s measured on an adapter converted to Mbit/s (decimal SI).
pub fn bytes_to_mbps(bytes_per_sec: f64) -> f64 {
    bytes_per_sec * 8.0 / 1_000_000.0
}

/// Dynamic gauge scale maximums with hysteresis handled by the caller.
pub const GAUGE_SCALES: [f64; 10] = [
    10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0, 10_000.0,
];
pub fn gauge_scale_for(mbps: f64) -> f64 {
    let v = if mbps.is_finite() { mbps.max(0.0) } else { 0.0 };
    for s in GAUGE_SCALES {
        if v <= s {
            return s;
        }
    }
    *GAUGE_SCALES.last().unwrap()
}
/// Stable scale: only grow immediately, shrink when usage drops below 45%.
pub fn stable_gauge_scale(current: f64, mbps: f64) -> f64 {
    let needed = gauge_scale_for(mbps);
    if needed > current || current <= 0.0 {
        return needed;
    }
    if mbps < current * 0.45 {
        return needed.max(10.0);
    }
    current
}
/// Needle angle in radians for a semicircular gauge (PI..2PI).
pub fn gauge_angle(mbps: f64, max: f64) -> f64 {
    let max = if max.is_finite() && max > 0.0 {
        max
    } else {
        10.0
    };
    let frac = (if mbps.is_finite() { mbps.max(0.0) } else { 0.0 } / max).clamp(0.0, 1.0);
    std::f64::consts::PI + frac * std::f64::consts::PI
}
pub fn parse_public(ip: &str, value: &serde_json::Value) -> anyhow::Result<PublicInfo> {
    let ip = ip.parse::<IpAddr>()?.to_string();
    let text = |v: &serde_json::Value| v.as_str().filter(|s| !s.is_empty()).map(str::to_owned);
    let company = &value["company"];
    let organization = text(&value["connection"]["org"])
        .or_else(|| text(company))
        .or_else(|| text(&company["name"]));
    let isp = if let Some(isp) = text(&value["connection"]["isp"]) {
        Some(isp)
    } else if company["type"].as_str() == Some("isp") {
        text(&company["name"])
    } else {
        None
    };
    let asn = value["connection"]["asn"]
        .as_u64()
        .map(|n| format!("AS{n}"))
        .or_else(|| text(&value["asn"]))
        .or_else(|| value["asn"]["asn"].as_u64().map(|n| format!("AS{n}")));
    let location = text(&value["country"]).or_else(|| text(&value["location"]["country"]));
    Ok(PublicInfo {
        ip,
        ipv4: None,
        ipv6: None,
        organization,
        isp,
        asn,
        asn_org: None,
        prefix: None,
        network_name: None,
        registry: None,
        location,
        at: chrono::Utc::now(),
        warning: None,
    })
}
fn ipify_ip(client: &reqwest::blocking::Client, url: &str) -> Option<String> {
    limited_json(client.get(url).send().ok()?)
        .ok()?
        .get("ip")?
        .as_str()
        .and_then(|s| s.parse::<IpAddr>().ok().map(|ip| ip.to_string()))
}
fn enrich_with_asn(info: &mut PublicInfo, cancel: &Cancellation) {
    if check_cancel(cancel).is_err() {
        return;
    }
    match asn_lookup(&info.ip, cancel) {
        Ok(a) => {
            if info.asn.is_none() {
                info.asn = a.asns.first().cloned();
            }
            if info.asn_org.is_none() {
                info.asn_org = a.holder;
            }
            if info.prefix.is_none() {
                info.prefix = a.prefix;
            }
        }
        Err(e) => {
            let note = format!("ASN tidak tersedia: {e}");
            info.warning = Some(
                info.warning
                    .take()
                    .map(|w| format!("{w}; {note}"))
                    .unwrap_or(note),
            );
        }
    }
}
/// Parse RIPEstat network-info `data` object (pure, fixture-testable).
pub fn parse_ripestat_network(data: &serde_json::Value) -> (Vec<String>, Option<String>) {
    let mut asns = Vec::new();
    if let Some(list) = data.get("asns").and_then(|v| v.as_array()) {
        for entry in list {
            if let Some(n) = entry.get("asn").and_then(|v| v.as_u64()) {
                asns.push(format!("AS{n}"));
            } else if let Some(s) = entry.get("asn").and_then(|v| v.as_str()) {
                let s = s.trim().to_ascii_uppercase();
                asns.push(if s.starts_with("AS") {
                    s
                } else {
                    format!("AS{s}")
                });
            }
        }
    }
    let prefix = data
        .get("prefix")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    (asns, prefix)
}
pub fn asn_lookup(ip: &str, cancel: &Cancellation) -> anyhow::Result<AsnInfo> {
    let ip: IpAddr = ip.parse()?;
    check_cancel(cancel)?;
    let client = client()?;
    let url = format!("https://stat.ripe.net/data/network-info/data.json?resource={ip}");
    let body = limited_json(client.get(url).send()?)?;
    anyhow::ensure!(
        body.get("status").and_then(|v| v.as_str()) != Some("error"),
        "RIPEstat network-info menolak permintaan"
    );
    let data = &body["data"];
    let (asns, prefix) = parse_ripestat_network(data);
    let mut holder = None;
    if let Some(first) = asns.first() {
        check_cancel(cancel)?;
        let digits: String = first.chars().filter(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            let overview = limited_json(
                client
                    .get(format!(
                        "https://stat.ripe.net/data/as-overview/data.json?resource=AS{digits}"
                    ))
                    .send()?,
            );
            if let Ok(body) = overview {
                holder = body["data"]["holder"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned);
            }
        }
    }
    Ok(AsnInfo {
        asns,
        holder,
        prefix,
        source: "RIPEstat network-info + as-overview".into(),
        at: chrono::Utc::now(),
    })
}
fn bounded_json(
    response: reqwest::blocking::Response,
    limit: usize,
) -> anyhow::Result<serde_json::Value> {
    let response = response.error_for_status()?;
    let mut bytes = Vec::new();
    response.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= limit, "Respons registry terlalu besar");
    Ok(serde_json::from_slice(&bytes)?)
}
fn rdap_text(v: &serde_json::Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(str::to_owned)
}
/// Pick the authoritative RDAP base URL from an IANA bootstrap registry (pure).
pub fn bootstrap_service_url(bootstrap: &serde_json::Value, ip: &IpAddr) -> Option<String> {
    let services = bootstrap.get("services")?.as_array()?;
    for entry in services {
        let ranges = entry.as_array()?;
        if ranges.len() < 2 {
            continue;
        }
        let cidrs = ranges[0].as_array()?;
        for cidr in cidrs {
            if let Some(c) = cidr.as_str()
                && cidr_matches(c, ip)
            {
                let urls = ranges[1].as_array()?;
                if let Some(u) = urls.first().and_then(|u| u.as_str()) {
                    let mut base = u.to_owned();
                    if !base.ends_with('/') {
                        base.push('/');
                    }
                    if base.starts_with("https://") {
                        return Some(base);
                    }
                }
            }
        }
    }
    None
}
fn cidr_matches(cidr: &str, ip: &IpAddr) -> bool {
    let Some((net, bits)) = cidr.split_once('/') else {
        return false;
    };
    let Ok(bits) = bits.parse::<u32>() else {
        return false;
    };
    match (net.parse::<IpAddr>(), ip) {
        (Ok(IpAddr::V4(net)), IpAddr::V4(addr)) => {
            let bits = bits.min(32);
            if bits == 0 {
                return true;
            }
            let mask = u32::MAX.checked_shl(32 - bits).unwrap_or(0);
            (u32::from(net) & mask) == (u32::from(*addr) & mask)
        }
        (Ok(IpAddr::V6(net)), IpAddr::V6(addr)) => {
            let bits = bits.min(128);
            let n = u128::from(net);
            let a = u128::from(*addr);
            if bits == 0 {
                return true;
            }
            let mask = u128::MAX.checked_shl(128 - bits).unwrap_or(0);
            (n & mask) == (a & mask)
        }
        _ => false,
    }
}
/// Normalize an RDAP `ip/network` object (pure, fixture-testable).
pub fn parse_rdap_object(query: &str, body: &serde_json::Value) -> WhoisBundle {
    let entity_org = body
        .get("entities")
        .and_then(|v| v.as_array())
        .and_then(|list| {
            list.iter().find_map(|e| {
                e.get("vcardArray")
                    .and_then(|v| v.as_array())
                    .and_then(|v| v.get(1))
                    .and_then(|v| v.as_array())
                    .and_then(|props| {
                        props.iter().find_map(|p| {
                            let name = p.as_array()?.first()?.as_str()?;
                            if name == "fn" {
                                p.as_array()?
                                    .get(3)?
                                    .as_str()
                                    .filter(|s| !s.is_empty())
                                    .map(str::to_owned)
                            } else {
                                None
                            }
                        })
                    })
                    .or_else(|| rdap_text(&e["handle"]))
            })
        });
    let cidr = body
        .get("cidr0_cidrs")
        .and_then(|v| v.as_array())
        .and_then(|l| l.first())
        .and_then(|c| {
            Some(format!(
                "{}/{}",
                c.get("v4prefix")?
                    .as_str()
                    .or_else(|| c.get("v6prefix")?.as_str())?,
                c.get("length")?.as_u64()?
            ))
        });
    let event = |name: &str| {
        body.get("events").and_then(|v| v.as_array()).and_then(|l| {
            l.iter().find_map(|e| {
                if e.get("eventAction")?.as_str() == Some(name) {
                    e.get("eventDate")?.as_str().map(str::to_owned)
                } else {
                    None
                }
            })
        })
    };
    WhoisBundle {
        query: query.into(),
        ip: rdap_text(&body["handle"])
            .or_else(|| body.get("startAddress").and_then(rdap_text))
            .or_else(|| Some(query.into())),
        network_name: rdap_text(&body["name"]),
        organization: entity_org,
        range: match (
            body.get("startAddress").and_then(rdap_text),
            body.get("endAddress").and_then(rdap_text),
        ) {
            (Some(s), Some(e)) => Some(format!("{s} - {e}")),
            (Some(s), None) => Some(s),
            _ => None,
        },
        cidr,
        registry: None,
        country: rdap_text(&body["country"]),
        created: event("registration"),
        updated: event("last changed"),
        asn: None,
        as_org: None,
        prefix: None,
        whois_source: None,
        rdap_source: None,
        at: chrono::Utc::now(),
        warning: None,
    }
}
/// RDAP lookup following RFC 9224 bootstrap discovery. Returns partial data on
/// registry failure rather than failing outright; caller adds fallback context.
pub fn rdap_lookup(ip: &str, cancel: &Cancellation) -> anyhow::Result<WhoisBundle> {
    let addr: IpAddr = ip.parse()?;
    check_cancel(cancel)?;
    let client = client()?;
    let bootstrap_url = if addr.is_ipv4() {
        "https://data.iana.org/rdap/ipv4.json"
    } else {
        "https://data.iana.org/rdap/ipv6.json"
    };
    let bootstrap = bounded_json(client.get(bootstrap_url).send()?, 262_144)?;
    let base = bootstrap_service_url(&bootstrap, &addr)
        .ok_or_else(|| anyhow::anyhow!("Registry RDAP tidak ditemukan untuk alamat ini"))?;
    check_cancel(cancel)?;
    let url = format!("{base}ip/{addr}");
    let body = bounded_json(client.get(&url).send()?, 262_144)?;
    let mut bundle = parse_rdap_object(ip, &body);
    bundle.rdap_source = Some(url);
    Ok(bundle)
}
pub fn ripestat_whois(ip: &str, cancel: &Cancellation) -> anyhow::Result<WhoisBundle> {
    let addr: IpAddr = ip.parse()?;
    check_cancel(cancel)?;
    let client = client()?;
    let url = format!("https://stat.ripe.net/data/whois/data.json?resource={addr}");
    let body = limited_json(client.get(&url).send()?)?;
    let mut bundle = WhoisBundle {
        query: ip.into(),
        ip: Some(ip.into()),
        whois_source: Some(url),
        at: chrono::Utc::now(),
        ..Default::default()
    };
    if let Some(records) = body["data"]["records"].as_array() {
        for record in records {
            for detail in record["details"].as_array().cloned().unwrap_or_default() {
                let key = detail["key"].as_str().unwrap_or("").to_ascii_lowercase();
                let value = detail["value"].as_str().unwrap_or("").to_owned();
                if value.is_empty() {
                    continue;
                }
                match key.as_str() {
                    "netname" | "net-name" if bundle.network_name.is_none() => {
                        bundle.network_name = Some(value)
                    }
                    "org-name" | "orgname" | "organisation" if bundle.organization.is_none() => {
                        bundle.organization = Some(value)
                    }
                    "inetnum" | "inet6num" | "netrange" if bundle.range.is_none() => {
                        bundle.range = Some(value)
                    }
                    "cidr" if bundle.cidr.is_none() => bundle.cidr = Some(value),
                    "country" if bundle.country.is_none() => bundle.country = Some(value),
                    "created" if bundle.created.is_none() => bundle.created = Some(value),
                    "last-modified" if bundle.updated.is_none() => bundle.updated = Some(value),
                    _ => {}
                }
            }
        }
    }
    Ok(bundle)
}
fn merge_bundle(dst: &mut WhoisBundle, src: WhoisBundle) {
    macro_rules! fill {
        ($($f:ident),+) => { $( if dst.$f.is_none() { dst.$f = src.$f.clone(); } )+ };
    }
    fill!(
        network_name,
        organization,
        range,
        cidr,
        registry,
        country,
        created,
        updated,
        asn,
        as_org,
        prefix
    );
    if dst.whois_source.is_none() {
        dst.whois_source = src.whois_source.clone();
    }
    if dst.rdap_source.is_none() {
        dst.rdap_source = src.rdap_source.clone();
    }
    if let Some(w) = src.warning {
        dst.warning = Some(dst.warning.take().map(|v| format!("{v}; {w}")).unwrap_or(w));
    }
}
/// Combined WHOIS/RDAP/ASN lookup. Never invents data: unavailable providers
/// leave their fields empty and record a warning instead.
pub fn whois_bundle(query: &str, cancel: &Cancellation) -> anyhow::Result<WhoisBundle> {
    let query = query.trim();
    anyhow::ensure!(!query.is_empty(), "Masukkan IP, ASN, atau domain");
    let ip = if let Ok(ip) = query.parse::<IpAddr>() {
        ip.to_string()
    } else if let Some(asn) = query.trim().to_ascii_uppercase().strip_prefix("AS")
        && !asn.is_empty()
        && asn.chars().all(|c| c.is_ascii_digit())
    {
        anyhow::bail!("Pencarian detail ASN belum didukung; masukkan alamat IP publik");
    } else {
        anyhow::ensure!(valid_target(query), "Masukkan IP publik, bukan URL");
        anyhow::bail!("Pencarian domain RDAP belum didukung; masukkan alamat IP publik");
    };
    check_cancel(cancel)?;
    let mut bundle = WhoisBundle {
        query: ip.clone(),
        ip: Some(ip.clone()),
        at: chrono::Utc::now(),
        ..Default::default()
    };
    let mut failures = 0;
    match rdap_lookup(&ip, cancel) {
        Ok(r) => merge_bundle(&mut bundle, r),
        Err(e) => {
            failures += 1;
            bundle.warning = Some(format!("RDAP tidak tersedia: {e}"));
        }
    }
    match ripestat_whois(&ip, cancel) {
        Ok(r) => merge_bundle(&mut bundle, r),
        Err(e) => {
            failures += 1;
            let note = format!("WHOIS RIPEstat tidak tersedia: {e}");
            bundle.warning = Some(
                bundle
                    .warning
                    .take()
                    .map(|w| format!("{w}; {note}"))
                    .unwrap_or(note),
            );
        }
    }
    match asn_lookup(&ip, cancel) {
        Ok(a) => {
            if bundle.asn.is_none() {
                bundle.asn = a.asns.first().cloned();
            }
            if bundle.as_org.is_none() {
                bundle.as_org = a.holder;
            }
            if bundle.prefix.is_none() {
                bundle.prefix = a.prefix;
            }
        }
        Err(e) => {
            failures += 1;
            let note = format!("ASN tidak tersedia: {e}");
            bundle.warning = Some(
                bundle
                    .warning
                    .take()
                    .map(|w| format!("{w}; {note}"))
                    .unwrap_or(note),
            );
        }
    }
    if failures >= 3 {
        anyhow::bail!(
            "{}",
            bundle
                .warning
                .unwrap_or_else(|| "Semua penyedia lookup gagal".into())
        );
    }
    Ok(bundle)
}
/// Strict validation for the advanced lookup field: public IP literals only.
pub fn valid_lookup_input(input: &str) -> bool {
    if let Ok(ip) = input.trim().parse::<IpAddr>() {
        return !ip.is_loopback() && !ip.is_unspecified();
    }
    false
}
pub fn public_info(cancel: &Cancellation) -> anyhow::Result<PublicInfo> {
    check_cancel(cancel)?;
    let client = client()?;
    let v4 = ipify_ip(&client, "https://api.ipify.org?format=json");
    check_cancel(cancel)?;
    let v6 = ipify_ip(&client, "https://api64.ipify.org?format=json").filter(|ip| ip.contains(':'));
    let ip = v4
        .clone()
        .or(v6.clone())
        .ok_or_else(|| anyhow::anyhow!("ipify tidak mengirim alamat IP"))?;
    let mut result = parse_public(&ip, &serde_json::Value::Null)?;
    result.ipv4 = v4;
    result.ipv6 = v6;
    check_cancel(cancel)?;
    let lookup = (|| -> anyhow::Result<serde_json::Value> {
        limited_json(client.get(format!("https://ipwho.is/{ip}")).send()?)
    })();
    match lookup {
        Ok(value) => {
            let v4 = result.ipv4.clone();
            let v6 = result.ipv6.clone();
            result = parse_public(&ip, &value)?;
            result.ipv4 = v4;
            result.ipv6 = v6;
            if value.get("error").is_some() || value["success"].as_bool() == Some(false) {
                result.warning = Some("Metadata penyedia tidak tersedia / kuota tercapai".into());
            }
        }
        Err(_) => {
            result.warning =
                Some("IP publik tersedia; ISP/ASN tidak tersedia dari penyedia saat ini".into())
        }
    }
    enrich_with_asn(&mut result, cancel);
    // Cross-fill ASN org / prefix into the card without overwriting ipwho.is ISP.
    Ok(result)
}
#[derive(Clone, Debug)]
pub struct SpeedResult {
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub latency_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
    pub packet_loss: Option<f64>,
    pub server: String,
    pub bytes: Option<u64>,
    pub elapsed: f64,
    pub at: chrono::DateTime<chrono::Utc>,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ping_no_fake_zero_and_loss() {
        let r = ping_statistics(
            "x".into(),
            "x".into(),
            &[Some(10.0), Some(20.0), None, Some(40.0)],
        );
        assert_eq!(r.loss, Some(25.0));
        assert_eq!(r.jitter, Some(10.0));
        assert_eq!(r.received, 3);
        let empty = ping_statistics("".into(), "".into(), &[None, None]);
        assert_eq!(empty.average, None);
        assert_eq!(empty.loss, Some(100.0));
        assert_eq!(empty.jitter, None);
    }
    #[test]
    fn gauge_scales_angles_and_conversions() {
        assert_eq!(bytes_to_mbps(12500000.0), 100.0);
        assert_eq!(gauge_scale_for(0.0), 10.0);
        assert_eq!(gauge_scale_for(92.4), 100.0);
        assert_eq!(gauge_scale_for(1500.0), 2000.0);
        assert!(!gauge_angle(f64::NAN, 100.0).is_nan());
        assert!((gauge_angle(0.0, 100.0) - std::f64::consts::PI).abs() < 1e-9);
        assert!((gauge_angle(100.0, 100.0) - 2.0 * std::f64::consts::PI).abs() < 1e-9);
        assert_eq!(stable_gauge_scale(100.0, 200.0), 250.0);
        assert_eq!(stable_gauge_scale(100.0, 90.0), 100.0);
        assert_eq!(stable_gauge_scale(100.0, 10.0), 10.0);
        assert!(valid_lookup_input("8.8.8.8"));
        assert!(valid_lookup_input("2001:4860:4860::8888"));
        assert!(!valid_lookup_input("127.0.0.1"));
        assert!(!valid_lookup_input("https://example.com"));
    }
    #[test]
    fn ripestat_rdap_bootstrap_parsing() {
        let (asns, prefix) = parse_ripestat_network(&serde_json::json!({
            "asns": [{"asn": 4761}, {"asn": "AS17995"}],
            "prefix": "103.147.8.0/22"
        }));
        assert_eq!(asns, vec!["AS4761", "AS17995"]);
        assert_eq!(prefix.as_deref(), Some("103.147.8.0/22"));
        let bootstrap = serde_json::json!({"services": [
            [["1.0.0.0/8"], ["https://rdap.apnic.net/"]],
            [["0.0.0.0/0"], ["https://rdap.arin.net/registry/"]]
        ]});
        let ip: IpAddr = "1.1.1.1".parse().unwrap();
        assert_eq!(
            bootstrap_service_url(&bootstrap, &ip).as_deref(),
            Some("https://rdap.apnic.net/")
        );
        let bundle = parse_rdap_object(
            "1.1.1.1",
            &serde_json::json!({
                "handle": "1.1.1.0 - 1.1.1.255",
                "startAddress": "1.1.1.0",
                "endAddress": "1.1.1.255",
                "name": "APNIC-FIXTURE",
                "country": "AU",
                "cidr0_cidrs": [{"v4prefix": "1.1.1.0", "length": 24}],
                "entities": [{"vcardArray": ["vcard", [["fn", {}, "text", "Fixture Org"]]]}],
                "events": [
                    {"eventAction": "registration", "eventDate": "2011-01-01T00:00:00Z"},
                    {"eventAction": "last changed", "eventDate": "2020-01-01T00:00:00Z"}
                ]
            }),
        );
        assert_eq!(bundle.organization.as_deref(), Some("Fixture Org"));
        assert_eq!(bundle.cidr.as_deref(), Some("1.1.1.0/24"));
        assert_eq!(bundle.country.as_deref(), Some("AU"));
    }
    #[test]
    fn targets_and_public_metadata() {
        assert!(valid_target("::1"));
        assert!(valid_target("example.com"));
        assert!(!valid_target("https://example.com"));
        assert!(!valid_target("x & del *"));
        let r = parse_public(
            "1.1.1.1",
            &serde_json::json!({"company":"Registered owner","asn":"AS1 Owner"}),
        )
        .unwrap();
        assert_eq!(r.isp, None);
        assert!(r.organization.is_some());
        let r = parse_public(
            "2001:db8::1",
            &serde_json::json!({"success":true,"country":"Indonesia","connection":{"isp":"Fixture ISP","org":"Fixture Network","asn":64512}}),
        ).unwrap();
        assert_eq!(r.isp.as_deref(), Some("Fixture ISP"));
        assert_eq!(r.organization.as_deref(), Some("Fixture Network"));
        assert_eq!(r.asn.as_deref(), Some("AS64512"));
        assert_eq!(r.location.as_deref(), Some("Indonesia"));
        assert!(parse_public("invalid", &serde_json::Value::Null).is_err());
    }
}
