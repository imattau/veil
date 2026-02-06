import "dart:typed_data";

Uint8List hexToBytes(String hex) {
  var normalized = hex.toLowerCase();
  if (normalized.startsWith("0x")) {
    normalized = normalized.substring(2);
  }
  if (normalized.length % 2 != 0) {
    throw ArgumentError("hex length must be even");
  }
  final out = Uint8List(normalized.length ~/ 2);
  for (var i = 0; i < out.length; i += 1) {
    final byte = normalized.substring(i * 2, i * 2 + 2);
    out[i] = int.parse(byte, radix: 16);
  }
  return out;
}

String bytesToHex(Uint8List bytes) {
  final buffer = StringBuffer();
  for (final b in bytes) {
    buffer.write(b.toRadixString(16).padLeft(2, "0"));
  }
  return buffer.toString();
}
