use crate::protocol::*;
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc, time::{Duration, Instant}};
use tokio::sync::oneshot;

#[derive(Clone)]
pub struct PromptRegistry {
    inner: Arc<Mutex<HashMap<String, Pending>>>,
    counter: Arc<std::sync::atomic::AtomicU64>,
}

struct Pending {
    /// None after a resolve() consumed the sender (including a late resolve on a timed-out prompt).
    tx: Option<oneshot::Sender<(String, Via)>>,
    // Will be read by the tray inbox in a later plan (Task 4 spec).
    #[allow(dead_code)]
    pub request: AskRequest,
}

impl Default for PromptRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            counter: Arc::new(0.into()),
        }
    }

    /// Registers the prompt, calls `notify(id, req)` (used to emit to the UI),
    /// then awaits the answer or times out. On timeout the prompt STAYS pending
    /// (tray inbox semantics; plan 4 reads these via get_response).
    /// Note: timeout_s == 0 times out immediately — acceptable for the skeleton.
    pub async fn ask(&self, req: AskRequest, notify: impl FnOnce(&str, &AskRequest)) -> AskResponse {
        let n = self.counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let id = format!("p_{n}");
        let (tx, rx) = oneshot::channel();
        self.inner.lock().insert(id.clone(), Pending { tx: Some(tx), request: req.clone() });
        notify(&id, &req);
        let started = Instant::now();
        match tokio::time::timeout(Duration::from_secs(req.timeout_s), rx).await {
            Ok(Ok((answer, via))) => {
                self.inner.lock().remove(&id);
                AskResponse::Answered { answer, via, elapsed_s: started.elapsed().as_secs_f64() }
            }
            // Err(Elapsed) = real timeout; Ok(Err(_)) = sender dropped, structurally impossible here
            _ => AskResponse::TimedOut { answered: false, prompt_id: id },
        }
    }

    pub fn resolve(&self, id: &str, answer: String, via: Via) -> bool {
        let mut map = self.inner.lock();
        match map.get_mut(id).and_then(|p| p.tx.take()) {
            Some(tx) => tx.send((answer, via)).is_ok(),
            None => false,
        }
    }

    pub fn pending_ids(&self) -> Vec<String> {
        self.inner.lock().keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> AskRequest { serde_json::from_str(r#"{"title":"t","timeout_s":1}"#).unwrap() }

    #[tokio::test]
    async fn resolve_completes_ask() {
        let reg = PromptRegistry::new();
        let reg2 = reg.clone();
        let task = tokio::spawn(async move { reg2.ask(req(), |_id, _req| {}).await });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let id = reg.pending_ids()[0].clone();
        assert!(reg.resolve(&id, "hello".into(), Via::Text));
        match task.await.unwrap() {
            AskResponse::Answered { answer, .. } => assert_eq!(answer, "hello"),
            _ => panic!("expected Answered"),
        }
    }

    #[tokio::test]
    async fn timeout_returns_timed_out_and_keeps_pending() {
        let reg = PromptRegistry::new();
        let resp = reg.ask(req(), |_id, _req| {}).await; // timeout_s = 1
        match resp {
            AskResponse::TimedOut { prompt_id, .. } => assert!(reg.pending_ids().contains(&prompt_id)),
            _ => panic!("expected TimedOut"),
        }
    }

    #[tokio::test]
    async fn resolve_unknown_id_is_false() {
        assert!(!PromptRegistry::new().resolve("nope", "x".into(), Via::Text));
    }
}
