//! Server settings from the environment (`.env` beside docker-compose on the VPS).

use std::net::SocketAddr;

use anyhow::{bail, Context};
use ipnet::IpNet;

/// Default bind: loopback only, nginx fronts it.
pub const DEFAULT_BIND: &str = "127.0.0.1:3536";
/// Loopback + docker bridge networks (where nginx's connections come from).
pub const DEFAULT_TRUSTED_PROXY_CIDR: &str = "127.0.0.1/32,172.16.0.0/12";
const MIN_SERVER_SECRET_LEN: usize = 32;
const MIN_TURN_SECRET_LEN: usize = 16;

/// Everything the server reads at start.
#[derive(Clone)]
pub struct ServerConfig {
    pub bind: SocketAddr,
    /// HMAC key for room tokens (`NETPLAY_SERVER_SECRET`, ≥ 32 bytes).
    pub server_secret: Vec<u8>,
    /// coturn `static-auth-secret` (`TURN_SECRET`).
    pub turn_secret: String,
    /// Host name in the STUN / TURN urls (`TURN_HOST`).
    pub turn_host: String,
    /// Socket peers allowed to set `X-Real-IP` (`TRUSTED_PROXY_CIDR`, comma list).
    pub trusted_proxies: Vec<IpNet>,
    /// Global room cap (`MAX_ROOMS`).
    pub max_rooms: usize,
    /// Spectators per room (`MAX_SPECTATORS`).
    pub max_spectators: usize,
}

impl std::fmt::Debug for ServerConfig {
    // Secrets never reach a log line, even through `{:?}`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerConfig")
            .field("bind", &self.bind)
            .field("turn_host", &self.turn_host)
            .field("trusted_proxies", &self.trusted_proxies)
            .field("max_rooms", &self.max_rooms)
            .field("max_spectators", &self.max_spectators)
            .finish_non_exhaustive()
    }
}

impl ServerConfig {
    /// Reads the process environment.
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Reads settings through `lookup` (tests pass a map).
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> anyhow::Result<Self> {
        let get = |name: &str| lookup(name).filter(|v| !v.trim().is_empty());
        let server_secret =
            get("NETPLAY_SERVER_SECRET").context("NETPLAY_SERVER_SECRET is required")?;
        if server_secret.len() < MIN_SERVER_SECRET_LEN {
            bail!("NETPLAY_SERVER_SECRET must be at least {MIN_SERVER_SECRET_LEN} bytes");
        }
        let turn_secret = get("TURN_SECRET").context("TURN_SECRET is required")?;
        if turn_secret.len() < MIN_TURN_SECRET_LEN {
            bail!("TURN_SECRET must be at least {MIN_TURN_SECRET_LEN} bytes");
        }
        let bind = get("NETPLAY_BIND").unwrap_or_else(|| DEFAULT_BIND.to_owned());
        let cidrs =
            get("TRUSTED_PROXY_CIDR").unwrap_or_else(|| DEFAULT_TRUSTED_PROXY_CIDR.to_owned());
        Ok(Self {
            bind: bind.parse().context("NETPLAY_BIND is not an ip:port")?,
            server_secret: server_secret.into_bytes(),
            turn_secret,
            turn_host: get("TURN_HOST").unwrap_or_else(|| "netplay.nostyra.com".to_owned()),
            trusted_proxies: parse_cidr_list(&cidrs)?,
            max_rooms: parse_count(get("MAX_ROOMS"), 500, "MAX_ROOMS")?,
            max_spectators: parse_count(get("MAX_SPECTATORS"), 8, "MAX_SPECTATORS")?,
        })
    }
}

/// `"127.0.0.1/32, 172.16.0.0/12"` → networks; a bare address counts as a /32 (/128).
pub fn parse_cidr_list(text: &str) -> anyhow::Result<Vec<IpNet>> {
    text.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<IpNet>()
                .or_else(|_| s.parse::<std::net::IpAddr>().map(IpNet::from))
                .with_context(|| format!("TRUSTED_PROXY_CIDR entry {s:?} is not a CIDR"))
        })
        .collect()
}

fn parse_count(value: Option<String>, default: usize, name: &str) -> anyhow::Result<usize> {
    match value {
        None => Ok(default),
        Some(v) => v
            .trim()
            .parse()
            .with_context(|| format!("{name} is not a number")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config(pairs: &[(&str, &str)]) -> anyhow::Result<ServerConfig> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        ServerConfig::from_lookup(|name| map.get(name).cloned())
    }

    const SECRETS: [(&str, &str); 2] = [
        ("NETPLAY_SERVER_SECRET", "0123456789abcdef0123456789abcdef"),
        ("TURN_SECRET", "turn-secret-16by"),
    ];

    #[test]
    fn defaults_apply() {
        let cfg = config(&SECRETS).unwrap();
        assert_eq!(cfg.bind.to_string(), DEFAULT_BIND);
        assert_eq!(cfg.max_rooms, 500);
        assert_eq!(cfg.max_spectators, 8);
        assert_eq!(cfg.trusted_proxies.len(), 2);
        assert!(
            !format!("{cfg:?}").contains("0123456789abcdef"),
            "Debug hides secrets"
        );
    }

    #[test]
    fn short_or_missing_secrets_are_refused() {
        assert!(config(&[SECRETS[1]]).is_err());
        assert!(config(&[("NETPLAY_SERVER_SECRET", "short"), SECRETS[1]]).is_err());
        assert!(config(&[SECRETS[0], ("TURN_SECRET", "short")]).is_err());
    }

    #[test]
    fn cidr_list_accepts_bare_addresses_and_rejects_junk() {
        let nets = parse_cidr_list("10.0.0.1, ::1/128,192.168.0.0/16").unwrap();
        assert_eq!(nets.len(), 3);
        assert!(nets[0].contains(&"10.0.0.1".parse::<std::net::IpAddr>().unwrap()));
        assert!(parse_cidr_list("not-a-cidr").is_err());
    }
}
