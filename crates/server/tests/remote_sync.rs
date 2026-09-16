// WSX-002 contract tests: reconnecting remote sync loop, bounded queue, dispose.
// Maps to obligations WSX-002-T01..T05 in tasks/WSX-002.md.
// Module included by path; shared lib.rs wiring left to integrator.
#[path = "../src/remote_sync.rs"]
mod remote_sync;

use remote_sync::{
    backoff_secs, RemoteSync, SyncError, SyncEvent, SyncState, SyncTransport, MAX_SYNC_QUEUE_BYTES,
    MAX_SYNC_QUEUE_ITEMS,
};
use std::cell::{Cell, RefCell};

struct Fixture {
    fail_open: bool,
    fail_send: bool,
    opened: Cell<u32>,
    sent: RefCell<Vec<Vec<u8>>>,
    closed: Cell<bool>,
}

impl Fixture {
    fn ok() -> Self {
        Self {
            fail_open: false,
            fail_send: false,
            opened: Cell::new(0),
            sent: RefCell::new(Vec::new()),
            closed: Cell::new(false),
        }
    }

    fn failing() -> Self {
        Self {
            fail_open: true,
            ..Self::ok()
        }
    }
}

impl SyncTransport for Fixture {
    fn open(&self) -> Result<(), SyncError> {
        self.opened.set(self.opened.get() + 1);
        if self.fail_open {
            Err(SyncError::Transport)
        } else {
            Ok(())
        }
    }

    fn send(&self, event: &SyncEvent) -> Result<(), SyncError> {
        if self.fail_send {
            return Err(SyncError::Transport);
        }
        self.sent.borrow_mut().push(event.bytes.clone());
        Ok(())
    }

    fn close(&self) {
        self.closed.set(true);
    }

    fn is_closed(&self) -> bool {
        self.closed.get()
    }
}

fn ev(bytes: &[u8]) -> SyncEvent {
    SyncEvent {
        seq: 0,
        bytes: bytes.to_vec(),
    }
}

#[test]
fn wsx002_t01_happy_path_fifo_ack() {
    let t = Fixture::ok();
    let mut s = RemoteSync::new(&t);
    assert_eq!(s.state(), SyncState::Disconnected);
    s.push(ev(b"a")).unwrap();
    s.push(ev(b"bb")).unwrap();
    s.push(ev(b"ccc")).unwrap();
    assert_eq!(s.connect(), Ok(()));
    assert_eq!(s.state(), SyncState::Connected);
    assert_eq!(
        *t.sent.borrow(),
        vec![b"a".to_vec(), b"bb".to_vec(), b"ccc".to_vec()]
    );
    let seqs: Vec<u64> = s.queue().iter().map(|e| e.seq).collect();
    assert_eq!(seqs.len(), 3);
    for seq in seqs {
        assert!(s.ack(seq), "ack must remove seq {seq}");
    }
    assert_eq!(s.queue_len(), 0);
    assert_eq!(s.queued_bytes(), 0);
}

#[test]
fn wsx002_t02_backoff_sequence_and_error_state() {
    assert_eq!(
        (0..5).map(backoff_secs).collect::<Vec<_>>(),
        vec![2, 4, 8, 16, 30]
    );
    assert_eq!(backoff_secs(u32::MAX), 30);
    let t = Fixture::failing();
    let mut s = RemoteSync::new(&t);
    assert_eq!(s.connect(), Err(SyncError::Transport));
    assert_eq!(s.state(), SyncState::Error { retry_in_secs: 2 });
    s.push(ev(b"x")).unwrap();
    let bytes = s.queued_bytes();
    assert_eq!(s.connect(), Err(SyncError::Transport));
    assert_eq!(s.state(), SyncState::Error { retry_in_secs: 4 });
    assert_eq!(s.queued_bytes(), bytes);
    assert_eq!(s.queue_len(), 1);
}

#[test]
fn wsx002_t03_queue_caps_leave_state_unchanged() {
    let t = Fixture::ok();
    let mut s = RemoteSync::new(&t);
    for _ in 0..MAX_SYNC_QUEUE_ITEMS {
        s.push(ev(b"q")).unwrap();
    }
    assert_eq!(s.queue_len(), 256);
    let bytes = s.queued_bytes();
    assert_eq!(s.push(ev(b"q")), Err(SyncError::QueueFull));
    assert_eq!(s.queue_len(), 256);
    assert_eq!(s.queued_bytes(), bytes);
    assert_eq!(s.state(), SyncState::Disconnected);

    let t2 = Fixture::ok();
    let mut s2 = RemoteSync::new(&t2);
    let big = vec![7u8; 262_144];
    for _ in 0..16 {
        s2.push(SyncEvent {
            seq: 0,
            bytes: big.clone(),
        })
        .unwrap();
    }
    assert_eq!(s2.queued_bytes(), MAX_SYNC_QUEUE_BYTES);
    assert_eq!(s2.push(ev(b"tiny")), Err(SyncError::QueueFull));
    assert_eq!(s2.queued_bytes(), MAX_SYNC_QUEUE_BYTES);
    assert_eq!(
        s2.push(SyncEvent {
            seq: 0,
            bytes: vec![0u8; 262_145],
        }),
        Err(SyncError::TooLarge)
    );
    assert_eq!(s2.queued_bytes(), MAX_SYNC_QUEUE_BYTES);
}

#[test]
fn wsx002_t04_dispose_reclaims_and_rejects() {
    let t = Fixture::ok();
    let mut s = RemoteSync::new(&t);
    s.push(ev(b"hello")).unwrap();
    assert!(s.queued_bytes() > 0);
    s.dispose();
    assert_eq!(s.queued_bytes(), 0);
    assert_eq!(s.queue_len(), 0);
    assert!(t.is_closed());
    assert_eq!(s.state(), SyncState::Disposed);
    let calls = t.opened.get();
    assert_eq!(s.push(ev(b"late")), Err(SyncError::Disposed));
    assert_eq!(s.connect(), Err(SyncError::Disposed));
    assert_eq!(t.opened.get(), calls);
    assert!(t.sent.borrow().is_empty());
}

#[test]
fn wsx002_t05_isolation_and_determinism() {
    let dir = tempfile::tempdir().unwrap();
    let run = || -> (SyncState, Vec<Vec<u8>>) {
        let t = Fixture::ok();
        let out: Vec<Vec<u8>>;
        let st: SyncState;
        {
            let mut s = RemoteSync::new(&t);
            s.push(ev(b"one")).unwrap();
            s.push(ev(b"two")).unwrap();
            s.connect().unwrap();
            out = t.sent.borrow().clone();
            st = s.state();
        }
        (st, out)
    };
    assert_eq!(run(), run());
    assert_eq!(backoff_secs(3), backoff_secs(3));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/remote_sync.rs"))
        .unwrap();
    for banned in [
        "std::fs",
        "std::net",
        "std::thread",
        "std::process",
        "tokio",
        "Command",
        "TcpStream",
        "UdpSocket",
    ] {
        assert!(!src.contains(banned), "banned api in module: {banned}");
    }
}
