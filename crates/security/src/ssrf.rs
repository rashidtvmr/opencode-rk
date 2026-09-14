//! SSRF prevention guard - blocks tool calls hitting internal/private IPs.
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SsrfBlocked {
    pub address: String,
}

impl std::fmt::Display for SsrfBlocked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SSRF blocked: {} is a private/internal address",
            self.address
        )
    }
}

impl std::error::Error for SsrfBlocked {}

#[derive(Clone, Debug)]
pub struct SsrfGuard {
    allow_localhost: bool,
}

impl SsrfGuard {
    #[must_use]
    pub fn new() -> Self {
        Self {
            allow_localhost: false,
        }
    }

    pub fn allow_localhost(&mut self, allow: bool) {
        self.allow_localhost = allow;
    }

    pub fn check_url(&self, url: &str) -> Result<(), SsrfBlocked> {
        let host = extract_host(url)?;
        if let Ok(ip) = host.parse::<IpAddr>() {
            return self.check_ip(ip);
        }
        let lower = host.to_lowercase();
        if lower == "localhost" || lower.ends_with(".local") || lower == "[::1]" {
            if self.allow_localhost {
                return Ok(());
            }
            return Err(SsrfBlocked { address: host });
        }
        Err(SsrfBlocked { address: host })
    }

    pub fn check_ip(&self, ip: IpAddr) -> Result<(), SsrfBlocked> {
        let ip_str = ip.to_string();
        if !self.allow_localhost && ip.is_loopback() {
            return Err(SsrfBlocked { address: ip_str });
        }
        if let IpAddr::V4(v4) = ip {
            if is_in_cidr(v4, Ipv4Addr::new(10, 0, 0, 0), 8) {
                return Err(SsrfBlocked { address: ip_str });
            }
            if is_in_cidr(v4, Ipv4Addr::new(172, 16, 0, 0), 12) {
                return Err(SsrfBlocked { address: ip_str });
            }
            if is_in_cidr(v4, Ipv4Addr::new(192, 168, 0, 0), 16) {
                return Err(SsrfBlocked { address: ip_str });
            }
            if is_in_cidr(v4, Ipv4Addr::new(169, 254, 0, 0), 16) {
                return Err(SsrfBlocked { address: ip_str });
            }
        }
        if let IpAddr::V6(v6) = ip {
            if is_in_ipv6_cidr(v6, 0xfd00, 8) {
                return Err(SsrfBlocked { address: ip_str });
            }
            if is_in_ipv6_cidr(v6, 0xfe80, 10) {
                return Err(SsrfBlocked { address: ip_str });
            }
        }
        Ok(())
    }
}

impl Default for SsrfGuard {
    fn default() -> Self {
        Self::new()
    }
}

fn extract_host(url: &str) -> Result<String, SsrfBlocked> {
    let scheme_sep = url.find("://").ok_or_else(|| SsrfBlocked {
        address: url.to_owned(),
    })?;
    let after_scheme = &url[scheme_sep + 3..];
    let host_end = after_scheme
        .find(&['/', ':'][..])
        .unwrap_or(after_scheme.len());
    let host = &after_scheme[..host_end];
    if host.is_empty() {
        return Err(SsrfBlocked {
            address: url.to_owned(),
        });
    }
    Ok(host.to_owned())
}

fn is_in_cidr(ip: Ipv4Addr, prefix: Ipv4Addr, prefix_len: u8) -> bool {
    if prefix_len >= 32 {
        return ip == prefix;
    }
    let shift = 32 - prefix_len;
    u32::from(ip) >> shift == u32::from(prefix) >> shift
}

fn is_in_ipv6_cidr(ip: Ipv6Addr, prefix_value: u16, prefix_len: u8) -> bool {
    let first_seg = ip.segments()[0] as u16;
    if prefix_len >= 16 {
        return first_seg == prefix_value;
    }
    let shift = 16 - prefix_len;
    (first_seg >> shift) == (prefix_value >> shift)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard() -> SsrfGuard {
        SsrfGuard::new()
    }

    fn guard_with_localhost() -> SsrfGuard {
        let mut g = SsrfGuard::new();
        g.allow_localhost(true);
        g
    }

    #[test]
    fn blocks_localhost() {
        let g = guard();
        assert!(g.check_ip("127.0.0.1".parse().unwrap()).is_err());
        assert!(g.check_ip("127.255.255.254".parse().unwrap()).is_err());
        assert!(g.check_url("http://localhost/").is_err());
        assert!(g.check_url("http://127.0.0.1:8080/").is_err());
    }

    #[test]
    fn blocks_private_ranges() {
        let g = guard();
        assert!(g.check_ip("10.0.0.1".parse().unwrap()).is_err());
        assert!(g.check_ip("10.255.255.255".parse().unwrap()).is_err());
        assert!(g.check_ip("172.16.0.1".parse().unwrap()).is_err());
        assert!(g.check_ip("172.31.255.255".parse().unwrap()).is_err());
        assert!(g.check_ip("192.168.1.1".parse().unwrap()).is_err());
        assert!(g.check_ip("192.168.255.255".parse().unwrap()).is_err());
        assert!(g.check_ip("169.254.1.1".parse().unwrap()).is_err());
        assert!(g.check_ip("172.15.255.255".parse().unwrap()).is_ok());
        assert!(g.check_ip("172.32.0.1".parse().unwrap()).is_ok());
    }

    #[test]
    fn allows_public() {
        let g = guard();
        assert!(g.check_ip("8.8.8.8".parse().unwrap()).is_ok());
        assert!(g.check_ip("1.1.1.1".parse().unwrap()).is_ok());
        assert!(g.check_ip("93.184.216.34".parse().unwrap()).is_ok());
        assert!(g.check_url("https://1.1.1.1/").is_ok());
        assert!(g.check_url("https://8.8.8.8/dns-query").is_ok());
        // Hostnames without DNS resolution: conservative block
        assert!(g.check_url("https://example.com/").is_err());
        assert!(g.check_url("https://google.com/").is_err());
    }

    #[test]
    fn blocks_ipv6_loopback() {
        let g = guard();
        assert!(g.check_ip("::1".parse().unwrap()).is_err());
        assert!(g.check_ip("fe80::1".parse().unwrap()).is_err());
        assert!(g.check_ip("fd00::1".parse().unwrap()).is_err());
        assert!(g.check_ip("fd12:3456:789a::1".parse().unwrap()).is_err());
    }

    #[test]
    fn dev_mode_allows_localhost() {
        let g = guard_with_localhost();
        assert!(g.check_ip("127.0.0.1".parse().unwrap()).is_ok());
        assert!(g.check_ip("::1".parse().unwrap()).is_ok());
        assert!(g.check_url("http://localhost/").is_ok());
        // Private ranges still blocked in dev mode
        assert!(g.check_ip("10.0.0.1".parse().unwrap()).is_err());
        assert!(g.check_ip("192.168.1.1".parse().unwrap()).is_err());
    }

    #[test]
    fn ipv6_public_allowed() {
        let g = guard();
        assert!(g.check_ip("2001:4860:4860::8888".parse().unwrap()).is_ok());
    }

    #[test]
    fn cidr_edge_cases() {
        let g = guard();
        assert!(g.check_ip("9.255.255.255".parse().unwrap()).is_ok());
        assert!(g.check_ip("10.0.0.0".parse().unwrap()).is_err());
        assert!(g.check_ip("172.15.255.255".parse().unwrap()).is_ok());
        assert!(g.check_ip("172.16.0.0".parse().unwrap()).is_err());
        assert!(g.check_ip("172.31.255.255".parse().unwrap()).is_err());
        assert!(g.check_ip("172.32.0.0".parse().unwrap()).is_ok());
        assert!(g.check_ip("192.167.255.255".parse().unwrap()).is_ok());
        assert!(g.check_ip("192.168.0.0".parse().unwrap()).is_err());
    }

    #[test]
    fn ssrf_blocked_display() {
        let e = SsrfBlocked {
            address: "127.0.0.1".to_owned(),
        };
        assert!(e.to_string().contains("127.0.0.1"));
    }
}
