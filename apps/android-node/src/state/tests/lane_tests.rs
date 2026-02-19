use super::*;

#[test]
fn lane_details_update_summary() {
    let state = NodeState::new("0.1-test");
    state.mark_lane_details(vec![
        LaneDetail {
            role: "fast".to_string(),
            lane: "quic".to_string(),
            connected: true,
            last_error: None,
            last_error_code: None,
            stats: crate::api::LaneStats::default(),
        },
        LaneDetail {
            role: "fallback".to_string(),
            lane: "websocket".to_string(),
            connected: false,
            last_error: Some("send_error".to_string()),
            last_error_code: Some("500".to_string()),
            stats: crate::api::LaneStats::default(),
        },
    ]);

    let status = state.status();
    assert_eq!(status.lanes.details.len(), 2);
    assert!(status.lanes.quic.connected);
    assert!(!status.lanes.websocket.connected);
    assert_eq!(
        status.lanes.websocket.last_error.as_deref(),
        Some("send_error")
    );
}
