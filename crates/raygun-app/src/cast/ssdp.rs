//! Finding TVs with SSDP (UPnP's discovery): an `M-SEARCH` for DLNA media renderers and for DIAL
//! devices (TVs' app launchers, which Chromecasts answer too), sent from each network interface,
//! answered by unicast. Each answer names the device's description, an XML file saying what it is
//! and where its services are controlled.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use quick_xml::Reader;
use quick_xml::events::Event;

pub const MEDIA_RENDERER: &str = "urn:schemas-upnp-org:device:MediaRenderer:1";
pub const DIAL: &str = "urn:dial-multiscreen-org:service:dial:1";
const AV_TRANSPORT: &str = "urn:schemas-upnp-org:service:AVTransport:";
const RENDERING_CONTROL: &str = "urn:schemas-upnp-org:service:RenderingControl:";
const GROUP: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(239, 255, 255, 250), 1900);
/// The longest answer read; descriptions come over HTTP.
const MAX_ANSWER: usize = 4096;

/// An `M-SEARCH` for `target`, sent to `host` (the multicast group, or one device).
pub fn search_request(target: &str, host: &str) -> String {
    format!(
        "M-SEARCH * HTTP/1.1\r\nHOST: {host}\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: {target}\r\nUSER-AGENT: {}/1 UPnP/1.1 Raygun/1\r\n\r\n",
        std::env::consts::OS
    )
}

/// A device's answer to an `M-SEARCH`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// Its description's address.
    pub location: String,
    /// What it answered as (`ST`).
    pub target: String,
    pub usn: String,
}

/// Reads an answer: `HTTP/1.1 200 OK` and its headers, in any case.
pub fn parse_answer(text: &str) -> Option<Answer> {
    let mut lines = text.split("\r\n").flat_map(|line| line.split('\n'));
    let status = lines.next()?;
    if !status.starts_with("HTTP/")
        || !status
            .split_whitespace()
            .nth(1)
            .is_some_and(|code| code == "200")
    {
        return None;
    }
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
        }
    }
    let location = headers.remove("location")?;
    if !location.starts_with("http://") && !location.starts_with("https://") {
        return None;
    }
    Some(Answer {
        location,
        target: headers.remove("st").unwrap_or_default(),
        usn: headers.remove("usn").unwrap_or_default(),
    })
}

/// What a device description says.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Description {
    pub device_type: String,
    pub name: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub udn: String,
    /// AVTransport's control address, resolved: what plays a video (DLNA renderers).
    pub av_transport: Option<String>,
    /// RenderingControl's: the volume.
    pub rendering_control: Option<String>,
}

impl Description {
    /// A DLNA renderer has somewhere to send videos.
    pub fn is_renderer(&self) -> bool {
        self.av_transport.is_some()
    }

    /// Made by Google: a Chromecast or Google TV answering as a DIAL device.
    pub fn is_google(&self) -> bool {
        self.manufacturer
            .as_deref()
            .is_some_and(|maker| maker.to_ascii_lowercase().contains("google"))
    }
}

#[derive(Default)]
struct DeviceScan {
    fields: Description,
    service_type: String,
    control: String,
}

/// Reads a device description fetched from `location`. A TV's renderer may be an embedded device
/// of its root device: the device with an AVTransport service is the one described, else the
/// root.
pub fn parse_description(xml: &str, location: &str) -> Option<Description> {
    let mut reader = Reader::from_reader(xml.as_bytes());
    let mut buffer = Vec::new();
    let mut base: Option<String> = None;
    let mut stack: Vec<DeviceScan> = Vec::new();
    let mut done: Vec<Description> = Vec::new();
    let mut path: Vec<String> = Vec::new();
    let mut text = String::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) => {
                let name = local_name(element.name().as_ref());
                if name == "device" {
                    stack.push(DeviceScan::default());
                }
                path.push(name);
                text.clear();
            }
            // Entity references arrive as their own events.
            Ok(Event::Text(content)) => text.push_str(&content),
            Ok(Event::CData(content)) => text.push_str(&content),
            Ok(Event::GeneralRef(reference)) => {
                let entity = format!("&{};", &*reference);
                text.push_str(&quick_xml::escape::unescape(&entity).unwrap_or_default());
            }
            Ok(Event::End(_)) => {
                let Some(name) = path.pop() else { break };
                let value = std::mem::take(&mut text).trim().to_owned();
                if name == "URLBase" && path.len() == 1 {
                    base = Some(value);
                    continue;
                }
                if name == "device" {
                    if let Some(device) = stack.pop() {
                        done.push(device.fields);
                    }
                    continue;
                }
                let Some(device) = stack.last_mut() else {
                    continue;
                };
                // Fields of the device itself, not of its services or icons.
                match (path.last().map(String::as_str), name.as_str()) {
                    (Some("device"), "deviceType") => device.fields.device_type = value,
                    (Some("device"), "friendlyName") => device.fields.name = value,
                    (Some("device"), "manufacturer") => device.fields.manufacturer = Some(value),
                    (Some("device"), "modelName") => device.fields.model = Some(value),
                    (Some("device"), "UDN") => device.fields.udn = value,
                    (Some("service"), "serviceType") => device.service_type = value,
                    (Some("service"), "controlURL") => device.control = value,
                    (_, "service") => {
                        let control = std::mem::take(&mut device.control);
                        let service_type = std::mem::take(&mut device.service_type);
                        let resolved = resolve(base.as_deref().unwrap_or(location), &control);
                        if service_type.starts_with(AV_TRANSPORT) {
                            device.fields.av_transport = resolved;
                        } else if service_type.starts_with(RENDERING_CONTROL) {
                            device.fields.rendering_control = resolved;
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buffer.clear();
    }
    // Embedded devices close before their root, so the root is last.
    let root = done.last().cloned()?;
    let mut chosen = done
        .into_iter()
        .find(Description::is_renderer)
        .unwrap_or_else(|| root.clone());
    if chosen.name.is_empty() {
        chosen.name = root.name;
    }
    if chosen.udn.is_empty() {
        chosen.udn = root.udn;
    }
    (!chosen.udn.is_empty() || chosen.is_renderer()).then_some(chosen)
}

fn local_name(name: &str) -> String {
    name.rsplit(':').next().unwrap_or_default().to_owned()
}
/// `relative` against `base`; only web addresses.
pub fn resolve(base: &str, relative: &str) -> Option<String> {
    let relative = relative.trim();
    if relative.is_empty() {
        return None;
    }
    let joined = url::Url::parse(base).ok()?.join(relative).ok()?;
    matches!(joined.scheme(), "http" | "https").then(|| joined.to_string())
}

/// This device's IPv4 addresses on its networks (Wi-Fi, Ethernet), each to search from.
fn interfaces() -> Vec<Ipv4Addr> {
    let mut found: Vec<Ipv4Addr> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|interface| !interface.is_loopback())
        .filter_map(|interface| match interface.ip() {
            IpAddr::V4(ip) if ip.is_private() || ip.is_link_local() => Some(ip),
            _ => None,
        })
        .collect();
    found.sort();
    found.dedup();
    found
}

/// A UDP socket bound to `interface` that sends multicast out of it.
fn socket_on(interface: Ipv4Addr) -> std::io::Result<tokio::net::UdpSocket> {
    use socket2::{Domain, Protocol, Socket, Type};
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_multicast_if_v4(&interface)?;
    socket.set_multicast_ttl_v4(2)?;
    socket.bind(&SocketAddr::from((interface, 0)).into())?;
    socket.set_nonblocking(true)?;
    tokio::net::UdpSocket::from_std(socket.into())
}

/// Searches every network for `targets` and collects answers for `wait`, each device once.
pub async fn search(targets: &[&str], wait: Duration) -> Vec<(Answer, IpAddr)> {
    let mut sockets = Vec::new();
    for interface in interfaces() {
        match socket_on(interface) {
            Ok(socket) => sockets.push(socket),
            Err(error) => tracing::debug!(%interface, %error, "no SSDP search there"),
        }
    }
    if sockets.is_empty() {
        // No address found (or a platform that hides them): the system's choice of network.
        if let Ok(socket) = tokio::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await {
            sockets.push(socket);
        }
    }
    let group = GROUP.to_string();
    for socket in &sockets {
        for target in targets {
            // Sent twice: UDP may drop one.
            for _ in 0..2 {
                let _ = socket
                    .send_to(search_request(target, &group).as_bytes(), GROUP)
                    .await;
            }
        }
    }
    collect(sockets, wait).await
}

/// Asks one device directly (UPnP 1.1's unicast `M-SEARCH`), for a TV added by its address.
pub async fn search_at(ip: IpAddr, targets: &[&str], wait: Duration) -> Vec<(Answer, IpAddr)> {
    let bind: SocketAddr = match ip {
        IpAddr::V4(_) => (Ipv4Addr::UNSPECIFIED, 0).into(),
        IpAddr::V6(_) => (std::net::Ipv6Addr::UNSPECIFIED, 0).into(),
    };
    let Ok(socket) = tokio::net::UdpSocket::bind(bind).await else {
        return Vec::new();
    };
    let to = SocketAddr::new(ip, 1900);
    for target in targets {
        for _ in 0..2 {
            let _ = socket
                .send_to(search_request(target, &to.to_string()).as_bytes(), to)
                .await;
        }
    }
    collect(vec![socket], wait).await
}

async fn collect(sockets: Vec<tokio::net::UdpSocket>, wait: Duration) -> Vec<(Answer, IpAddr)> {
    let (sender, mut answers) = tokio::sync::mpsc::unbounded_channel();
    let mut readers = Vec::new();
    for socket in sockets {
        let sender = sender.clone();
        readers.push(tokio::spawn(async move {
            let mut buffer = vec![0u8; MAX_ANSWER];
            while let Ok((length, from)) = socket.recv_from(&mut buffer).await {
                let text = String::from_utf8_lossy(&buffer[..length]);
                if let Some(answer) = parse_answer(&text) {
                    let _ = sender.send((answer, from.ip()));
                }
            }
        }));
    }
    drop(sender);
    tokio::time::sleep(wait).await;
    for reader in readers {
        reader.abort();
    }
    let mut seen = std::collections::HashSet::new();
    let mut found = Vec::new();
    while let Ok((answer, from)) = answers.try_recv() {
        if seen.insert(answer.location.clone()) {
            found.push((answer, from));
        }
    }
    found
}

/// Fetches and reads a description; `dial` says whether its answer named a DIAL app server.
pub async fn describe(
    client: &reqwest::Client,
    location: &str,
) -> Result<(Description, bool), String> {
    let response = client
        .get(location)
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("the device answered {}", response.status()));
    }
    let dial = response.headers().contains_key("application-url");
    let text = response.text().await.map_err(|error| error.to_string())?;
    parse_description(&text, location)
        .map(|description| (description, dial))
        .ok_or_else(|| "the device's description couldn't be read".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searches_ask_for_one_kind_of_device() {
        let request = search_request(MEDIA_RENDERER, "239.255.255.250:1900");
        assert!(request.starts_with("M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\n"));
        assert!(request.contains("MAN: \"ssdp:discover\"\r\n"));
        assert!(request.contains("ST: urn:schemas-upnp-org:device:MediaRenderer:1\r\n"));
        assert!(request.ends_with("\r\n\r\n"));
    }

    #[test]
    fn answers_name_their_description() {
        let answer = "HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age=1800\r\nDATE: Tue, 03 Nov 2026 10:00:00 GMT\r\nEXT:\r\nLocation: http://192.168.1.20:9197/dmr\r\nSERVER: SHP, UPnP/1.0, Samsung UPnP SDK/1.0\r\nST: urn:schemas-upnp-org:device:MediaRenderer:1\r\nUSN: uuid:0a1b::urn:schemas-upnp-org:device:MediaRenderer:1\r\n\r\n";
        assert_eq!(
            parse_answer(answer),
            Some(Answer {
                location: "http://192.168.1.20:9197/dmr".into(),
                target: MEDIA_RENDERER.into(),
                usn: "uuid:0a1b::urn:schemas-upnp-org:device:MediaRenderer:1".into(),
            })
        );
        // Announcements, errors and answers without an address are left out.
        assert_eq!(
            parse_answer("NOTIFY * HTTP/1.1\r\nLOCATION: http://a/\r\n\r\n"),
            None
        );
        assert_eq!(
            parse_answer("HTTP/1.1 404 Not Found\r\nLOCATION: http://a/\r\n\r\n"),
            None
        );
        assert_eq!(parse_answer("HTTP/1.1 200 OK\r\nST: x\r\n\r\n"), None);
        assert_eq!(
            parse_answer("HTTP/1.1 200 OK\r\nLOCATION: file:///etc\r\n\r\n"),
            None
        );
    }

    #[test]
    fn a_tvs_renderer_inside_its_root_device() {
        let xml = r#"<?xml version="1.0"?>
<root xmlns="urn:schemas-upnp-org:device-1-0" xmlns:dlna="urn:schemas-dlna-org:device-1-0">
  <specVersion><major>1</major><minor>0</minor></specVersion>
  <device>
    <deviceType>urn:schemas-upnp-org:device:Basic:1</deviceType>
    <friendlyName>Living Room TV</friendlyName>
    <manufacturer>Example &amp; Co</manufacturer>
    <UDN>uuid:root-1</UDN>
    <iconList><icon><mimetype>image/png</mimetype><url>/icon.png</url></icon></iconList>
    <deviceList>
      <device>
        <deviceType>urn:schemas-upnp-org:device:MediaRenderer:1</deviceType>
        <friendlyName></friendlyName>
        <modelName>UE55TU7000</modelName>
        <UDN>uuid:renderer-1</UDN>
        <serviceList>
          <service>
            <serviceType>urn:schemas-upnp-org:service:RenderingControl:1</serviceType>
            <controlURL>/upnp/control/RenderingControl1</controlURL>
          </service>
          <service>
            <serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType>
            <controlURL>upnp/control/AVTransport1</controlURL>
          </service>
        </serviceList>
      </device>
    </deviceList>
  </device>
</root>"#;
        let description = parse_description(xml, "http://192.168.1.20:9197/dmr/desc.xml").unwrap();
        assert_eq!(description.device_type, MEDIA_RENDERER);
        assert_eq!(description.name, "Living Room TV");
        assert_eq!(description.model.as_deref(), Some("UE55TU7000"));
        assert_eq!(description.udn, "uuid:renderer-1");
        assert_eq!(
            description.av_transport.as_deref(),
            Some("http://192.168.1.20:9197/dmr/upnp/control/AVTransport1")
        );
        assert_eq!(
            description.rendering_control.as_deref(),
            Some("http://192.168.1.20:9197/upnp/control/RenderingControl1")
        );
        assert!(description.is_renderer());
    }

    #[test]
    fn url_base_and_a_dial_device() {
        let xml = r#"<root xmlns="urn:schemas-upnp-org:device-1-0">
  <URLBase>http://192.168.1.30:8008</URLBase>
  <device>
    <deviceType>urn:dial-multiscreen-org:device:dial:1</deviceType>
    <friendlyName>Bedroom</friendlyName>
    <manufacturer>Google Inc.</manufacturer>
    <modelName>Eureka Dongle</modelName>
    <UDN>uuid:3e1cc7c0-f4f4-f4f4</UDN>
    <serviceList>
      <service>
        <serviceType>urn:dial-multiscreen-org:service:dial:1</serviceType>
        <controlURL>/ssdp/notfound</controlURL>
      </service>
    </serviceList>
  </device>
</root>"#;
        let description =
            parse_description(xml, "http://192.168.1.30:8008/ssdp/device-desc.xml").unwrap();
        assert!(!description.is_renderer());
        assert!(description.is_google());
        assert_eq!(description.name, "Bedroom");
        assert_eq!(description.model.as_deref(), Some("Eureka Dongle"));
        assert_eq!(
            parse_description("<html>not a device</html>", "http://a/"),
            None
        );
        assert_eq!(
            resolve("http://a.example/x/desc.xml", "ctl"),
            Some("http://a.example/x/ctl".into())
        );
        assert_eq!(resolve("http://a.example/", "file:///x"), None);
    }
}
