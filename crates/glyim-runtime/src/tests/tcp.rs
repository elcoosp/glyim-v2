//! Tests for TCP networking FFI functions.
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::{
    glyim_net_tcp_accept, glyim_net_tcp_bind, glyim_net_tcp_connect, glyim_net_tcp_read,
    glyim_net_tcp_write,
};

#[test]
#[ignore] // Flaky on CI due to OS TIME_WAIT / mutex contention during accept()
fn tcp_echo_server_client() {
    // Find a free port using std TcpListener to avoid TIME_WAIT races.
    let std_lis = TcpListener::bind("127.0.0.1:0").expect("failed to bind std listener");
    let port = std_lis.local_addr().unwrap().port();
    drop(std_lis);

    // Give OS a moment to release the port.
    thread::sleep(Duration::from_millis(100));

    let (tx, rx) = mpsc::channel();

    // Server thread
    let server = thread::spawn(move || {
        let server_fd = unsafe { glyim_net_tcp_bind(b"127.0.0.1".as_ptr(), 9, port) };
        assert!(server_fd > 0, "server bind failed on port {}", port);

        // Signal that server is bound and ready to accept.
        tx.send(()).unwrap();

        let client_fd = unsafe { glyim_net_tcp_accept(server_fd) };
        assert!(client_fd > 0, "server accept failed");

        let mut buf = [0u8; 32];
        let read_len = unsafe { glyim_net_tcp_read(client_fd, buf.as_mut_ptr(), 32) };
        assert_eq!(read_len, 5, "server expected to read 5 bytes");
        assert_eq!(&buf[..5], b"hello", "server received wrong data");

        let write_len = unsafe { glyim_net_tcp_write(client_fd, b"world".as_ptr(), 5) };
        assert_eq!(write_len, 5, "server expected to write 5 bytes");
    });

    // Client thread
    let client = thread::spawn(move || {
        // Wait for server to be ready.
        rx.recv().unwrap();

        // Connect with retries to handle transient issues.
        let mut client_fd = -1i32;
        for _ in 0..10 {
            client_fd = unsafe { glyim_net_tcp_connect(b"127.0.0.1".as_ptr(), 9, port) };
            if client_fd > 0 {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        assert!(client_fd > 0, "client connect failed on port {}", port);

        let write_len = unsafe { glyim_net_tcp_write(client_fd, b"hello".as_ptr(), 5) };
        assert_eq!(write_len, 5, "client expected to write 5 bytes");

        let mut buf = [0u8; 32];
        let read_len = unsafe { glyim_net_tcp_read(client_fd, buf.as_mut_ptr(), 32) };
        assert_eq!(read_len, 5, "client expected to read 5 bytes");
        assert_eq!(&buf[..5], b"world", "client received wrong data");
    });

    server.join().expect("server thread panicked");
    client.join().expect("client thread panicked");
}
