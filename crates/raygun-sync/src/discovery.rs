//! Finding paired devices on the local network with mDNS (`_raygun-sync._tcp.local.`), so their
//! addresses stay current when DHCP hands out new ones.

use std::net::IpAddr;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

const SERVICE: &str = "_raygun-sync._tcp.local.";

/// Advertises this device and reports every other Raygun device it hears of. Dropping it stops
/// both.
pub struct Discovery {
    daemon: ServiceDaemon,
}

impl Discovery {
    /// Starts advertising `device_id` at `ip:port`; `found(device_id, "ip:port")` is called from a
    /// background thread for each device resolved.
    pub fn start(
        device_id: &str,
        ip: IpAddr,
        port: u16,
        found: impl Fn(String, String) + Send + 'static,
    ) -> Result<Self, mdns_sd::Error> {
        let daemon = ServiceDaemon::new()?;
        // Instance and host names must be DNS labels; the device id is.
        let instance = format!("raygun-{device_id}");
        let info = ServiceInfo::new(
            SERVICE,
            &instance,
            &format!("{instance}.local."),
            ip,
            port,
            [("id", device_id)].as_slice(),
        )?;
        daemon.register(info)?;
        let events = daemon.browse(SERVICE)?;
        let own = device_id.to_owned();
        std::thread::Builder::new()
            .name("sync-discovery".into())
            .spawn(move || {
                while let Ok(event) = events.recv() {
                    if let ServiceEvent::ServiceResolved(service) = event {
                        let Some(id) = service.get_property_val_str("id") else {
                            continue;
                        };
                        if id == own {
                            continue;
                        }
                        if let Some(address) = service.get_addresses_v4().into_iter().next() {
                            found(id.to_owned(), format!("{address}:{}", service.get_port()));
                        }
                    }
                }
            })
            .map_err(|error| mdns_sd::Error::Msg(error.to_string()))?;
        Ok(Self { daemon })
    }
}

impl Drop for Discovery {
    fn drop(&mut self) {
        let _ = self.daemon.shutdown();
    }
}
