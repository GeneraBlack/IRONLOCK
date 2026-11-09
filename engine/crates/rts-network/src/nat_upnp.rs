use anyhow::{anyhow, bail, Context, Result};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpnpMappingResult {
    pub local_addr: SocketAddr,
    pub external_ip: Ipv4Addr,
    pub external_port: u16,
    pub is_public_wan_ip: bool,
    pub gateway_control_url: String,
}

impl UpnpMappingResult {
    pub fn external_socket_addr(&self) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(self.external_ip), self.external_port)
    }
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
    let local_ip = detect_local_lan_ipv4()?;
    let location_url = ssd_discover_igd_location(ssdp_timeout)?;
    let (control_url, service_type) = fetch_igd_control_url(&location_url)?;

    // 1. Request SOAP AddPortMapping
    let add_mapping_body = format!(
        r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:AddPortMapping xmlns:u="{service_type}">
      <NewRemoteHost></NewRemoteHost>
      <NewExternalPort>{external_port}</NewExternalPort>
      <NewProtocol>TCP</NewProtocol>
      <NewInternalPort>{local_port}</NewInternalPort>
      <NewInternalClient>{local_ip}</NewInternalClient>
      <NewEnabled>1</NewEnabled>
      <NewPortMappingDescription>{description}</NewPortMappingDescription>
      <NewLeaseDuration>{lease_duration_secs}</NewLeaseDuration>
    </u:AddPortMapping>
  </s:Body>
</s:Envelope>"#
    );
    let _ = send_soap_action(
        &control_url,
        &format!("\"{service_type}#AddPortMapping\""),
        &add_mapping_body,
    )?;

    // 2. Request SOAP GetExternalIPAddress
    let get_ip_body = format!(
        r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:GetExternalIPAddress xmlns:u="{service_type}">
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
        is_public_wan_ip: !is_cgnat_or_private_ipv4(external_ip),
        gateway_control_url: control_url,
    })
}

fn ssd_discover_igd_location(timeout: Duration) -> Result<String> {
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.set_read_timeout(Some(timeout))?;
    sock.set_write_timeout(Some(timeout))?;

    let search_targets = [
        "urn:schemas-upnp-org:service:WANIPConnection:1",
        "urn:schemas-upnp-org:service:WANPPPConnection:1",
        "urn:schemas-upnp-org:device:InternetGatewayDevice:1",
    ];
    let multicast_addr: SocketAddr = "239.255.255.250:1900".parse()?;

    for st in search_targets {
        let req = format!(
            "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 1\r\nST: {st}\r\n\r\n"
        );
        let _ = sock.send_to(req.as_bytes(), multicast_addr);
    }

    let mut buf = [0u8; 4096];
    let (n, _) = sock
        .recv_from(&mut buf)
        .context("No UPnP IGD router responded to SSDP M-SEARCH within timeout")?;
    let text = String::from_utf8_lossy(&buf[..n]);
    for line in text.lines() {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("location") {
                return Ok(v.trim().to_string());
            }
        }
    }
    bail!("SSDP response did not contain a LOCATION header")
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
    stream.read_to_end(&mut resp)?;
    let xml = String::from_utf8_lossy(&resp).to_string();

    for svc in [
        "urn:schemas-upnp-org:service:WANIPConnection:1",
        "urn:schemas-upnp-org:service:WANPPPConnection:1",
    ] {
        if let Some(pos) = xml.find(svc) {
            let tail = &xml[pos..];
            if let Some(ctrl_rel) = extract_xml_tag(tail, "controlURL") {
                let full_ctrl = if ctrl_rel.starts_with("http://") {
                    ctrl_rel
                } else if ctrl_rel.starts_with('/') {
                    format!("{base_url}{ctrl_rel}")
                } else {
                    format!("{base_url}/{ctrl_rel}")
                };
                return Ok((full_ctrl, svc.to_string()));
            }
        }
    }
    bail!("No WANIPConnection or WANPPPConnection service found in IGD XML")
}

fn send_soap_action(control_url: &str, soap_action: &str, xml_body: &str) -> Result<String> {
    let (host_port, _base_url, path) = parse_http_url(control_url)?;
    let addr = host_port
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| anyhow!("Could not resolve UPnP control host {host_port}"))?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(800))?;
    stream.set_read_timeout(Some(Duration::from_millis(1200)))?;
    stream.set_write_timeout(Some(Duration::from_millis(1200)))?;

    let body_bytes = xml_body.as_bytes();
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: {host_port}\r\nContent-Type: text/xml; charset=\"utf-8\"\r\nSOAPAction: {soap_action}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body_bytes.len()
    );
    stream.write_all(req.as_bytes())?;
    stream.write_all(body_bytes)?;
    stream.flush()?;

    let mut resp = Vec::new();
    stream.read_to_end(&mut resp)?;
    let text = String::from_utf8_lossy(&resp).to_string();
    let status_ok = text
        .lines()
        .next()
        .map(|l| l.contains("200 "))
        .unwrap_or(false);
    if !status_ok {
        bail!("UPnP SOAP request failed: {}", text.lines().next().unwrap_or(""));
    }
    Ok(text)
}

pub fn parse_http_url(url: &str) -> Result<(String, String, String)> {
    let stripped = url
        .trim()
        .strip_prefix("http://")
        .or_else(|| url.trim().strip_prefix("https://"))
        .unwrap_or(url.trim());
    let (host_part, path_part) = match stripped.split_once('/') {
        Some((h, p)) => (h, format!("/{p}")),
        None => (stripped, "/".to_string()),
    };
    let host_port = if host_part.contains(':') {
        host_part.to_string()
    } else {
        format!("{host_part}:80")
    };
    let base_url = format!("http://{host_port}");
    Ok((host_port, base_url, path_part))
}

pub fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
