#[cfg(any(target_os = "linux", target_os = "android"))]
use std::collections::HashSet;
#[cfg(target_os = "windows")]
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use crate::utilities::portpicker::Port;
#[cfg(target_os = "windows")]
use winapi::shared::tcpmib::MIB_TCPTABLE2;
#[cfg(target_os = "windows")]
use winapi::shared::tcpmib::MIB_TCP_STATE;
#[cfg(target_os = "windows")]
use winapi::shared::winerror::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
#[cfg(target_os = "windows")]
use winapi::um::{iphlpapi::GetTcpTable2, winsock2::ntohl, winsock2::ntohs};

#[cfg(target_os = "macos")]
use libproc::libproc::net_info::TcpSIState;
#[cfg(any(target_os = "linux", target_os = "android"))]
use procfs::net::TcpState;
#[cfg(any(target_os = "linux", target_os = "android"))]
use procfs::process::FDTarget;
#[cfg(any(target_os = "linux", target_os = "android"))]
use procfs::process::Process;

#[cfg(target_os = "windows")]
pub fn get_ipv4_port_for_pid(pid: u32) -> Option<Port> {
    get_tcp_entry_list()
        .unwrap_or_default()
        .iter()
        .find(|x| x.pid == pid)
        .map(|x| x.local_address.port())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn get_ipv4_port_for_pid(pid: u32) -> Option<Port> {
    if let Ok(proc) = Process::new(pid as i32) {
        let mut inodes = HashSet::new();

        if let Ok(fds) = proc.fd() {
            for fd in fds {
                if let FDTarget::Socket(inode) = fd.unwrap().target {
                    inodes.insert(inode);
                }
            }
        }

        let tcp = procfs::net::tcp().unwrap_or_default();

        return tcp
            .iter()
            .find(|x| x.state == TcpState::Established && inodes.contains(&x.inode))
            .map(|x| x.local_address.port());
    }
    None
}

#[derive(Debug, Clone)]
#[cfg(target_os = "windows")]
pub struct TcpNetEntry {
    pub local_address: SocketAddr,
    pub remote_address: SocketAddr,
    pub state: MIB_TCP_STATE,
    pub pid: u32,
}

#[cfg(target_os = "windows")]
fn get_tcp_entry_list() -> Result<Vec<TcpNetEntry>, std::io::Error> {
    let mut entry_list = Vec::new();

    let mut buffer_size = 0;
    let ret = unsafe { GetTcpTable2(std::ptr::null_mut(), &mut buffer_size, 0) };
    if ret != ERROR_INSUFFICIENT_BUFFER {
        return Err(std::io::Error::last_os_error());
    }

    let mut buffer = vec![0u8; buffer_size as usize];
    let ret = unsafe {
        GetTcpTable2(
            buffer.as_mut_ptr() as *mut MIB_TCPTABLE2,
            &mut buffer_size,
            0,
        )
    };
    if ret != NO_ERROR {
        return Err(std::io::Error::last_os_error());
    }

    let tcp_table = unsafe { &*(buffer.as_ptr() as *const MIB_TCPTABLE2) };
    for i in 0..tcp_table.dwNumEntries {
        let entry = unsafe { &*tcp_table.table.as_ptr().add(i as usize) };
        entry_list.push(TcpNetEntry {
            local_address: SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::from(unsafe { ntohl(entry.dwLocalAddr) }),
                unsafe { ntohs(entry.dwLocalPort as u16) },
            )),
            remote_address: SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::from(entry.dwRemoteAddr),
                unsafe { ntohs(entry.dwRemotePort as u16) },
            )),
            pid: entry.dwOwningPid,
            state: entry.dwState,
        });
    }

    Ok(entry_list)
}

#[cfg(test)]
mod tests {
    use crate::portpicker;
    use std::net::TcpListener;

    use crate::procs::tcp_port::get_ipv4_port_for_pid;

    #[test]
    fn test_tcp_port() {
        assert!(test_ip_port("0.0.0.0"));
        assert!(test_ip_port("127.0.0.1"));
        assert!(test_ip_port("127.0.0.1"));
    }

    fn test_ip_port(host: &str) -> bool {
        let port = portpicker::pick_unused_port().unwrap();

        let address = format!("{}:{}", host, port);
        let _listener = TcpListener::bind(&address).unwrap();
        let pid = std::process::id();

        if let Some(found_port) = get_ipv4_port_for_pid(pid) {
            found_port == port
        } else {
            false
        }
    }
}
