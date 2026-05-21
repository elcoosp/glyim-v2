//! TCP networking tests for glyim-runtime

use crate::*;
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

// Helper: find an available port
fn find_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

#[test]
fn tcp_echo_server_client() {
    let port = find_free_port();
    let server_thread = thread::spawn(move || {
        let listener = TcpListener::bind(("127.0.0.1", port)).unwrap();
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).unwrap();
        stream.write_all(&buf[..n]).unwrap();
    });

    // Give server time to start
    thread::sleep(Duration::from_millis(100));

    let addr = "127.0.0.1\0";
    let fd = unsafe { glyim_net_tcp_connect(addr.as_ptr(), addr.len() - 1, port) };
    assert!(fd >= 0, "tcp connect failed");

    let msg = b"hello";
    let written = unsafe { glyim_net_tcp_write(fd, msg.as_ptr(), msg.len()) };
    assert_eq!(written, msg.len() as isize);

    let mut buf = [0u8; 5];
    let read = unsafe { glyim_net_tcp_read(fd, buf.as_mut_ptr(), buf.len()) };
    assert_eq!(read, msg.len() as isize);
    assert_eq!(&buf[..], msg);

    server_thread.join().unwrap();
}
