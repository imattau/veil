typedef TagHex = String;

typedef Epoch = int;

typedef Namespace = int;

class ShardMeta {
  final int version;
  final Namespace namespace;
  final Epoch epoch;
  final TagHex tagHex;
  final String objectRootHex;
  final int k;
  final int n;
  final int index;
  final int payloadLen;

  const ShardMeta({
    required this.version,
    required this.namespace,
    required this.epoch,
    required this.tagHex,
    required this.objectRootHex,
    required this.k,
    required this.n,
    required this.index,
    required this.payloadLen,
  });
}

class ObjectMeta {
  final int version;
  final Namespace namespace;
  final Epoch epoch;
  final int flags;
  final bool signed;
  final bool public;
  final bool ackRequested;
  final bool batched;
  final TagHex tagHex;
  final String objectRootHex;
  final String? senderPubkeyHex;
  final String nonceHex;
  final int ciphertextLen;
  final int paddingLen;

  const ObjectMeta({
    required this.version,
    required this.namespace,
    required this.epoch,
    required this.flags,
    required this.signed,
    required this.public,
    required this.ackRequested,
    required this.batched,
    required this.tagHex,
    required this.objectRootHex,
    required this.senderPubkeyHex,
    required this.nonceHex,
    required this.ciphertextLen,
    required this.paddingLen,
  });
}
