use crate::api::LaneDetail;

use super::StateInner;

pub(super) fn apply_lane_health_update(
    inner: &mut StateInner,
    lane: &str,
    connected: bool,
    last_error: Option<String>,
) {
    let target = lane_target_mut(inner, lane);
    target.connected = connected;
    target.last_error = last_error;
}

pub(super) fn apply_lane_details(inner: &mut StateInner, details: Vec<LaneDetail>) {
    inner.lane_details = details.clone();
    inner.quic = Default::default();
    inner.websocket = Default::default();
    inner.tor = Default::default();

    for detail in &details {
        let target = detail_lane_target_mut(inner, &detail.lane);
        target.connected |= detail.connected;
        if target.last_error.is_none() {
            target.last_error = detail.last_error.clone();
        }
    }
}

fn lane_target_mut<'a>(inner: &'a mut StateInner, lane: &str) -> &'a mut crate::api::LaneHealth {
    match lane {
        "quic" => &mut inner.quic,
        "tor" => &mut inner.tor,
        _ => &mut inner.websocket,
    }
}

fn detail_lane_target_mut<'a>(
    inner: &'a mut StateInner,
    lane: &str,
) -> &'a mut crate::api::LaneHealth {
    if lane.contains("quic") {
        &mut inner.quic
    } else if lane.contains("tor") {
        &mut inner.tor
    } else {
        &mut inner.websocket
    }
}
