use super::*;

use uuid::Uuid;
use veil_core::ObjectRoot;

pub(in crate::server) async fn publish(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PublishRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if request.payload.len() > MAX_RAW_PAYLOAD_BYTES {
        return bad_request("payload_too_large", "payload exceeds max size");
    }
    let message_id = state.node.enqueue_publish(request);
    let response = PublishResponse {
        message_id,
        queued: true,
    };
    Json(response).into_response()
}

pub(in crate::server) async fn publish_object(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ObjectPublishRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let payload = match base64::engine::general_purpose::STANDARD.decode(&request.payload_b64) {
        Ok(bytes) => bytes,
        Err(_) => return bad_request("invalid_base64", "payload must be base64 encoded"),
    };
    if payload.len() > MAX_UPLOAD_PAYLOAD_BYTES {
        return bad_request("payload_too_large", "payload exceeds max size");
    }
    let content_root = veil_fec::sharder::derive_object_root(&payload);

    // Build the object immediately to get the wire_root
    let (encoded_object, wire_root): (Vec<u8>, ObjectRoot) = match state
        .protocol
        .build_object(payload, request.namespace, 0)
        .await
    {
        Ok(res) => res,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };

    // Inject into local cache so it's immediately fetchable
    let _ = state.protocol.inject_object(encoded_object.clone()).await;

    let wrapped_payload = serde_json::json!({
        "kind": "raw_object_b64",
        "payload_b64": base64::engine::general_purpose::STANDARD.encode(encoded_object),
    })
    .to_string();

    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace: request.namespace,
        payload: wrapped_payload,
    });

    Json(ObjectPublishResponse {
        object_root: hex::encode(content_root),
        wire_root: hex::encode(wire_root),
        message_id,
        queued: true,
    })
    .into_response()
}

pub(in crate::server) async fn queue_raw_object_payload(
    state: &AppState,
    namespace: u16,
    payload: &[u8],
) -> Result<(Uuid, [u8; 32]), String> {
    // Build the object immediately to get the wire_root
    let (encoded_object, wire_root): (Vec<u8>, ObjectRoot) = state
        .protocol
        .build_object(payload.to_vec(), namespace, 0)
        .await?;

    // Inject into local cache so it's immediately fetchable
    state.protocol.inject_object(encoded_object.clone()).await?;

    let wrapped_payload = serde_json::json!({
        "kind": "raw_object_b64",
        "payload_b64": base64::engine::general_purpose::STANDARD.encode(encoded_object),
    })
    .to_string();

    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace,
        payload: wrapped_payload,
    });
    Ok((message_id, wire_root))
}
