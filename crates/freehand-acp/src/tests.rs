//! Tests for the Freehand ACP v1 agent adapter.
//!
//! Wire and handler integration are covered by the daemon `acp` end-to-end
//! smoke test (`scripts/verify-acp-stdio.sh`) against the real binary on real
//! stdio. These unit tests cover the pure helpers plus the cancel-token
//! reset lifecycle through the production `run_prompt_with_reset` function
//! with a fake turn runner, so the test directly guards the reset code in
//! `crates/freehand-acp/src/lib.rs`.

use super::{AcpSession, TurnError, extract_text, monotonic_id, run_prompt_with_reset};
use agent_client_protocol::schema::v1::{ContentBlock, StopReason, TextContent};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[test]
fn extract_text_joins_text_blocks_with_newlines() {
    let blocks = vec![
        ContentBlock::Text(TextContent::new("hello")),
        ContentBlock::Text(TextContent::new("world")),
    ];
    assert_eq!(extract_text(&blocks), "hello\nworld");
}

#[test]
fn extract_text_returns_empty_for_no_text_blocks() {
    let blocks: Vec<ContentBlock> = Vec::new();
    assert_eq!(extract_text(&blocks), "");
}

#[test]
fn monotonic_id_is_strictly_increasing_within_a_process() {
    let a = monotonic_id();
    let b = monotonic_id();
    let c = monotonic_id();
    assert!(a < b);
    assert!(b < c);
}

#[test]
fn cancel_token_flip_changes_subsequent_observation() {
    let token = AtomicBool::new(false);
    assert!(!token.load(Ordering::SeqCst));
    token.store(true, Ordering::SeqCst);
    assert!(token.load(Ordering::SeqCst));
}

/// Red test for the cancel-token reset lifecycle, exercised through the
/// production `run_prompt_with_reset` function. If the reset after the
/// turn returns is removed, this test fails because the second prompt
/// keeps observing the stale cancel flag and the fake runner returns
/// `Cancelled` again.
#[tokio::test(flavor = "current_thread")]
async fn cancel_does_not_brick_following_prompts() {
    let session = Arc::new(AcpSession {
        session_id: agent_client_protocol::schema::v1::SessionId::new("acp-test"),
        cwd: PathBuf::from("/tmp"),
        cancel: Arc::new(AtomicBool::new(false)),
        closed: Arc::new(AtomicBool::new(false)),
    });

    // First cancel the session, then prompt: the runner observes the
    // cancel flag and returns Cancelled.
    session.cancel.store(true, Ordering::SeqCst);
    let first = run_prompt_with_reset(&session, "first", fake_turn_runner, ()).await;
    assert_eq!(first, StopReason::Cancelled);

    // Second prompt without a fresh cancel: the runner must NOT see the
    // stale flag because the production code resets it after the first
    // turn returns.
    let second = run_prompt_with_reset(&session, "second", fake_turn_runner, ()).await;
    assert_eq!(second, StopReason::EndTurn);
}

/// Fake turn runner that mirrors the runtime contract: if the session
/// cancel flag is set, return `Cancelled`; otherwise return `Ok(())`.
fn fake_turn_runner(session: &Arc<AcpSession>, _prompt: &str, _cx: &()) -> Result<(), TurnError> {
    if session.cancel.load(Ordering::SeqCst) {
        Err(TurnError::Cancelled)
    } else {
        Ok(())
    }
}
