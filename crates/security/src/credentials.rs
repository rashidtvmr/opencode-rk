//! Integration credential lifecycle management (API keys/tokens with expiry).
use std::{collections::HashMap, time::SystemTime};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Credential {
    pub id: String,
    pub provider: String,
    pub token: String,
    pub expires_at: Option<SystemTime>,
}
impl Credential {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        provider: impl Into<String>,
        token: impl Into<String>,
        expires_at: Option<SystemTime>,
    ) -> Self {
        Self {
            id: id.into(),
            provider: provider.into(),
            token: token.into(),
            expires_at: expires_at,
        }
    }
    #[must_use]
    pub fn is_expired_at(&self, now: SystemTime) -> bool {
        self.expires_at.is_some_and(|t| t <= now)
    }
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.is_expired_at(SystemTime::now())
    }
}
#[derive(Clone, Debug, Default)]
pub struct CredentialStore {
    credentials: HashMap<String, Credential>,
}
impl CredentialStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            credentials: HashMap::new(),
        }
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.credentials.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.credentials.is_empty()
    }
    pub fn add(&mut self, cred: Credential) {
        self.credentials.insert(cred.id.clone(), cred);
    }
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Credential> {
        self.credentials.get(id)
    }
    pub fn remove(&mut self, id: &str) -> bool {
        self.credentials.remove(id).is_some()
    }
    #[must_use]
    pub fn is_expired(&self, id: &str) -> bool {
        self.credentials.get(id).is_none_or(|c| c.is_expired())
    }
    pub fn prune_expired(&mut self) -> usize {
        let now = SystemTime::now();
        let dead: Vec<String> = self
            .credentials
            .iter()
            .filter(|(_, c)| c.is_expired_at(now))
            .map(|(k, _)| k.clone())
            .collect();
        let n = dead.len();
        for k in dead {
            self.credentials.remove(&k);
        }
        n
    }
    #[must_use]
    pub fn list_providers(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .credentials
            .values()
            .map(|c| c.provider.clone())
            .collect();
        out.sort();
        out.dedup();
        out
    }
}
// ponytail: skipped zeroize on remove/drop, add when zeroize dep accepted.
#[cfg(test)]
mod tests {
    use super::*;
    fn cred(id: &str, provider: &str, exp: Option<SystemTime>) -> Credential {
        Credential::new(id, provider, "tok", exp)
    }
    fn past() -> SystemTime {
        SystemTime::now() - std::time::Duration::from_secs(60)
    }
    fn future() -> SystemTime {
        SystemTime::now() + std::time::Duration::from_secs(3600)
    }
    #[test]
    fn add_and_get() {
        let mut s = CredentialStore::new();
        s.add(cred("a", "github", Some(future())));
        let c = s.get("a").expect("present");
        assert_eq!(c.id, "a");
        assert_eq!(c.provider, "github");
        assert_eq!(c.token, "tok");
        assert!(s.get("missing").is_none());
    }
    #[test]
    fn expired_detected() {
        let mut s = CredentialStore::new();
        s.add(cred("old", "github", Some(past())));
        s.add(cred("fresh", "github", Some(future())));
        s.add(cred("eternal", "github", None));
        assert!(s.is_expired("old"));
        assert!(!s.is_expired("fresh"));
        assert!(!s.is_expired("eternal"));
        assert!(s.is_expired("missing"));
    }
    #[test]
    fn prune_removes_expired() {
        let mut s = CredentialStore::new();
        s.add(cred("old1", "a", Some(past())));
        s.add(cred("old2", "b", Some(past())));
        s.add(cred("fresh", "a", Some(future())));
        s.add(cred("eternal", "c", None));
        assert_eq!(s.prune_expired(), 2);
        assert_eq!(s.len(), 2);
        assert!(s.get("old1").is_none());
        assert!(s.get("old2").is_none());
        assert!(s.get("fresh").is_some());
        assert!(s.get("eternal").is_some());
    }
    #[test]
    fn remove_works() {
        let mut s = CredentialStore::new();
        s.add(cred("a", "github", Some(future())));
        assert!(s.remove("a"));
        assert!(s.get("a").is_none());
        assert!(!s.remove("a"));
    }
    #[test]
    fn list_providers() {
        let mut s = CredentialStore::new();
        assert!(s.list_providers().is_empty());
        s.add(cred("a", "github", Some(future())));
        s.add(cred("b", "openai", None));
        s.add(cred("c", "github", Some(future())));
        assert_eq!(
            s.list_providers(),
            vec!["github".to_owned(), "openai".to_owned()]
        );
    }
}
