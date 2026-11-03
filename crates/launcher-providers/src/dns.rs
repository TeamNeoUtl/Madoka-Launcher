use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, OnceLock};

use hickory_resolver::TokioResolver;
use hickory_resolver::config::{ResolverConfig, ServerGroup};
use hickory_resolver::net::runtime::TokioRuntimeProvider;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use tracing::warn;

/// Addresses of the `AdGuard` DNS default servers.
pub const ADGUARD_DNS: [IpAddr; 4] = [
    IpAddr::V4(Ipv4Addr::new(94, 140, 14, 14)),
    IpAddr::V4(Ipv4Addr::new(94, 140, 15, 15)),
    IpAddr::V6(Ipv6Addr::new(0x2a10, 0x50c0, 0, 0, 0, 0, 0xad1, 0xff)),
    IpAddr::V6(Ipv6Addr::new(0x2a10, 0x50c0, 0, 0, 0, 0, 0xad2, 0xff)),
];

/// Name resolution of every request that a `ReqwestHttp` or `ReqwestFetcher` sends.
/// The resolver starts at the first lookup.
/// A failed build or lookup falls back to the system resolver.
#[derive(Clone)]
pub struct Dns {
    servers: Arc<[IpAddr]>,
    resolver: Arc<OnceLock<Option<TokioResolver>>>,
}

impl Default for Dns {
    fn default() -> Self {
        Self::new(&[])
    }
}

impl Dns {
    /// Uses `servers`, or `ADGUARD_DNS` when `servers` is empty.
    #[must_use]
    pub fn new(servers: &[IpAddr]) -> Self {
        let servers: Arc<[IpAddr]> = if servers.is_empty() {
            Arc::from(ADGUARD_DNS.as_slice())
        } else {
            Arc::from(servers)
        };
        Self {
            servers,
            resolver: Arc::new(OnceLock::new()),
        }
    }

    fn resolver(&self) -> Option<&TokioResolver> {
        self.resolver
            .get_or_init(|| {
                let group = ServerGroup {
                    ips: &self.servers,
                    server_name: "",
                    path: "",
                };
                TokioResolver::builder_with_config(
                    ResolverConfig::udp_and_tcp(&group),
                    TokioRuntimeProvider::default(),
                )
                .build()
                .inspect_err(|error| warn!(%error, "DNS resolver build failed"))
                .ok()
            })
            .as_ref()
    }

    async fn lookup_configured(&self, host: &str) -> Option<Vec<SocketAddr>> {
        let resolver = self.resolver()?;
        match resolver.lookup_ip(host).await {
            Ok(lookup) => Some(lookup.iter().map(|ip| SocketAddr::new(ip, 0)).collect()),
            Err(error) => {
                warn!(host, %error, "configured DNS failed");
                None
            }
        }
    }

    async fn lookup(&self, host: &str) -> io::Result<Vec<SocketAddr>> {
        if let Some(addrs) = self.lookup_configured(host).await {
            return Ok(addrs);
        }
        Ok(tokio::net::lookup_host((host, 0)).await?.collect())
    }
}

impl Resolve for Dns {
    fn resolve(&self, name: Name) -> Resolving {
        let dns = self.clone();
        Box::pin(async move {
            let addrs = dns.lookup(name.as_str()).await?;
            Ok(Box::new(addrs.into_iter()) as Addrs)
        })
    }
}
