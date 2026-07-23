use sysmedic_core::snapshot::ListeningPort;
use sysmedic_core::{Collector, Snapshot};

use crate::util;

/// TCP state 0x0A == LISTEN (see include/net/tcp_states.h).
const TCP_LISTEN: &str = "0A";

pub struct PortsCollector;

impl Collector for PortsCollector {
    fn name(&self) -> &'static str {
        "ports"
    }

    fn collect(&self, snapshot: &mut Snapshot) {
        let mut ports = Vec::new();
        if let Some(v4) = util::read_file("/proc/net/tcp") {
            ports.extend(parse_proc_net_tcp(&v4, "tcp", false));
        }
        if let Some(v6) = util::read_file("/proc/net/tcp6") {
            ports.extend(parse_proc_net_tcp(&v6, "tcp6", true));
        }
        if ports.is_empty() {
            snapshot
                .collection_errors
                .push("ports: /proc/net/tcp not readable".into());
            return;
        }
        ports.sort_by_key(|p| (p.port, p.proto));
        ports.dedup_by_key(|p| (p.port, p.proto, p.exposed));
        snapshot.ports = Some(ports);
    }
}

/// Parse listening sockets from `/proc/net/tcp` or `/proc/net/tcp6`.
pub fn parse_proc_net_tcp(content: &str, proto: &'static str, v6: bool) -> Vec<ListeningPort> {
    content
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut cols = line.split_whitespace();
            let local = cols.nth(1)?; // field 1: local_address
            let state = cols.nth(1)?; // field 3: state (after local, rem)
            if state != TCP_LISTEN {
                return None;
            }
            let (hex_addr, hex_port) = local.split_once(':')?;
            let port = u16::from_str_radix(hex_port, 16).ok()?;
            let (address, loopback) = if v6 {
                decode_v6(hex_addr)
            } else {
                decode_v4(hex_addr)
            };
            Some(ListeningPort {
                proto,
                address,
                port,
                exposed: !loopback,
            })
        })
        .collect()
}

/// Decode a little-endian IPv4 hex address → (dotted string, is_loopback).
fn decode_v4(hex: &str) -> (String, bool) {
    let Ok(raw) = u32::from_str_radix(hex, 16) else {
        return (hex.to_string(), false);
    };
    // /proc stores the address little-endian: octets are the bytes in order.
    let b = raw.to_le_bytes(); // [o1, o2, o3, o4]
    let loopback = b[0] == 127;
    (format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]), loopback)
}

/// Decode a 32-hex-char IPv6 address → (string, is_loopback). We only need to
/// distinguish loopback (::1) and wildcard (::) from routable addresses.
fn decode_v6(hex: &str) -> (String, bool) {
    let all_zero = hex.chars().all(|c| c == '0');
    if all_zero {
        return ("::".to_string(), false); // wildcard — exposed
    }
    // ::1 loopback, stored with the low byte set in the last little-endian word.
    if hex.eq_ignore_ascii_case("00000000000000000000000001000000") {
        return ("::1".to_string(), true);
    }
    ("[ipv6]".to_string(), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    // sl local_address rem_address st ... (only fields 1 and 3 matter)
    const V4: &str = "\
  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt
   0: 00000000:0016 00000000:0000 0A 00000000:00000000 00:00000000 00000000
   1: 0100007F:0035 00000000:0000 0A 00000000:00000000 00:00000000 00000000
   2: 0100007F:C1B6 0100007F:0035 01 00000000:00000000 00:00000000 00000000
";

    #[test]
    fn extracts_listening_ports_only() {
        let ports = parse_proc_net_tcp(V4, "tcp", false);
        // Two LISTEN rows (state 0A); the ESTABLISHED (01) row is skipped.
        assert_eq!(ports.len(), 2);
    }

    #[test]
    fn wildcard_is_exposed_loopback_is_not() {
        let ports = parse_proc_net_tcp(V4, "tcp", false);
        let ssh = ports.iter().find(|p| p.port == 22).unwrap();
        assert_eq!(ssh.address, "0.0.0.0");
        assert!(ssh.exposed);
        let dns = ports.iter().find(|p| p.port == 53).unwrap();
        assert_eq!(dns.address, "127.0.0.1");
        assert!(!dns.exposed);
    }

    #[test]
    fn v6_wildcard_and_loopback() {
        let v6 = "  sl  local_address\n   0: 00000000000000000000000000000000:0050 00000000000000000000000000000000:0000 0A x\n   1: 00000000000000000000000001000000:0277 00000000000000000000000000000000:0000 0A x\n";
        let ports = parse_proc_net_tcp(v6, "tcp6", true);
        let http = ports.iter().find(|p| p.port == 80).unwrap();
        assert_eq!(http.address, "::");
        assert!(http.exposed);
        let local = ports.iter().find(|p| p.port == 631).unwrap();
        assert_eq!(local.address, "::1");
        assert!(!local.exposed);
    }
}
