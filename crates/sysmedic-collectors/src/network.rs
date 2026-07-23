use sysmedic_core::snapshot::NetworkInfo;
use sysmedic_core::{Collector, Snapshot};

use crate::util;

pub struct NetworkCollector;

impl Collector for NetworkCollector {
    fn name(&self) -> &'static str {
        "network"
    }

    fn collect(&self, snapshot: &mut Snapshot) {
        let route = util::read_file("/proc/net/route");
        let resolv = util::read_file("/etc/resolv.conf");
        match (route, resolv) {
            (Some(route), resolv) => {
                snapshot.network = Some(NetworkInfo {
                    has_default_route: parse_has_default_route(&route),
                    dns_servers: resolv.map(|r| parse_resolv(&r)).unwrap_or_default(),
                });
            }
            _ => snapshot
                .collection_errors
                .push("network: /proc/net/route not readable".into()),
        }
    }
}

/// A default route has destination `00000000` in `/proc/net/route`.
pub fn parse_has_default_route(route: &str) -> bool {
    route.lines().skip(1).any(|line| {
        line.split_whitespace()
            .nth(1)
            .is_some_and(|dest| dest == "00000000")
    })
}

pub fn parse_resolv(resolv: &str) -> Vec<String> {
    resolv
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix("nameserver")
                .map(|rest| rest.trim().to_string())
        })
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_default_route() {
        let fixture =
            "Iface\tDestination\tGateway\neth0\t00000000\t0100007F\neth0\t0000A8C0\t00000000\n";
        assert!(parse_has_default_route(fixture));
        let no_default = "Iface\tDestination\tGateway\neth0\t0000A8C0\t00000000\n";
        assert!(!parse_has_default_route(no_default));
    }

    #[test]
    fn parses_nameservers() {
        let fixture = "# comment\nnameserver 127.0.0.53\nnameserver 1.1.1.1\nsearch lan\n";
        assert_eq!(parse_resolv(fixture), vec!["127.0.0.53", "1.1.1.1"]);
    }
}
