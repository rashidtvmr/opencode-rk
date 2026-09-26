#![forbid(unsafe_code)]
//! Counted `SdkStream` send wrapper (bridge glue).

use crate::sdk_stream::SdkStream;
use crate::stream_drain::feed_sdk;

#[derive(Debug, Default)]
pub struct SdkFlow {
    pub sdk: SdkStream,
    pub sent: u64,
}

impl SdkFlow {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn send(&mut self, bodies: &[String]) {
        feed_sdk(&mut self.sdk, bodies);
        self.sent += bodies.len() as u64;
    }
    pub fn sent(&self) -> u64 {
        self.sent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_zero() {
        let f = SdkFlow::new();
        assert_eq!(f.sent(), 0);
    }

    #[test]
    fn send_feeds_text() {
        let mut f = SdkFlow::new();
        f.send(&["a".to_owned(), "b".to_owned()]);
        assert_eq!(f.sdk.drain_text(), "ab");
    }

    #[test]
    fn send_bumps_count() {
        let mut f = SdkFlow::new();
        f.send(&["a".to_owned(), "b".to_owned()]);
        f.send(&["c".to_owned()]);
        assert_eq!(f.sent(), 3);
        assert_eq!(f.sent, 3);
    }

    #[test]
    fn send_empty_no_bump() {
        let mut f = SdkFlow::new();
        f.send(&[]);
        assert_eq!(f.sent(), 0);
    }

    #[test]
    fn closed_still_counts() {
        let mut f = SdkFlow::new();
        f.sdk.close();
        f.send(&["a".to_owned()]);
        assert_eq!(f.sent(), 1);
        assert!(f.sdk.drain_text().is_empty());
    }
}
