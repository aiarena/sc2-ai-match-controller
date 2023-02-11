use crate::utilities::portpicker::Port;

use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo};

pub fn get_ipv4_port_for_pid(pid: u32) -> Option<Port> {
    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto_flags = ProtocolFlags::TCP | ProtocolFlags::UDP;
    let sockets_info = get_sockets_info(af_flags, proto_flags).unwrap_or_default();

    let process = sockets_info
        .into_iter()
        .find(|socket_info| socket_info.associated_pids.contains(&pid));
    process.and_then(|p| match p.protocol_socket_info {
        ProtocolSocketInfo::Tcp(tcp) => Some(tcp.local_port),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use crate::portpicker;
    use std::net::TcpListener;

    use crate::procs::tcp_port::get_ipv4_port_for_pid;

    #[test]
    fn test_tcp_port() {
        // assert!(test_ip_port("0.0.0.0"));
        // assert!(test_ip_port("127.0.0.1"));
        // assert!(test_ip_port("127.0.0.1"));
    }
    #[allow(unused)]
    fn test_ip_port(host: &str) -> bool {
        let port = portpicker::pick_unused_port().unwrap();

        let address = format!("{host}:{port}");
        let _listener = TcpListener::bind(address).unwrap();
        let pid = std::process::id();

        if let Some(found_port) = get_ipv4_port_for_pid(pid) {
            found_port == port
        } else {
            false
        }
    }
}
