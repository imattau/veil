import "dart:convert";
import "dart:typed_data";

Uint8List encodeCbor(dynamic value) {
  final builder = BytesBuilder(copy: false);
  _encodeValue(builder, value);
  return builder.toBytes();
}

dynamic decodeCbor(Uint8List bytes) {
  final reader = _CborReader(bytes);
  return reader.readValue();
}

void _encodeValue(BytesBuilder builder, dynamic value) {
  if (value is int) {
    if (value < 0) {
      throw ArgumentError("negative integers not supported");
    }
    _encodeUnsigned(builder, 0, value);
    return;
  }
  if (value is Uint8List) {
    _encodeUnsigned(builder, 2, value.length);
    builder.add(value);
    return;
  }
  if (value is String) {
    final data = utf8.encode(value);
    _encodeUnsigned(builder, 3, data.length);
    builder.add(data);
    return;
  }
  if (value is List) {
    _encodeUnsigned(builder, 4, value.length);
    for (final entry in value) {
      _encodeValue(builder, entry);
    }
    return;
  }
  if (value is Map) {
    _encodeUnsigned(builder, 5, value.length);
    for (final entry in value.entries) {
      _encodeValue(builder, entry.key);
      _encodeValue(builder, entry.value);
    }
    return;
  }
  throw ArgumentError("unsupported CBOR type: ${value.runtimeType}");
}

void _encodeUnsigned(BytesBuilder builder, int majorType, int value) {
  final prefix = majorType << 5;
  if (value < 24) {
    builder.add([prefix | value]);
    return;
  }
  if (value < 256) {
    builder.add([prefix | 24, value]);
    return;
  }
  if (value < 65536) {
    builder.add([prefix | 25, (value >> 8) & 0xff, value & 0xff]);
    return;
  }
  if (value < 4294967296) {
    builder.add([
      prefix | 26,
      (value >> 24) & 0xff,
      (value >> 16) & 0xff,
      (value >> 8) & 0xff,
      value & 0xff,
    ]);
    return;
  }
  throw ArgumentError("integer too large for minimal CBOR encoder");
}

class _CborReader {
  final Uint8List bytes;
  int offset = 0;

  _CborReader(this.bytes);

  int _readByte() {
    if (offset >= bytes.length) {
      throw StateError("unexpected end of CBOR data");
    }
    return bytes[offset++];
  }

  int _readUint(int additional) {
    if (additional < 24) {
      return additional;
    }
    if (additional == 24) {
      return _readByte();
    }
    if (additional == 25) {
      final hi = _readByte();
      final lo = _readByte();
      return (hi << 8) | lo;
    }
    if (additional == 26) {
      final b1 = _readByte();
      final b2 = _readByte();
      final b3 = _readByte();
      final b4 = _readByte();
      return (b1 << 24) | (b2 << 16) | (b3 << 8) | b4;
    }
    throw StateError("unsupported integer encoding");
  }

  dynamic readValue() {
    final initial = _readByte();
    final major = initial >> 5;
    final additional = initial & 0x1f;

    switch (major) {
      case 0:
        return _readUint(additional);
      case 2:
        final len = _readUint(additional);
        final out = bytes.sublist(offset, offset + len);
        offset += len;
        return Uint8List.fromList(out);
      case 3:
        final len = _readUint(additional);
        final out = bytes.sublist(offset, offset + len);
        offset += len;
        return utf8.decode(out);
      case 4:
        final len = _readUint(additional);
        final list = <dynamic>[];
        for (var i = 0; i < len; i += 1) {
          list.add(readValue());
        }
        return list;
      case 5:
        final len = _readUint(additional);
        final map = <dynamic, dynamic>{};
        for (var i = 0; i < len; i += 1) {
          final key = readValue();
          final value = readValue();
          map[key] = value;
        }
        return map;
      default:
        throw StateError("unsupported CBOR major type $major");
    }
  }
}
