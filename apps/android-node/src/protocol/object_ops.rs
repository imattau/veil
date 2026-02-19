use veil_core::tags::derive_feed_tag;
use veil_core::{Epoch, Namespace, ObjectRoot};
use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_node::cache::cache_put;
use veil_node::service::PublisherRuntime;

use super::ProtocolRuntime;

pub(super) fn publish_encoded_object_multi_lane(
    runtime: &mut ProtocolRuntime,
    encoded_object: &[u8],
    fast_peers: &[String],
    fallback_peers: &[String],
    step: u64,
) -> Result<(), String> {
    let PublisherRuntime {
        state,
        fast_adapter,
        fallback_adapter,
        config,
        ..
    } = runtime;

    veil_node::publish::publish_encoded_object_multi_lane(
        state,
        fast_adapter,
        fallback_adapter,
        encoded_object,
        fast_peers,
        fallback_peers,
        step,
        config,
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

pub(super) fn inject_encoded_object(
    runtime: &mut ProtocolRuntime,
    encoded_object: &[u8],
) -> Result<ObjectRoot, String> {
    let wire_root = veil_fec::sharder::derive_object_root(encoded_object);
    let object =
        veil_codec::object::decode_object_cbor(encoded_object).map_err(|e| e.to_string())?;

    let erasure_mode = runtime.config.erasure_mode_for_namespace(object.namespace);
    let shards = veil_fec::sharder::object_to_shards_with_mode_and_padding(
        encoded_object,
        object.namespace,
        object.epoch,
        object.tag,
        wire_root,
        erasure_mode,
        runtime.config.bucket_jitter_extra_levels,
    )
    .map_err(|e| e.to_string())?;

    for shard in shards {
        let sid = veil_fec::sharder::shard_id(&shard).map_err(|e| e.to_string())?;
        let bytes = veil_codec::shard::encode_shard_cbor(&shard).map_err(|e| e.to_string())?;
        cache_put(&mut runtime.state, sid, bytes, 0, 1000);
    }

    Ok(wire_root)
}

pub(super) fn build_batched_object(
    runtime: &ProtocolRuntime,
    item: Vec<u8>,
    namespace: Namespace,
    flags: u16,
    now_step: u64,
    epoch: Epoch,
    pubkey: [u8; 32],
) -> Result<(Vec<u8>, ObjectRoot), String> {
    let items = vec![item];
    let mut payload = Vec::new();
    ciborium::ser::into_writer(&items, &mut payload).map_err(|e| e.to_string())?;

    let tag = derive_feed_tag(&pubkey, namespace);
    let encoded_object = veil_node::publish::build_encoded_object(
        &payload,
        namespace,
        epoch,
        tag,
        &runtime.encrypt_key,
        now_step,
        flags | veil_codec::object::OBJECT_FLAG_BATCHED,
        &XChaCha20Poly1305Cipher,
        runtime.signer.as_ref(),
    )
    .map_err(|e| e.to_string())?;

    let wire_root = veil_fec::sharder::derive_object_root(&encoded_object);
    Ok((encoded_object, wire_root))
}
