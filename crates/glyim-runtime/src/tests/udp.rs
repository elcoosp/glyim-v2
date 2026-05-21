//! UDP networking tests for glyim-runtime

use crate::*;
use std::net::UdpSocket;

#[test]
fn udp_send_recv() {
    let server = UdpSocket::bind("127.0.0.1:0").unwrap();
    let port = server.local_addr().unwrap().port();
    let fd = unsafe { glyim_net_udp_bind(b"127.0.0.1\0".as_ptr(), 9, 0) };
    assert!(fd >= 0, "udp bind failed");

    let msg = b"ping";
    let dest_addr = b"127.0.0.1\0";
    let sent = unsafe {
        glyim_net_udp_send_to(fd, msg.as_ptr(), msg.len(), dest_addr.as_ptr(), dest_addr.len() - 1, port)
    };
    assert_eq!(sent, msg.len() as isize);

    let mut buf = [0u8; 4];
    let mut src_addr_buf = [0u8; 64];
    let mut src_addr_len = src_addr_buf.len();
    let mut src_port = 0;
    let recv = unsafe {
        glyim_net_udp_recv_from(fd, buf.as_mut_ptr(), buf.len(), src_addr_buf.as_mut_ptr(), &mut src_addr_len, &mut src_port)
    };
    assert_eq!(recv, msg.len() as isize);
    assert_eq!(&buf[..], msg);
}
