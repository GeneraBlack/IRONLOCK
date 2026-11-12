use anyhow::{anyhow, bail, Context, Result};
use std::io::{ErrorKind, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

pub const UPNP_SERVICE_TYPES: [&str; 3] = [
    "urn:schemas-upnp-org:service:WANIPConnection:2",
    "urn:schemas-upnp-org:service:WANIPConnection:1",
    "urn:schemas-upnp-org:service:WANPPPConnection:1",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpnpMappingResult {
    pub local_addr: SocketAddr,
    pub external_ip: Ipv4Addr,
    pub external_port: u16,
    pub protocol: String,
    pub is_public_wan_ip: bool,
    pub gateway_control_url: String,
    pub service_type: String,
}

impl UpnpMappingResult {
    pub fn external_socket_addr(&self) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(self.external_ip), self.external_port)
    }

    /// Removes this TCP/UDP port mapping from the UPnP IGD router.
    pub fn delete_port_mapping(&self) -> Result<()> {
        let proto = if self.protocol.trim().is_empty() {
            "TCP"
        } else {
            self.protocol.trim()
        };
        delete_port_mapping(
            &self.gateway_control_url,
            &self.service_type,
            self.external_port,
            proto,
        )
    }
}

/// RAII lease guard that automatically sends SOAP `DeletePortMapping` for all mapped TCP and UDP
/// ports when dropped so home router UPnP port tables never leak across sessions.
#[derive(Debug, Default)]
pub struct UpnpLeaseGuard {
    leases: Vec<UpnpMappingResult>,
}

impl UpnpLeaseGuard {
    pub fn new() -> Self {
        Self { leases: Vec::new() }
    }

    pub fn from_mappings(leases: Vec<UpnpMappingResult>) -> Self {
        Self { leases }
    }

    /// Maps both `tcp_port` (`TCP`) and optional `udp_port` (`UDP`) on the local UPnP IGD gateway
    /// and returns an RAII [`UpnpLeaseGuard`] owning all succeeded leases.
    pub fn map_tcp_and_udp(
        tcp_port: u16,
        udp_port: Option<u16>,
        lease_duration_secs: u32,
        description: &str,
        ssdp_timeout: Duration,
    ) -> Self {
        let mut guard = Self::new();
        if let Ok(tcp_lease) = try_upnp_map_tcp_port_with_timeout(
            tcp_port,
            tcp_port,
            lease_duration_secs,
            description,
            ssdp_timeout,
        ) {
            guard.push(tcp_lease);
        }
        if let Some(u_port) = udp_port {
            if let Ok(udp_lease) = try_upnp_map_udp_port_with_timeout(
                u_port,
                u_port,
                lease_duration_secs,
                description,
                ssdp_timeout,
            ) {
                guard.push(udp_lease);
            }
        }
        guard
    }

    pub fn push(&mut self, mapping: UpnpMappingResult) {
        self.leases.push(mapping);
    }

    pub fn leases(&self) -> &[UpnpMappingResult] {
        &self.leases
    }

    pub fn is_empty(&self) -> bool {
        self.leases.is_empty()
    }

    pub fn tcp_mapping(&self) -> Option<&UpnpMappingResult> {
        self.leases
            .iter()
            .find(|m| m.protocol.eq_ignore_ascii_case("TCP"))
    }

    pub fn udp_mapping(&self) -> Option<&UpnpMappingResult> {
        self.leases
            .iter()
            .find(|m| m.protocol.eq_ignore_ascii_case("UDP"))
    }

    /// Explicitly deletes all owned UPnP port mappings now and returns how many succeeded.
    pub fn release(&mut self) -> usize {
        let drained = std::mem::take(&mut self.leases);
        let mut deleted = 0usize;
        for mapping in drained {
            if mapping.delete_port_mapping().is_ok() {
                deleted += 1;
            }
        }
        deleted
    }
}

impl Drop for UpnpLeaseGuard {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

/// Escapes special XML characters (`&`, `<`, `>`, `"`, `'`) and strips control characters
/// to prevent malformed XML or SOAP injection via user-supplied strings.
pub fn escape_xml(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 8);
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

/// Returns `true` if the IPv4 address is private (RFC1918), loopback, link-local,
/// or Carrier-Grade NAT / DS-Lite (RFC6598 `100.64.0.0/10`).
pub fn is_cgnat_or_private_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    let is_cgnat = octets[0] == 100 && (octets[1] & 0b1100_0000) == 64; // 100.64.0.0/10
    ip.is_private() || ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() || is_cgnat
}

/// Discovers the local LAN IPv4 address used to reach the default internet gateway.
pub fn detect_local_lan_ipv4() -> Result<Ipv4Addr> {
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.connect("8.8.8.8:80")?;
    match sock.local_addr()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() => Ok(ip),
        _ => bail!("Could not determine non-loopback local IPv4 address"),
    }
}

pub fn build_add_port_mapping_soap(
    service_type: &str,
    external_port: u16,
    protocol: &str,
    local_port: u16,
    local_ip: Ipv4Addr,
    lease_duration_secs: u32,
    description: &str,
) -> String {
    let safe_service = escape_xml(service_type);
    let safe_proto = escape_xml(protocol);
    let safe_desc = escape_xml(description);
    format!(
        r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:AddPortMapping xmlns:u="{safe_service}">
      <NewRemoteHost></NewRemoteHost>
      <NewExternalPort>{external_port}</NewExternalPort>
      <NewProtocol>{safe_proto}</NewProtocol>
      <NewInternalPort>{local_port}</NewInternalPort>
      <NewInternalClient>{local_ip}</NewInternalClient>
      <NewEnabled>1</NewEnabled>
      <NewPortMappingDescription>{safe_desc}</NewPortMappingDescription>
      <NewLeaseDuration>{lease_duration_secs}</NewLeaseDuration>
    </u:AddPortMapping>
  </s:Body>
</s:Envelope>"#
    )
}

pub fn build_delete_port_mapping_soap(
    service_type: &str,
    external_port: u16,
    protocol: &str,
) -> String {
    let safe_service = escape_xml(service_type);
    let safe_proto = escape_xml(protocol);
    format!(
        r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:DeletePortMapping xmlns:u="{safe_service}">
      <NewRemoteHost></NewRemoteHost>
      <NewExternalPort>{external_port}</NewExternalPort>
      <NewProtocol>{safe_proto}</NewProtocol>
    </u:DeletePortMapping>
  </s:Body>
</s:Envelope>"#
    )
}

/// Deletes an existing UPnP port mapping using a known `control_url` and `service_type`.
pub fn delete_port_mapping(
    control_url: &str,
    service_type: &str,
    external_port: u16,
    protocol: &str,
) -> Result<()> {
    let svc = if service_type.trim().is_empty() {
        "urn:schemas-upnp-org:service:WANIPConnection:1"
    } else {
        service_type.trim()
    };
    let body = build_delete_port_mapping_soap(svc, external_port, protocol);
    let _ = send_soap_action(control_url, &format!("\"{svc}#DeletePortMapping\""), &body)?;
    Ok(())
}

/// Discovers the local UPnP IGD gateway and deletes a mapped external TCP port.
pub fn try_upnp_delete_tcp_port(external_port: u16, ssdp_timeout: Duration) -> Result<()> {
    let (control_url, service_type) = discover_igd_control_endpoint(ssdp_timeout)?;
    delete_port_mapping(&control_url, &service_type, external_port, "TCP")
}

/// Discovers the local UPnP IGD gateway and deletes a mapped external UDP port.
pub fn try_upnp_delete_udp_port(external_port: u16, ssdp_timeout: Duration) -> Result<()> {
    let (control_url, service_type) = discover_igd_control_endpoint(ssdp_timeout)?;
    delete_port_mapping(&control_url, &service_type, external_port, "UDP")
}

/// Attempts to automatically open a TCP port on the local router via UPnP IGD (SSDP + SOAP)
/// and queries the router's external WAN IPv4 address using a default 750ms SSDP discovery timeout.
pub fn try_upnp_map_tcp_port(
    local_port: u16,
    external_port: u16,
    lease_duration_secs: u32,
    description: &str,
) -> Result<UpnpMappingResult> {
    try_upnp_map_tcp_port_with_timeout(
        local_port,
        external_port,
        lease_duration_secs,
        description,
        Duration::from_millis(750),
    )
}

/// Attempts to automatically open a TCP port on the local router via UPnP IGD (SSDP + SOAP)
/// with a configurable SSDP discovery timeout.
pub fn try_upnp_map_tcp_port_with_timeout(
    local_port: u16,
    external_port: u16,
    lease_duration_secs: u32,
    description: &str,
    ssdp_timeout: Duration,
) -> Result<UpnpMappingResult> {
    try_upnp_map_port_with_timeout(
        local_port,
        external_port,
        "TCP",
        lease_duration_secs,
        description,
        ssdp_timeout,
    )
}

/// Attempts to automatically open a UDP port on the local router via UPnP IGD (SSDP + SOAP)
/// using a default 750ms SSDP discovery timeout.
pub fn try_upnp_map_udp_port(
    local_port: u16,
    external_port: u16,
    lease_duration_secs: u32,
    description: &str,
) -> Result<UpnpMappingResult> {
    try_upnp_map_udp_port_with_timeout(
        local_port,
        external_port,
        lease_duration_secs,
        description,
        Duration::from_millis(750),
    )
}

/// Attempts to automatically open a UDP port on the local router via UPnP IGD (SSDP + SOAP)
/// with a configurable SSDP discovery timeout.
pub fn try_upnp_map_udp_port_with_timeout(
    local_port: u16,
    external_port: u16,
    lease_duration_secs: u32,
    description: &str,
    ssdp_timeout: Duration,
) -> Result<UpnpMappingResult> {
    try_upnp_map_port_with_timeout(
        local_port,
        external_port,
        "UDP",
        lease_duration_secs,
        description,
        ssdp_timeout,
    )
}

fn try_upnp_map_port_with_timeout(
    local_port: u16,
    external_port: u16,
    protocol: &str,
    lease_duration_secs: u32,
    description: &str,
    ssdp_timeout: Duration,
) -> Result<UpnpMappingResult> {
    let proto_upper = protocol.trim().to_ascii_uppercase();
    let local_ip = detect_local_lan_ipv4()?;
    let (control_url, service_type) = discover_igd_control_endpoint(ssdp_timeout)?;

    // 1. Request SOAP AddPortMapping (falls back to permanent lease 0 if IGDv2 rejects dynamic lease duration)
    let add_mapping_body = build_add_port_mapping_soap(
        &service_type,
        external_port,
        &proto_upper,
        local_port,
        local_ip,
        lease_duration_secs,
        description,
    );
    let add_action = format!("\"{service_type}#AddPortMapping\"");
    if let Err(first_err) = send_soap_action(&control_url, &add_action, &add_mapping_body) {
        if lease_duration_secs != 0 {
            let fallback_body = build_add_port_mapping_soap(
                &service_type,
                external_port,
                &proto_upper,
                local_port,
                local_ip,
                0,
                description,
            );
            send_soap_action(&control_url, &add_action, &fallback_body).with_context(|| {
                format!("UPnP AddPortMapping ({proto_upper}) failed (initial error: {first_err})")
            })?;
        } else {
            return Err(first_err);
        }
    }

    // 2. Request SOAP GetExternalIPAddress
    let safe_service = escape_xml(&service_type);
    let get_ip_body = format!(
        r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:GetExternalIPAddress xmlns:u="{safe_service}">
    </u:GetExternalIPAddress>
  </s:Body>
</s:Envelope>"#
    );
    let ip_xml = send_soap_action(
        &control_url,
        &format!("\"{service_type}#GetExternalIPAddress\""),
        &get_ip_body,
    )?;
    let ext_ip_str = extract_xml_tag(&ip_xml, "NewExternalIPAddress")
        .ok_or_else(|| anyhow!("UPnP response missing NewExternalIPAddress"))?;
    let external_ip: Ipv4Addr = ext_ip_str
        .trim()
        .parse()
        .context("Invalid IPv4 in NewExternalIPAddress")?;

    Ok(UpnpMappingResult {
        local_addr: SocketAddr::new(IpAddr::V4(local_ip), local_port),
        external_ip,
        external_port,
        protocol: proto_upper,
        is_public_wan_ip: !is_cgnat_or_private_ipv4(external_ip),
        gateway_control_url: control_url,
        service_type,
    })
}

/// Extracts `(location_url, is_likely_igd)` from an SSDP UDP response packet.
pub fn parse_ssdp_response(packet_text: &str) -> Option<(String, bool)> {
    let mut location: Option<String> = None;
    let mut is_igd = false;

    for line in packet_text.lines() {
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim();
            let val = v.trim();
            if key.eq_ignore_ascii_case("location") && !val.is_empty() {
                location = Some(val.to_string());
            } else if key.eq_ignore_ascii_case("st")
                || key.eq_ignore_ascii_case("nt")
                || key.eq_ignore_ascii_case("usn")
            {
                let lower = val.to_ascii_lowercase();
                if lower.contains("wanipconnection")
                    || lower.contains("wanpppconnection")
                    || lower.contains("internetgatewaydevice")
                {
                    is_igd = true;
                }
            }
        }
    }

    location.map(|loc| (loc, is_igd))
}

/// Scans multiple SSDP M-SEARCH UDP responses on an open socket until `timeout` expires,
/// prioritizing InternetGatewayDevice / WANIPConnection responses over unrelated UPnP devices
/// (e.g. smart TVs, printers, media renderers).
pub fn collect_ssdp_igd_locations(sock: &UdpSocket, timeout: Duration) -> Vec<String> {
    let deadline = Instant::now() + timeout;
    let mut igd_locations: Vec<String> = Vec::new();
    let mut other_locations: Vec<String> = Vec::new();
    let mut buf = [0u8; 4096];

    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let remaining = deadline.saturating_duration_since(now);
        let step_wait = if !igd_locations.is_empty() {
            // Once we have at least one explicit IGD responder, only wait briefly for additional IGDv2 packets
            remaining.min(Duration::from_millis(25))
        } else {
            remaining
        };
        if step_wait.is_zero() {
            break;
        }
        let _ = sock.set_read_timeout(Some(step_wait));
        match sock.recv_from(&mut buf) {
            Ok((n, _)) => {
                let text = String::from_utf8_lossy(&buf[..n]);
                if let Some((loc, is_igd)) = parse_ssdp_response(&text) {
                    if is_igd {
                        if !igd_locations.iter().any(|x| x == &loc) {
                            igd_locations.push(loc);
                        }
                    } else if !other_locations.iter().any(|x| x == &loc)
                        && !igd_locations.iter().any(|x| x == &loc)
                    {
                        other_locations.push(loc);
                    }
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
                ) =>
            {
                if e.kind() == ErrorKind::Interrupted {
                    continue;
                }
                break;
            }
            Err(_) => break,
        }
    }

    igd_locations.extend(other_locations);
    igd_locations
}

fn discover_igd_control_endpoint(timeout: Duration) -> Result<(String, String)> {
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.set_write_timeout(Some(timeout.max(Duration::from_millis(100))))?;

    let search_targets = [
        "urn:schemas-upnp-org:service:WANIPConnection:2",
        "urn:schemas-upnp-org:service:WANIPConnection:1",
        "urn:schemas-upnp-org:service:WANPPPConnection:1",
        "urn:schemas-upnp-org:device:InternetGatewayDevice:2",
        "urn:schemas-upnp-org:device:InternetGatewayDevice:1",
    ];
    let multicast_addr: SocketAddr = "239.255.255.250:1900".parse()?;

    for st in search_targets {
        let req = format!(
            "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 1\r\nST: {st}\r\n\r\n"
        );
        let _ = sock.send_to(req.as_bytes(), multicast_addr);
    }

    let locations = collect_ssdp_igd_locations(&sock, timeout);
    if locations.is_empty() {
        bail!("No UPnP IGD router responded to SSDP M-SEARCH within timeout");
    }

    let mut last_err = anyhow!("No UPnP LOCATION yielded a valid WANIPConnection control URL");
    for loc in locations {
        match fetch_igd_control_url(&loc) {
            Ok(pair) => return Ok(pair),
            Err(err) => last_err = err,
        }
    }
    Err(last_err)
}

pub fn extract_igd_control_url_from_xml(xml: &str, base_url: &str) -> Option<(String, String)> {
    let resolve_ctrl = |ctrl_rel: String| -> String {
        if ctrl_rel.starts_with("http://") || ctrl_rel.starts_with("https://") {
            ctrl_rel
        } else if ctrl_rel.starts_with('/') {
            format!("{base_url}{ctrl_rel}")
        } else {
            format!("{base_url}/{ctrl_rel}")
        }
    };

    // First scan each `<service>...</service>` block so `<controlURL>` before `<serviceType>` works too
    for svc in UPNP_SERVICE_TYPES {
        let mut cursor = xml;
        while let Some((svc_block, after_block)) = next_xml_block(cursor, "service") {
            if let Some(found_type) = extract_xml_tag(svc_block, "serviceType") {
                if found_type.trim() == svc {
                    if let Some(ctrl_rel) = extract_xml_tag(svc_block, "controlURL") {
                        if !ctrl_rel.is_empty() {
                            return Some((resolve_ctrl(ctrl_rel), svc.to_string()));
                        }
                    }
                }
            }
            cursor = after_block;
        }

        // Fallback substring scan for non-standard device descriptions
        if let Some(pos) = xml.find(svc) {
            let tail = &xml[pos..];
            if let Some(ctrl_rel) = extract_xml_tag(tail, "controlURL") {
                if !ctrl_rel.is_empty() {
                    return Some((resolve_ctrl(ctrl_rel), svc.to_string()));
                }
            }
        }
    }

    None
}

fn fetch_igd_control_url(location_url: &str) -> Result<(String, String)> {
    let (host_port, base_url, path) = parse_http_url(location_url)?;
    let addr = host_port
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| anyhow!("Could not resolve UPnP gateway {host_port}"))?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(800))?;
    stream.set_read_timeout(Some(Duration::from_millis(1000)))?;
    stream.set_write_timeout(Some(Duration::from_millis(1000)))?;

    let req = format!("GET {path} HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes())?;
    let mut resp = Vec::new();
    stream.take(512 * 1024).read_to_end(&mut resp)?;
    let xml = String::from_utf8_lossy(&resp).to_string();

    extract_igd_control_url_from_xml(&xml, &base_url)
        .ok_or_else(|| anyhow!("No WANIPConnection or WANPPPConnection service found in IGD XML"))
}

fn send_soap_action(control_url: &str, soap_action: &str, xml_body: &str) -> Result<String> {
    let (host_port, _base_url, path) = parse_http_url(control_url)?;
    let addr = host_port
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| anyhow!("Could not resolve UPnP control host {host_port}"))?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(800))?;
    let _ = stream.set_nodelay(true);
    stream.set_read_timeout(Some(Duration::from_millis(1200)))?;
    stream.set_write_timeout(Some(Duration::from_millis(1200)))?;

    let body_bytes = xml_body.as_bytes();
    let req_head = format!(
        "POST {path} HTTP/1.1\r\nHost: {host_port}\r\nContent-Type: text/xml; charset=\"utf-8\"\r\nSOAPAction: {soap_action}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body_bytes.len()
    );
    let mut wire = Vec::with_capacity(req_head.len() + body_bytes.len());
    wire.extend_from_slice(req_head.as_bytes());
    wire.extend_from_slice(body_bytes);
    stream.write_all(&wire)?;
    stream.flush()?;

    let mut resp = Vec::new();
    stream.take(256 * 1024).read_to_end(&mut resp)?;
    let text = String::from_utf8_lossy(&resp).to_string();
    let status_ok = text
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .map(|code| (200..300).contains(&code))
        .unwrap_or(false);
    if !status_ok {
        bail!(
            "UPnP SOAP request failed: {}",
            text.lines().next().unwrap_or("")
        );
    }
    Ok(text)
}

pub fn parse_http_url(url: &str) -> Result<(String, String, String)> {
    let trimmed = url.trim();
    let (scheme, stripped) = if let Some(rest) = trimmed.strip_prefix("https://") {
        ("https", rest)
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        ("http", rest)
    } else {
        ("http", trimmed)
    };
    let (host_part, path_part) = match stripped.split_once('/') {
        Some((h, p)) => (h, format!("/{p}")),
        None => (stripped, "/".to_string()),
    };
    let default_port = if scheme == "https" { 443 } else { 80 };
    let host_port = if host_part.contains(':') {
        host_part.to_string()
    } else {
        format!("{host_part}:{default_port}")
    };
    let base_url = format!("{scheme}://{host_port}");
    Ok((host_port, base_url, path_part))
}

fn matches_local_xml_name(raw_name: &str, expected_local: &str) -> bool {
    let local = raw_name
        .rsplit_once(':')
        .map(|(_, l)| l)
        .unwrap_or(raw_name);
    local == expected_local
}

fn next_xml_block<'a>(xml: &'a str, tag: &str) -> Option<(&'a str, &'a str)> {
    let bytes = xml.as_bytes();
    let mut i = 0;
    while let Some(rel_lt) = xml[i..].find('<') {
        let lt = i + rel_lt;
        let after_lt = lt + 1;
        if after_lt >= bytes.len() {
            return None;
        }
        let first = bytes[after_lt];
        if first == b'/' || first == b'?' || first == b'!' {
            i = after_lt;
            continue;
        }
        let rel_gt = xml[after_lt..].find('>')?;
        let gt = after_lt + rel_gt;
        let inside = xml[after_lt..gt].trim();
        if inside.ends_with('/') {
            i = gt + 1;
            continue;
        }
        let name_token = inside.split_whitespace().next().unwrap_or("");
        if matches_local_xml_name(name_token, tag) {
            let content_start = gt + 1;
            let mut j = content_start;
            while let Some(close_rel) = xml[j..].find("</") {
                let close_lt = j + close_rel;
                let close_name_start = close_lt + 2;
                let close_rel_gt = xml[close_name_start..].find('>')?;
                let close_gt = close_name_start + close_rel_gt;
                let close_name = xml[close_name_start..close_gt].trim();
                if matches_local_xml_name(close_name, tag) {
                    return Some((&xml[content_start..close_lt], &xml[(close_gt + 1)..]));
                }
                j = close_gt + 1;
            }
            return None;
        }
        i = gt + 1;
    }
    None
}

/// Extracts the trimmed text inside `<tag>...</tag>` or namespace-prefixed `<ns:tag attr="...">...</ns:tag>`.
pub fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
    next_xml_block(xml, tag).map(|(inner, _)| inner.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn parses_upnp_http_urls_and_xml_tags() {
        let (hp, base, path) =
            parse_http_url("http://192.168.178.1:49000/igddesc.xml").expect("parse url");
        assert_eq!(hp, "192.168.178.1:49000");
        assert_eq!(base, "http://192.168.178.1:49000");
        assert_eq!(path, "/igddesc.xml");

        let sample_xml = "<root><service><controlURL>/upnp/control/WANIPConn1</controlURL><NewExternalIPAddress>93.184.216.34</NewExternalIPAddress></service></root>";
        assert_eq!(
            extract_xml_tag(sample_xml, "controlURL").as_deref(),
            Some("/upnp/control/WANIPConn1")
        );
        assert_eq!(
            extract_xml_tag(sample_xml, "NewExternalIPAddress").as_deref(),
            Some("93.184.216.34")
        );
        assert!(!is_cgnat_or_private_ipv4("93.184.216.34".parse().unwrap()));
        assert!(is_cgnat_or_private_ipv4("192.168.178.20".parse().unwrap()));
        assert!(is_cgnat_or_private_ipv4("100.64.12.5".parse().unwrap())); // RFC6598 CGNAT / DS-Lite
        assert!(is_cgnat_or_private_ipv4("100.127.255.254".parse().unwrap()));
        assert!(!is_cgnat_or_private_ipv4("100.128.0.1".parse().unwrap()));
    }

    #[test]
    fn escapes_xml_and_supports_igdv2_namespaced_tags_and_multi_ssdp_delete() {
        let escaped = escape_xml("Host <Alpha> & \"Bravo\" 'Charlie'\x07");
        assert_eq!(
            escaped,
            "Host &lt;Alpha&gt; &amp; &quot;Bravo&quot; &apos;Charlie&apos;"
        );
        let soap = build_add_port_mapping_soap(
            "urn:schemas-upnp-org:service:WANIPConnection:2",
            7878,
            "TCP",
            7878,
            Ipv4Addr::new(192, 168, 1, 50),
            3600,
            "Game <Host> & Co",
        );
        assert!(soap.contains("Game &lt;Host&gt; &amp; Co"));
        assert!(!soap.contains("Game <Host>"));

        // Namespace-prefixed & attribute-bearing tags
        let ns_soap_resp = r#"<s:Envelope><s:Body><u:GetExternalIPAddressResponse xmlns:u="urn:schemas-upnp-org:service:WANIPConnection:2"><u:NewExternalIPAddress dt:dt="string">198.51.100.77</u:NewExternalIPAddress></u:GetExternalIPAddressResponse></s:Body></s:Envelope>"#;
        assert_eq!(
            extract_xml_tag(ns_soap_resp, "NewExternalIPAddress").as_deref(),
            Some("198.51.100.77")
        );

        // IGDv2 XML where `<controlURL>` precedes `<serviceType>` inside `<service>`
        let igdv2_xml = r#"<?xml version="1.0"?>
<root xmlns="urn:schemas-upnp-org:device-1-0">
  <device>
    <serviceList>
      <service>
        <controlURL>/MediaRenderer/AVTransport/Control</controlURL>
        <serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType>
      </service>
      <service>
        <s:controlURL>/igd2/ctl/IPConn</s:controlURL>
        <s:serviceType>urn:schemas-upnp-org:service:WANIPConnection:2</s:serviceType>
      </service>
    </serviceList>
  </device>
</root>"#;
        let (ctrl, svc) =
            extract_igd_control_url_from_xml(igdv2_xml, "http://192.168.1.1:49000").unwrap();
        assert_eq!(ctrl, "http://192.168.1.1:49000/igd2/ctl/IPConn");
        assert_eq!(svc, "urn:schemas-upnp-org:service:WANIPConnection:2");

        // Multi-packet SSDP scan: non-router device responds first, IGDv2 router responds second
        let recv_sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        let recv_addr = recv_sock.local_addr().unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        let tv_pkt = "HTTP/1.1 200 OK\r\nST: urn:schemas-upnp-org:device:MediaRenderer:1\r\nLOCATION: http://192.168.1.90:8008/ssdp/device-desc.xml\r\n\r\n";
        let igd_pkt = "HTTP/1.1 200 OK\r\nST: urn:schemas-upnp-org:service:WANIPConnection:2\r\nLOCATION: http://192.168.1.1:49000/igddesc.xml\r\n\r\n";
        sender.send_to(tv_pkt.as_bytes(), recv_addr).unwrap();
        sender.send_to(igd_pkt.as_bytes(), recv_addr).unwrap();

        let locs = collect_ssdp_igd_locations(&recv_sock, Duration::from_millis(120));
        assert_eq!(locs.len(), 2);
        assert_eq!(
            locs[0], "http://192.168.1.1:49000/igddesc.xml",
            "IGDv2 router must be prioritized ahead of non-IGD UPnP devices"
        );

        // Verify DeletePortMapping against a local mock UPnP SOAP server
        let soap_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let soap_addr = soap_listener.local_addr().unwrap();
        let soap_thread = thread::spawn(move || {
            let (mut stream, _) = soap_listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_millis(500)))
                .unwrap();
            let mut req_bytes = Vec::new();
            let mut chunk = [0u8; 512];
            while req_bytes.len() < 8192 {
                match stream.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        req_bytes.extend_from_slice(&chunk[..n]);
                        if String::from_utf8_lossy(&req_bytes).contains("</s:Envelope>") {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            let req_txt = String::from_utf8_lossy(&req_bytes).to_string();
            let resp = "HTTP/1.1 200 OK\r\nContent-Type: text/xml\r\nContent-Length: 45\r\nConnection: close\r\n\r\n<s:Envelope><s:Body></s:Body></s:Envelope>";
            stream.write_all(resp.as_bytes()).unwrap();
            let _ = stream.flush();
            let _ = stream.shutdown(std::net::Shutdown::Write);
            let _ = stream.read(&mut chunk);
            req_txt
        });

        let ctrl_url = format!("http://{soap_addr}/igd2/ctl/IPConn");
        delete_port_mapping(
            &ctrl_url,
            "urn:schemas-upnp-org:service:WANIPConnection:2",
            7878,
            "TCP",
        )
        .expect("delete_port_mapping should succeed");
        let received_soap = soap_thread.join().unwrap();
        assert!(received_soap.contains("WANIPConnection:2#DeletePortMapping"));
        assert!(received_soap.contains("<NewExternalPort>7878</NewExternalPort>"));
    }

    #[test]
    fn upnp_lease_guard_drop_deletes_both_tcp_and_udp_port_mappings() {
        let soap_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let soap_addr = soap_listener.local_addr().unwrap();
        let collector = thread::spawn(move || {
            let mut captured = Vec::new();
            for _ in 0..2 {
                let (mut stream, _) = soap_listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_millis(500)))
                    .unwrap();
                let mut req_bytes = Vec::new();
                let mut chunk = [0u8; 512];
                while req_bytes.len() < 8192 {
                    match stream.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => {
                            req_bytes.extend_from_slice(&chunk[..n]);
                            if String::from_utf8_lossy(&req_bytes).contains("</s:Envelope>") {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                captured.push(String::from_utf8_lossy(&req_bytes).to_string());
                let resp = "HTTP/1.1 200 OK\r\nContent-Type: text/xml\r\nContent-Length: 45\r\nConnection: close\r\n\r\n<s:Envelope><s:Body></s:Body></s:Envelope>";
                stream.write_all(resp.as_bytes()).unwrap();
                let _ = stream.flush();
                let _ = stream.shutdown(std::net::Shutdown::Write);
            }
            captured
        });

        let ctrl_url = format!("http://{soap_addr}/igd2/ctl/IPConn");
        {
            let mut guard = UpnpLeaseGuard::new();
            guard.push(UpnpMappingResult {
                local_addr: "192.168.1.40:7878".parse().unwrap(),
                external_ip: Ipv4Addr::new(93, 184, 216, 34),
                external_port: 7878,
                protocol: "TCP".to_string(),
                is_public_wan_ip: true,
                gateway_control_url: ctrl_url.clone(),
                service_type: "urn:schemas-upnp-org:service:WANIPConnection:2".to_string(),
            });
            guard.push(UpnpMappingResult {
                local_addr: "192.168.1.40:7879".parse().unwrap(),
                external_ip: Ipv4Addr::new(93, 184, 216, 34),
                external_port: 7879,
                protocol: "UDP".to_string(),
                is_public_wan_ip: true,
                gateway_control_url: ctrl_url,
                service_type: "urn:schemas-upnp-org:service:WANIPConnection:2".to_string(),
            });
            assert!(guard.tcp_mapping().is_some());
            assert!(guard.udp_mapping().is_some());
            // `guard` drops here at scope exit -> triggers SOAP DeletePortMapping for TCP 7878 & UDP 7879
        }

        let requests = collector.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests
            .iter()
            .any(|r| r.contains("<NewProtocol>TCP</NewProtocol>")
                && r.contains("<NewExternalPort>7878</NewExternalPort>")));
        assert!(requests
            .iter()
            .any(|r| r.contains("<NewProtocol>UDP</NewProtocol>")
                && r.contains("<NewExternalPort>7879</NewExternalPort>")));
    }
}
