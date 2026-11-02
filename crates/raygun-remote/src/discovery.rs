//! Finding Raygun devices to play on (M46.15): a device that lets others play on it advertises
//! `_raygun-connect._tcp.local.` while its remote server runs, and Connect browses for them.
//!
//! One mDNS daemon serves both, started on first use.

use std::net::IpAddr;
use std::sync::OnceLock;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

pub const SERVICE: &str = "_raygun-connect._tcp.local.";

/// A device heard on the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub id: String,
    pub name: String,
    /// `phone` or `computer`.
    pub kind: String,
    /// `ip:port`.
    pub address: String,
}

fn daemon() -> Result<&'static ServiceDaemon, String> {
    static DAEMON: OnceLock<Result<ServiceDaemon, String>> = OnceLock::new();
    DAEMON
        .get_or_init(|| ServiceDaemon::new().map_err(|error| error.to_string()))
        .as_ref()
        .map_err(Clone::clone)
}

/// This device on the network while it's kept; dropping it withdraws it.
pub struct Advertised {
    fullname: String,
}

/// Advertises this device at `ip:port` as one others can play on.
pub fn advertise(
    id: &str,
    name: &str,
    kind: &str,
    ip: IpAddr,
    port: u16,
) -> Result<Advertised, String> {
    let daemon = daemon()?;
    // Instance and host names must be DNS labels; the id is one.
    let instance = format!("raygun-{id}");
    let info = ServiceInfo::new(
        SERVICE,
        &instance,
        &format!("{instance}.local."),
        ip,
        port,
        [("id", id), ("name", name), ("kind", kind)].as_slice(),
    )
    .map_err(|error| error.to_string())?;
    let fullname = info.get_fullname().to_owned();
    daemon.register(info).map_err(|error| error.to_string())?;
    Ok(Advertised { fullname })
}

impl Drop for Advertised {
    fn drop(&mut self) {
        if let Ok(daemon) = daemon() {
            let _ = daemon.unregister(&self.fullname);
        }
    }
}

/// Browses for devices; `found` is called from a background thread for each one resolved, and
/// `lost(id)` when one withdraws. Runs for the rest of the process.
pub fn browse(
    own_id: String,
    found: impl Fn(Found) + Send + 'static,
    lost: impl Fn(String) + Send + 'static,
) -> Result<(), String> {
    let events = daemon()?
        .browse(SERVICE)
        .map_err(|error| error.to_string())?;
    std::thread::Builder::new()
        .name("connect-discovery".into())
        .spawn(move || {
            while let Ok(event) = events.recv() {
                match event {
                    ServiceEvent::ServiceResolved(service) => {
                        let Some(id) = service.get_property_val_str("id") else {
                            continue;
                        };
                        if id == own_id {
                            continue;
                        }
                        let Some(ip) = service.get_addresses_v4().into_iter().next() else {
                            continue;
                        };
                        found(Found {
                            id: id.to_owned(),
                            name: service
                                .get_property_val_str("name")
                                .unwrap_or("Raygun")
                                .to_owned(),
                            kind: service
                                .get_property_val_str("kind")
                                .unwrap_or("computer")
                                .to_owned(),
                            address: format!("{ip}:{}", service.get_port()),
                        });
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        // `raygun-<id>._raygun-connect._tcp.local.`
                        if let Some(id) = fullname
                            .strip_prefix("raygun-")
                            .and_then(|rest| rest.strip_suffix(&format!(".{SERVICE}")))
                        {
                            lost(id.to_owned());
                        }
                    }
                    _ => {}
                }
            }
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}
