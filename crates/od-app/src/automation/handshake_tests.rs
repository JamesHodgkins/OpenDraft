//! The wait a new connection is given to say hello: a silent one is dropped,
//! so idle local connections cannot fill the few slots, and a slow one that
//! still speaks in time is served.

use std::time::Duration;

use interprocess::local_socket::Stream;
use od_api::frame::{read_frame, write_frame};
use od_api::handshake::{Hello, HelloReply};

use super::host_tests::{connect, hello, start_impatient, Fixture};

/// Whether a client that says hello now is served: `Some(reply)` or `None`
/// when the connection is closed on it.
fn try_hello(fixture: &Fixture) -> Option<HelloReply> {
    let mut stream = connect(&fixture.descriptor);
    let hello = Hello {
        token: fixture.descriptor.token.clone(),
        api_versions: vec![od_api::API_VERSION],
        client_name: "test".to_owned(),
    };
    write_frame(&mut stream, &serde_json::to_vec(&hello).unwrap()).ok()?;
    let frame = read_frame(&mut stream).ok()?;
    serde_json::from_slice(&frame).ok()
}

#[test]
fn a_connection_that_never_says_hello_is_dropped_and_frees_its_slot() {
    let fixture = start_impatient("silent", Duration::from_millis(400));
    // Fill every slot with a client that connects and says nothing.
    let silent: Vec<Stream> = (0..8).map(|_| connect(&fixture.descriptor)).collect();
    std::thread::sleep(Duration::from_millis(50));
    assert!(
        try_hello(&fixture).is_none(),
        "every slot is taken, so a real client is turned away"
    );
    // Once the silent ones have waited long enough they are dropped.
    let mut waited = Duration::ZERO;
    let reply = loop {
        if let Some(reply) = try_hello(&fixture) {
            break reply;
        }
        assert!(waited < Duration::from_secs(5), "never served");
        std::thread::sleep(Duration::from_millis(50));
        waited += Duration::from_millis(50);
    };
    assert!(reply.ok);
    drop(silent);
}

#[test]
fn a_slow_hello_inside_the_wait_is_still_served() {
    let fixture = start_impatient("slow", Duration::from_secs(5));
    let mut stream = connect(&fixture.descriptor);
    std::thread::sleep(Duration::from_millis(300));
    let reply = hello(
        &mut stream,
        &fixture.descriptor.token,
        &[od_api::API_VERSION],
    );
    assert!(reply.ok);
}
