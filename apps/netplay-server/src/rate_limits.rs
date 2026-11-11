//! Per-client-IP rate limits (`tower-governor`). Behind nginx every socket peer is
//! nginx itself, so the key is `X-Real-IP` **only** when the socket peer is inside
//! `TRUSTED_PROXY_CIDR`; anyone else is keyed by its socket address (a direct client
//! cannot pick its own bucket by sending the header). IPv6 clients are keyed by their
//! /64 (one home or VPS gets one; rotating addresses inside it buys nothing).

use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request};
use governor::middleware::NoOpMiddleware;
use ipnet::IpNet;
use tower_governor::governor::{GovernorConfig, GovernorConfigBuilder};
use tower_governor::key_extractor::KeyExtractor;
use tower_governor::GovernorError;

const X_REAL_IP: &str = "x-real-ip";

/// The client key used for rate limiting: IPv4 address, or IPv6 /64 network address.
pub fn client_ip(headers: &HeaderMap, socket_peer: IpAddr, trusted: &[IpNet]) -> IpAddr {
    let socket_peer = unmap_v4(socket_peer);
    if !trusted.iter().any(|net| net.contains(&socket_peer)) {
        return rate_key(socket_peer);
    }
    let client = headers
        .get(X_REAL_IP)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(socket_peer);
    rate_key(unmap_v4(client))
}

/// `::ffff:a.b.c.d` → `a.b.c.d`.
fn unmap_v4(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        IpAddr::V4(_) => ip,
    }
}

/// Native IPv6 masked to its /64; IPv4 unchanged.
fn rate_key(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => IpAddr::V6(Ipv6Addr::from(u128::from(v6) & (u128::MAX << 64))),
        IpAddr::V4(_) => ip,
    }
}

/// `KeyExtractor` over [`client_ip`]; needs `ConnectInfo<SocketAddr>` on the request.
#[derive(Clone)]
pub struct ClientIpKey {
    trusted: Arc<[IpNet]>,
}

impl KeyExtractor for ClientIpKey {
    type Key = IpAddr;

    fn extract<T>(&self, req: &Request<T>) -> Result<IpAddr, GovernorError> {
        let peer = req
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|info| info.0.ip())
            .ok_or(GovernorError::UnableToExtractKey)?;
        Ok(client_ip(req.headers(), peer, &self.trusted))
    }
}

/// One limiter config (shared bucket map) per guarded route group.
pub type RouteLimit = Arc<GovernorConfig<ClientIpKey, NoOpMiddleware>>;

/// All limits: create 10 / min, join 30 / min, WebSocket connects 20 / min, lobby
/// status + start 60 / min (a 5 s poll uses 12; the cap stops code enumeration).
#[derive(Clone)]
pub struct RateLimits {
    pub create: RouteLimit,
    pub join: RouteLimit,
    pub ws: RouteLimit,
    pub status: RouteLimit,
}

impl RateLimits {
    pub fn new(trusted: &[IpNet]) -> Self {
        let key = ClientIpKey {
            trusted: trusted.into(),
        };
        Self {
            create: per_minute(&key, 10),
            join: per_minute(&key, 30),
            ws: per_minute(&key, 20),
            status: per_minute(&key, 60),
        }
    }

    /// Drops idle buckets (called by the GC task) so the maps do not grow forever.
    pub fn retain_recent(&self) {
        for limit in [&self.create, &self.join, &self.ws, &self.status] {
            limit.limiter().retain_recent();
        }
    }
}

/// `count` requests per minute, all of them usable as a burst.
fn per_minute(key: &ClientIpKey, count: u32) -> RouteLimit {
    let config = GovernorConfigBuilder::default()
        .key_extractor(key.clone())
        .period(Duration::from_secs(60) / count)
        .burst_size(count)
        .finish()
        .unwrap_or_else(|| unreachable!("period and burst are non-zero"));
    Arc::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trusted() -> Vec<IpNet> {
        crate::server_config::parse_cidr_list("127.0.0.1/32,172.16.0.0/12").unwrap()
    }

    fn headers(real_ip: &str) -> HeaderMap {
        let mut map = HeaderMap::new();
        map.insert(X_REAL_IP, real_ip.parse().unwrap());
        map
    }

    #[test]
    fn header_is_used_only_from_trusted_proxies() {
        let nginx: IpAddr = "127.0.0.1".parse().unwrap();
        let bridge: IpAddr = "172.18.0.1".parse().unwrap();
        let stranger: IpAddr = "203.0.113.9".parse().unwrap();
        let h = headers("198.51.100.7");
        assert_eq!(client_ip(&h, nginx, &trusted()).to_string(), "198.51.100.7");
        assert_eq!(
            client_ip(&h, bridge, &trusted()).to_string(),
            "198.51.100.7"
        );
        assert_eq!(client_ip(&h, stranger, &trusted()), stranger);
        assert_eq!(client_ip(&headers("junk"), nginx, &trusted()), nginx);
        assert_eq!(client_ip(&HeaderMap::new(), nginx, &trusted()), nginx);
    }

    #[test]
    fn ipv6_clients_share_a_bucket_per_64() {
        let nginx: IpAddr = "127.0.0.1".parse().unwrap();
        let key = |real: &str| client_ip(&headers(real), nginx, &trusted());
        assert_eq!(key("2001:db8:1:2::1"), key("2001:db8:1:2:ffff::9"));
        assert_eq!(key("2001:db8:1:2::1").to_string(), "2001:db8:1:2::");
        assert_ne!(key("2001:db8:1:2::1"), key("2001:db8:1:3::1"));
        assert_eq!(key("::ffff:198.51.100.7").to_string(), "198.51.100.7");
        let mapped_nginx: IpAddr = "::ffff:127.0.0.1".parse().unwrap();
        let h = headers("198.51.100.7");
        assert_eq!(
            client_ip(&h, mapped_nginx, &trusted()).to_string(),
            "198.51.100.7",
            "v4-mapped proxy address is still trusted"
        );
    }
}
