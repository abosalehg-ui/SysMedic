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
        // UDP too: the security audit describes itself as "services listening
        // on the network", and a TCP-only sweep silently missed everything
        // that speaks UDP — mDNS/avahi (5353), a resolver on 53, SSDP (1900),
        // WireGuard, a game or media server.
        if let Some(v4) = util::read_file("/proc/net/udp") {
            ports.extend(parse_proc_net_udp(&v4, "udp", false));
        }
        if let Some(v6) = util::read_file("/proc/net/udp6") {
            ports.extend(parse_proc_net_udp(&v6, "udp6", true));
        }
        if ports.is_empty() {
            snapshot
                .collection_errors
                .push("ports: /proc/net/tcp not readable".into());
            return;
        }
        // Sort on the full dedup key. Sorting on `(port, proto)` alone left
        // rows with the same port but different `exposed` interleaved, and
        // `dedup_by_key` only collapses *adjacent* equals — so duplicates
        // survived depending on the order the kernel happened to list them.
        ports.sort_by(|a, b| {
            (a.port, a.proto, a.exposed, &a.address).cmp(&(b.port, b.proto, b.exposed, &b.address))
        });
        ports.dedup_by_key(|p| (p.port, p.proto, p.exposed));
        snapshot.ports = Some(ports);
    }
}

/// Parse listening sockets from `/proc/net/tcp` or `/proc/net/tcp6`.
pub fn parse_proc_net_tcp(content: &str, proto: &'static str, v6: bool) -> Vec<ListeningPort> {
    parse_proc_net(content, proto, v6, |state| state == TCP_LISTEN)
}

/// Parse bound sockets from `/proc/net/udp` or `/proc/net/udp6`.
///
/// UDP has no LISTEN state — an unconnected bound socket sits in `07`
/// (TCP_CLOSE) and a connected one in `01`. Every row in this table is a
/// socket bound to the local address and port shown, which is exactly what
/// the exposure audit is about, so the state is not filtered on.
pub fn parse_proc_net_udp(content: &str, proto: &'static str, v6: bool) -> Vec<ListeningPort> {
    parse_proc_net(content, proto, v6, |_| true)
}

/// Shared row parser for the `/proc/net/{tcp,udp}{,6}` tables, which have the
/// same leading columns: `sl local_address rem_address st …`.
fn parse_proc_net(
    content: &str,
    proto: &'static str,
    v6: bool,
    keep_state: fn(&str) -> bool,
) -> Vec<ListeningPort> {
    content
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut cols = line.split_whitespace();
            let local = cols.nth(1)?; // field 1: local_address
            let state = cols.nth(1)?; // field 3: state (after local, rem)
            if !keep_state(state) {
                return None;
            }
            let (hex_addr, hex_port) = local.split_once(':')?;
            let port = u16::from_str_radix(hex_port, 16).ok()?;
            // Port 0 is an unbound socket, not a service anyone can reach.
            if port == 0 {
                return None;
            }
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

/// Decode a 32-hex-char IPv6 address from `/proc/net/tcp6` → (string,
/// is_loopback). The address is four little-endian 32-bit words; we rebuild the
/// 16 bytes and classify with [`std::net::Ipv6Addr`], so `::1` and an
/// IPv4-mapped loopback (`::ffff:127.0.0.1`) are both correctly treated as
/// non-exposed, and routable addresses print properly instead of `[ipv6]`.
fn decode_v6(hex: &str) -> (String, bool) {
    if hex.len() != 32 {
        return (hex.to_string(), false);
    }
    let mut bytes = [0u8; 16];
    for word in 0..4 {
        let Ok(w) = u32::from_str_radix(&hex[word * 8..word * 8 + 8], 16) else {
            return (hex.to_string(), false);
        };
        // Same little-endian convention as decode_v4.
        bytes[word * 4..word * 4 + 4].copy_from_slice(&w.to_le_bytes());
    }
    let addr = std::net::Ipv6Addr::from(bytes);
    let loopback = addr.is_loopback()
        || addr
            .to_ipv4_mapped()
            .map(|v4| v4.is_loopback())
            .unwrap_or(false);
    (addr.to_string(), loopback)
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

    #[test]
    fn udp_sockets_are_collected_regardless_of_state() {
        // /proc/net/udp: an unconnected bound socket is state 07, a connected
        // one 01 — neither is LISTEN, so a TCP-shaped filter found nothing.
        // Rows: avahi on 5353 (all interfaces), a resolver on 127.0.0.53:53,
        // and an unbound socket on port 0.
        let udp = "\
  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt
   0: 00000000:14E9 00000000:0000 07 00000000:00000000 00:00000000 00000000
   1: 3500007F:0035 00000000:0000 07 00000000:00000000 00:00000000 00000000
   2: 0100007F:0044 0100007F:0043 01 00000000:00000000 00:00000000 00000000
   3: 00000000:0000 00000000:0000 07 00000000:00000000 00:00000000 00000000
";
        let ports = parse_proc_net_udp(udp, "udp", false);
        assert_eq!(ports.len(), 3, "port 0 is unbound and must be skipped");

        let mdns = ports.iter().find(|p| p.port == 5353).unwrap();
        assert_eq!(mdns.address, "0.0.0.0");
        assert!(mdns.exposed, "mDNS on all interfaces is network-exposed");
        assert_eq!(mdns.proto, "udp");

        let resolver = ports.iter().find(|p| p.port == 53).unwrap();
        assert_eq!(resolver.address, "127.0.0.53");
        assert!(!resolver.exposed);

        // A connected UDP socket (state 01) is still a bound local port.
        assert!(ports.iter().any(|p| p.port == 68));
    }

    #[test]
    fn tcp_still_keeps_only_listening_sockets() {
        // Widening the parser for UDP must not widen it for TCP.
        let ports = parse_proc_net_tcp(V4, "tcp", false);
        assert_eq!(ports.len(), 2);
        assert!(ports.iter().all(|p| p.port == 22 || p.port == 53));
    }

    #[test]
    fn v4_mapped_loopback_is_not_exposed() {
        // ::ffff:127.0.0.1 — stored as words 0,0,ffff (LE), 127.0.0.1 (LE).
        // Word 2 = 0x0000FFFF byte-reversed is ffff at bytes 10..12; word 3 is
        // 127.0.0.1. This used to be classified as exposed — a false positive.
        let v6 = "  sl  local_address\n   0: 0000000000000000FFFF00000100007F:0050 00000000000000000000000000000000:0000 0A x\n";
        let ports = parse_proc_net_tcp(v6, "tcp6", true);
        let p = ports.iter().find(|p| p.port == 80).unwrap();
        assert!(!p.exposed, "v4-mapped loopback must not be exposed");
    }
}
