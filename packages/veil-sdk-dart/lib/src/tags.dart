import "bridge/veil_bridge.dart";

Future<List<String>> deriveRvTagWindowHex(
  String recipientPubkeyHex,
  int nowSeconds,
  int namespace, {
  int epochSeconds = 86400,
  int overlapSeconds = 3600,
  VeilBridge? bridge,
}) async {
  final client = bridge ?? const VeilBridge();
  final epoch = await client.currentEpoch(nowSeconds, epochSeconds);
  final offset = nowSeconds % epochSeconds;
  final epochs = <int>{epoch};
  if (overlapSeconds > 0) {
    if (offset >= epochSeconds - overlapSeconds) {
      epochs.add(epoch + 1);
    }
    if (offset < overlapSeconds && epoch > 0) {
      epochs.add(epoch - 1);
    }
  }
  final ordered = epochs.toList()..sort();
  final tags = <String>[];
  for (final value in ordered) {
    tags.add(await client.deriveRvTagHex(recipientPubkeyHex, value, namespace));
  }
  return tags;
}
