use super::*;

#[test]
fn persists_queue_to_disk() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("node_state.json");
    let state = NodeState::new_with_store("0.1-test", Some(path.clone()));
    let _ = state.enqueue_publish(PublishRequest {
        namespace: 32,
        payload: "hello".to_string(),
    });

    let restored = NodeState::new_with_store("0.1-test", Some(path));
    let status = restored.status();
    assert_eq!(status.queue.pending, 1);
}

#[test]
fn identity_persists_across_restart() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("node_state.json");
    let state = NodeState::new_with_store("0.1-test", Some(path.clone()));
    let first = state.identity();
    let restored = NodeState::new_with_store("0.1-test", Some(path));
    let second = restored.identity();
    assert_eq!(first.public_key, second.public_key);
    assert_eq!(first.secret_key, second.secret_key);
}

#[test]
fn policy_persists_across_restart() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("node_state.json");
    let state = NodeState::new_with_store("0.1-test", Some(path.clone()));
    state.trust_pubkey([0x11; 32]);

    let restored = NodeState::new_with_store("0.1-test", Some(path));
    let summary = restored.policy_summary();
    assert_eq!(summary.trusted, 1);
}
