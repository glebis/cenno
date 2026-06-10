use crate::protocol::*;
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc, time::{Duration, Instant}};
use tokio::sync::oneshot;

/// In-memory pending-prompt store. Timed-out prompts stay in the map by design
/// (tray-inbox semantics) — the map grows unboundedly until plan 4 adds
/// eviction/persistence and `get_response`.
#[derive(Clone)]
pub struct PromptRegistry {
    inner: Arc<Mutex<HashMap<String, Pending>>>,
    counter: Arc<std::sync::atomic::AtomicU64>,
}

struct Pending {
    /// None after a resolve() consumed the sender (including a late resolve on a timed-out prompt).
    tx: Option<oneshot::Sender<(String, Via)>>,
    pub request: AskRequest,
    /// When this prompt's ask() stops waiting. Lets pending() distinguish
    /// still-answerable prompts from timed-out leftovers (which stay in the
    /// map for plan-4 inbox semantics but must NOT be replayed to the UI).
    deadline: Instant,
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
        let deadline = Instant::now() + Duration::from_secs(req.timeout_s);
        self.inner.lock().insert(id.clone(), Pending { tx: Some(tx), request: req.clone(), deadline });
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

    /// User-initiated dismiss (the panel's ✕): take the pending sender and
    /// DROP it (don't send), so the parked `ask()`'s `rx.await` resolves to
    /// `Err` and returns `TimedOut` — the no-answer wire shape the agent
    /// already handles on timeout, so dismiss needs no protocol change.
    /// Mirrors resolve()'s sender-take semantics: a dismissed prompt is no
    /// longer answerable (pending() won't replay it). Returns false for an
    /// unknown id (or one whose sender was already consumed).
    pub fn dismiss(&self, id: &str) -> bool {
        let mut map = self.inner.lock();
        match map.get_mut(id).and_then(|p| p.tx.take()) {
            // Dropping the taken sender here ends ask()'s rx.await with Err.
            Some(_tx) => true,
            None => false,
        }
    }

    pub fn pending_ids(&self) -> Vec<String> {
        self.inner.lock().keys().cloned().collect()
    }

    /// Prompts whose ask() is still awaiting an answer — i.e. replayable to a
    /// webview that mounted after the `prompt` event was emitted (cold-start
    /// race). Excludes resolved (tx consumed) and timed-out (deadline passed)
    /// entries. Sorted oldest→newest by the monotonic id counter.
    ///
    /// The third tuple element is the seconds REMAINING until this prompt's
    /// deadline (ceiled, so a prompt with 0.3s left reports 1, never 0): a
    /// replayed prompt has partially burned its timeout_s, and the webview's
    /// auto-hide timer must run on what's left, not the original budget.
    pub fn pending(&self) -> Vec<(String, AskRequest, u64)> {
        let now = Instant::now();
        let mut v: Vec<(String, AskRequest, u64)> = self
            .inner
            .lock()
            .iter()
            .filter(|(_, p)| p.tx.is_some() && now < p.deadline)
            .map(|(id, p)| {
                let remaining_s = (p.deadline - now).as_secs_f64().ceil() as u64;
                (id.clone(), p.request.clone(), remaining_s)
            })
            .collect();
        v.sort_by_key(|(id, _, _)| id.strip_prefix("p_").and_then(|n| n.parse::<u64>().ok()));
        v
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

    #[tokio::test]
    async fn pending_lists_awaiting_prompt_and_empties_after_resolve() {
        let reg = PromptRegistry::new();
        let reg2 = reg.clone();
        let task = tokio::spawn(async move { reg2.ask(req(), |_id, _req| {}).await });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let pending = reg.pending();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].1.title, "t");
        let id = pending[0].0.clone();
        assert!(reg.resolve(&id, "hi".into(), Via::Text));
        task.await.unwrap();
        assert!(reg.pending().is_empty());
    }

    #[tokio::test]
    async fn pending_reports_remaining_not_original_timeout() {
        let reg = PromptRegistry::new();
        let reg2 = reg.clone();
        let long: AskRequest = serde_json::from_str(r#"{"title":"t","timeout_s":10}"#).unwrap();
        let task = tokio::spawn(async move { reg2.ask(long, |_id, _req| {}).await });
        // Burn ~1.1s of the 10s budget; remaining must reflect that
        // (ceil(8.9) = 9), never echo the original timeout_s.
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        let pending = reg.pending();
        assert_eq!(pending.len(), 1);
        let remaining = pending[0].2;
        assert!(remaining < 10, "remaining_s should have decreased, got {remaining}");
        assert!(remaining >= 8, "remaining_s implausibly low, got {remaining}");
        let id = pending[0].0.clone();
        assert!(reg.resolve(&id, "done".into(), Via::Text));
        task.await.unwrap();
    }

    #[tokio::test]
    async fn dismiss_completes_ask_as_timed_out() {
        let reg = PromptRegistry::new();
        let reg2 = reg.clone();
        // Long timeout so the only way ask() returns is via dismiss, not a
        // real elapse.
        let long: AskRequest = serde_json::from_str(r#"{"title":"t","timeout_s":30}"#).unwrap();
        let task = tokio::spawn(async move { reg2.ask(long, |_id, _req| {}).await });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let id = reg.pending_ids()[0].clone();
        assert!(reg.dismiss(&id));
        match task.await.unwrap() {
            AskResponse::TimedOut { prompt_id, answered } => {
                assert_eq!(prompt_id, id);
                assert!(!answered);
            }
            _ => panic!("expected TimedOut from a dismissed prompt"),
        }
        // A dismissed prompt is no longer answerable: pending() must not
        // replay it (mirrors resolve()'s sender-take semantics).
        assert!(reg.pending().is_empty());
    }

    #[tokio::test]
    async fn dismiss_unknown_id_is_false() {
        assert!(!PromptRegistry::new().dismiss("nope"));
    }

    #[tokio::test]
    async fn pending_excludes_timed_out_prompt() {
        let reg = PromptRegistry::new();
        let resp = reg.ask(req(), |_id, _req| {}).await; // timeout_s = 1, elapses
        assert!(matches!(resp, AskResponse::TimedOut { .. }));
        // Timed-out entry stays in the map (inbox semantics)...
        assert_eq!(reg.pending_ids().len(), 1);
        // ...but is no longer answerable, so it must not be replayed.
        assert!(reg.pending().is_empty());
    }
}
