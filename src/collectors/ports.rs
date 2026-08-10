use crate::models::{PortBinding, Protocol};
use anyhow::Result;
use netstat2::{AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState, get_sockets_info};
use sysinfo::{Pid, ProcessesToUpdate, System};

pub fn collect(port_filter: Option<u16>) -> Result<Vec<PortBinding>> {
    let sockets = get_sockets_info(
        AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6,
        ProtocolFlags::TCP | ProtocolFlags::UDP,
    )?;

    // structures storage
    let mut bindings: Vec<PortBinding> = Vec::new();

    for socket in sockets {
        // select first pid from socket
        let pid: Option<u32> = socket.associated_pids.first().copied();

        match socket.protocol_socket_info {
            // --- TCP ---
            ProtocolSocketInfo::Tcp(tcp_info) => {
                // only LISTEN sockets
                if tcp_info.state != TcpState::Listen {
                    continue;
                }
                if let Some(wanted_port) = port_filter {
                    if tcp_info.local_port != wanted_port {
                        continue;
                    }
                }
                bindings.push(PortBinding {
                    port: tcp_info.local_port,
                    protocol: Protocol::Tcp,
                    address: tcp_info.local_addr.to_string(),
                    pid,
                    process_name: None,
                    container_name: None,
                    container_image: None,
                });
            }

            ProtocolSocketInfo::Udp(udp_info) => {
                // udp do not have state, if port is open - it in list
                if let Some(wanted_port) = port_filter {
                    if udp_info.local_port != wanted_port {
                        continue;
                    }
                }
                bindings.push(PortBinding {
                    port: udp_info.local_port,
                    protocol: Protocol::Udp,
                    address: udp_info.local_addr.to_string(),
                    pid,
                    process_name: None,
                    container_name: None,
                    container_image: None,
                });
            }
        }
    }

    enrich_with_process_names(&mut bindings);

    // sort, first - port, second - address
    bindings.sort_by(|left, right| {
        left.port
            .cmp(&right.port)
            .then(left.address.cmp(&right.address))
    });
    Ok(bindings)
}

// for each PortBinding with known PID appropriate process name
pub(crate) fn enrich_with_process_names(bindings: &mut [PortBinding]) {
    let pids: Vec<Pid> = bindings
        .iter()
        .filter_map(|binding| binding.pid)
        .map(Pid::from_u32)
        .collect();

    if pids.is_empty() {
        return;
    }

    let mut system = System::new();

    // update in sysinfo only necessary ports
    // not all from the whole operating system

    system.refresh_processes(ProcessesToUpdate::Some(&pids), true);

    for binding in bindings.iter_mut() {
        let Some(raw_pid) = binding.pid else {
            continue;
        };
        let pid = Pid::from_u32(raw_pid);

        if let Some(process) = system.process(pid) {
            let name = process.name().to_string_lossy().into_owned();
            binding.process_name = Some(name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn collect_returns_ok_and_sorted() {
        let bindings = collect(None).expect("collect ports");

        for window in bindings.windows(2) {
            let a = &window[0];
            let b = &window[1];
            assert!(
                (a.port, a.address.as_str()) <= (b.port, b.address.as_str()),
                "unsorted: {}:{} then {}:{}",
                a.port,
                a.address,
                b.port,
                b.address
            );
        }
    }

    #[test]
    fn collect_filter_finds_bound_tcp_port() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let port = listener.local_addr().expect("local address").port();

        let bindings = collect(Some(port)).expect("collect filtered ports");

        assert!(
            bindings
                .iter()
                .any(|b| { b.port == port && matches!(b.protocol, Protocol::Tcp) }),
            "expected TCP listen on port {port}, got: {:?}",
            bindings
                .iter()
                .map(|b| (b.port, format!("{:?}", b.protocol), b.address.clone()))
                .collect::<Vec<_>>()
        );

        assert!(
            bindings.iter().all(|b| b.port == port),
            "filter leaked other ports"
        );

        drop(listener);
    }

    #[test]
    fn collect_filter_unknown_port_has_no_tcp_listen() {
        let port = 65_530u16;
        let bindings = collect(Some(port)).expect("collect port");

        let tcp_hits = bindings
            .iter()
            .filter(|b| b.port == port && matches!(b.protocol, Protocol::Tcp))
            .count();

        assert_eq!(tcp_hits, 0)
    }

    #[test]
    fn enrich_with_process_names_fills_current_process() {
        let me = std::process::id();
        let mut bindings = vec![PortBinding {
            port: 9,
            protocol: Protocol::Tcp,
            address: "127.0.0.1".to_string(),
            pid: Some(me),
            process_name: None,
            container_name: None,
            container_image: None,
        }];

        enrich_with_process_names(&mut bindings);
        let name = bindings[0]
            .process_name
            .as_deref()
            .expect("process_name should be set for current pid");

        assert!(!name.is_empty());
    }

    #[test]
    fn enrich_with_process_names_skips_missing_pid() {
        let mut bindings = vec![PortBinding {
            port: 9,
            protocol: Protocol::Tcp,
            address: "127.0.0.1".to_string(),
            pid: None,
            process_name: None,
            container_name: None,
            container_image: None,
        }];

        enrich_with_process_names(&mut bindings);
        assert!(bindings[0].process_name.is_none());
    }

    #[test]
    fn enrich_with_process_names_leaves_unknown_pid() {
        let mut bindings = vec![PortBinding {
            port: 9,
            protocol: Protocol::Tcp,
            address: "127.0.0.1".to_string(),
            pid: Some(u32::MAX - 1),
            process_name: None,
            container_name: None,
            container_image: None,
        }];
        enrich_with_process_names(&mut bindings);
        assert!(bindings[0].process_name.is_none())
    }
}
