import "dart:async";
import "dart:ffi" as ffi;
import "dart:io";
import "dart:typed_data";

import "package:ffi/ffi.dart";

import "lane.dart";

class QuicLane implements VeilLane {
  final String endpoint;
  final String peerId;
  final String bindAddr;
  final String? trustedPeerCertHex;

  final List<LaneMessage> _inbox = [];
  int _outboundQueued = 0;
  int _outboundOk = 0;
  int _outboundErr = 0;
  int _inboundReceived = 0;
  int _inboundDropped = 0;
  int _reconnectAttempts = 0;
  int _lastSendResult = 0;
  bool _closed = false;

  late final int _handle;

  QuicLane({
    required this.endpoint,
    required this.peerId,
    this.bindAddr = "0.0.0.0:0",
    this.trustedPeerCertHex,
  }) {
    final bindings = _getBindings();
    if (bindings == null) {
      _handle = 0;
      _outboundErr += 1;
    } else {
      _handle = bindings.start(
        bindAddr,
        _serverNameFromEndpoint(endpoint),
        trustedPeerCertHex ?? "",
      );
      if (_handle == 0) {
        _outboundErr += 1;
      }
    }
  }

  static bool isSupportedSync() {
    final bindings = _getBindings();
    return bindings?.isSupported() ?? false;
  }

  static Future<bool> isSupported() async {
    return isSupportedSync();
  }

  static Future<String?> fetchPinnedCertHex(String endpoint) async {
    final bindings = _getBindings();
    if (bindings == null) {
      return null;
    }
    final serverName = _serverNameFromEndpoint(endpoint);
    final cert = bindings.fetchPeerCert(endpoint, serverName);
    if (cert == null || cert.isEmpty) {
      return null;
    }
    return cert;
  }

  @override
  Future<void> send(String peer, List<int> bytes) async {
    if (_closed) {
      throw StateError("lane is closed");
    }
    final bindings = _getBindings();
    if (bindings == null || _handle == 0) {
      _outboundErr += 1;
      _outboundQueued += 1;
      return;
    }
    final result = bindings.send(_handle, peer, bytes);
    _lastSendResult = result;
    if (result == 0) {
      _outboundOk += 1;
    } else {
      _outboundErr += 1;
      _outboundQueued += 1;
    }
  }

  @override
  Future<LaneMessage?> recv() async {
    if (_inbox.isNotEmpty) {
      return _inbox.removeAt(0);
    }
    final bindings = _getBindings();
    if (bindings == null || _handle == 0) {
      return null;
    }
    final msg = bindings.recv(_handle);
    if (msg == null) {
      return null;
    }
    _inboundReceived += 1;
    return msg;
  }

  @override
  LaneHealthSnapshot healthSnapshot() {
    return LaneHealthSnapshot(
      outboundQueued: _outboundQueued,
      outboundSendOk: _outboundOk,
      outboundSendErr: _outboundErr,
      inboundReceived: _inboundReceived,
      inboundDropped: _inboundDropped,
      reconnectAttempts: _reconnectAttempts,
    );
  }

  Future<void> close() async {
    _closed = true;
    final bindings = _getBindings();
    if (bindings == null || _handle == 0) {
      return;
    }
    bindings.stop(_handle);
  }

  int get debugHandle => _handle;

  int get debugLastSendResult => _lastSendResult;

  QuicLaneMetrics? metricsSnapshot() {
    final bindings = _getBindings();
    if (bindings == null || _handle == 0) {
      return null;
    }
    final metrics = bindings.metrics(_handle);
    if (metrics == null) {
      return null;
    }
    return QuicLaneMetrics(
      outboundQueued: metrics.outboundQueued,
      sendAttempts: metrics.sendAttempts,
      sendSuccess: metrics.sendSuccess,
      sendErrors: metrics.sendErrors,
      inboundReceived: metrics.inboundReceived,
      inboundDropped: metrics.inboundDropped,
    );
  }
}

class QuicLaneMetrics {
  final int outboundQueued;
  final int sendAttempts;
  final int sendSuccess;
  final int sendErrors;
  final int inboundReceived;
  final int inboundDropped;

  const QuicLaneMetrics({
    required this.outboundQueued,
    required this.sendAttempts,
    required this.sendSuccess,
    required this.sendErrors,
    required this.inboundReceived,
    required this.inboundDropped,
  });
}

String _serverNameFromEndpoint(String endpoint) {
  final uri = Uri.tryParse(endpoint);
  if (uri != null && uri.host.isNotEmpty) {
    return uri.host;
  }
  final parts = endpoint.split(":");
  return parts.isNotEmpty ? parts.first : "localhost";
}

class _QuicBindings {
  _QuicBindings(this._lib)
    : _isSupported = _lib.lookupFunction<_isSupportedNative, _isSupportedDart>(
        "veil_quic_is_supported",
      ),
      _start = _lib.lookupFunction<_startNative, _startDart>("veil_quic_start"),
      _send = _lib.lookupFunction<_sendNative, _sendDart>("veil_quic_send"),
      _recv = _lib.lookupFunction<_recvNative, _recvDart>("veil_quic_recv"),
      _fetchPeerCert = _lib
          .lookupFunction<_fetchPeerCertNative, _fetchPeerCertDart>(
            "veil_quic_fetch_peer_cert",
          ),
      _freeString = _lib.lookupFunction<_freeStringNative, _freeStringDart>(
        "veil_quic_free_string",
      ),
      _freeRecv = _lib.lookupFunction<_freeRecvNative, _freeRecvDart>(
        "veil_quic_free_recv",
      ),
      _metrics = _lib.lookupFunction<_metricsNative, _metricsDart>(
        "veil_quic_metrics",
      ),
      _stop = _lib.lookupFunction<_stopNative, _stopDart>("veil_quic_stop");

  final ffi.DynamicLibrary _lib;
  final _isSupportedDart _isSupported;
  final _startDart _start;
  final _sendDart _send;
  final _recvDart _recv;
  final _fetchPeerCertDart _fetchPeerCert;
  final _freeStringDart _freeString;
  final _freeRecvDart _freeRecv;
  final _metricsDart _metrics;
  final _stopDart _stop;

  bool isSupported() => _isSupported() == 1;

  int start(String bindAddr, String serverName, String trustedCertHex) {
    final bindPtr = bindAddr.toNativeUtf8();
    final namePtr = serverName.toNativeUtf8();
    final certPtr = trustedCertHex.toNativeUtf8();
    final handle = _start(bindPtr, namePtr, certPtr);
    malloc.free(bindPtr);
    malloc.free(namePtr);
    malloc.free(certPtr);
    return handle;
  }

  int send(int handle, String peer, List<int> bytes) {
    final peerPtr = peer.toNativeUtf8();
    final dataPtr = malloc<ffi.Uint8>(bytes.length);
    dataPtr.asTypedList(bytes.length).setAll(0, bytes);
    final result = _send(handle, peerPtr, dataPtr, bytes.length);
    malloc.free(peerPtr);
    malloc.free(dataPtr);
    return result;
  }

  LaneMessage? recv(int handle) {
    final ptr = _recv(handle);
    if (ptr == ffi.nullptr) {
      return null;
    }
    final recv = ptr.ref;
    final peer = recv.peer.cast<Utf8>().toDartString();
    final bytes = Uint8List.fromList(recv.data.asTypedList(recv.len));
    _freeRecv(ptr);
    return LaneMessage(peer: peer, bytes: bytes);
  }

  _QuicMetrics? metrics(int handle) {
    final out = malloc<_QuicMetrics>();
    final rc = _metrics(handle, out);
    if (rc != 0) {
      malloc.free(out);
      return null;
    }
    final value = out.ref;
    malloc.free(out);
    return value;
  }

  String? fetchPeerCert(String endpoint, String serverName) {
    final endpointPtr = endpoint.toNativeUtf8();
    final namePtr = serverName.toNativeUtf8();
    final ptr = _fetchPeerCert(endpointPtr, namePtr);
    malloc.free(endpointPtr);
    malloc.free(namePtr);
    if (ptr == ffi.nullptr) {
      return null;
    }
    final value = ptr.cast<Utf8>().toDartString();
    _freeString(ptr);
    return value;
  }

  void stop(int handle) {
    _stop(handle);
  }
}

_QuicBindings? _bindings;

_QuicBindings? _getBindings() {
  if (_bindings != null) {
    return _bindings;
  }
  try {
    _bindings = _QuicBindings(_openLib());
    print("QuicLane: Native library loaded successfully.");
  } catch (e) {
    print("QuicLane: Failed to load native library: $e");
    _bindings = null;
  }
  return _bindings;
}

ffi.DynamicLibrary _openLib() {
  final overridePath = Platform.environment["VEIL_SDK_BRIDGE_LIB"];
  if (overridePath != null && overridePath.isNotEmpty) {
    print("QuicLane: Using VEIL_SDK_BRIDGE_LIB=$overridePath");
    return ffi.DynamicLibrary.open(overridePath);
  }

  String libName;
  if (Platform.isAndroid) {
    libName = "libveil_sdk_bridge.so";
  } else if (Platform.isIOS) {
    // On iOS, static libraries are directly linked.
    // DynamicLibrary.process() is used to access symbols from the main executable.
    return ffi.DynamicLibrary.process();
  } else if (Platform.isMacOS) {
    libName = "libveil_sdk_bridge.dylib";
  } else if (Platform.isWindows) {
    libName = "veil_sdk_bridge.dll";
  } else {
    // Default for Linux/other
    libName = "libveil_sdk_bridge.so";
  }
  print("QuicLane: Attempting to open native library: $libName");
  try {
    final lib = ffi.DynamicLibrary.open(libName);
    print("QuicLane: Native library $libName opened successfully.");
    return lib;
  } catch (e) {
    print("QuicLane: Failed to open native library $libName: $e");
    rethrow;
  }
}

typedef _isSupportedNative = ffi.Int32 Function();
typedef _isSupportedDart = int Function();
typedef _startNative =
    ffi.Uint64 Function(
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
    );
typedef _startDart =
    int Function(ffi.Pointer<Utf8>, ffi.Pointer<Utf8>, ffi.Pointer<Utf8>);
typedef _sendNative =
    ffi.Int32 Function(
      ffi.Uint64,
      ffi.Pointer<Utf8>,
      ffi.Pointer<ffi.Uint8>,
      ffi.IntPtr,
    );
typedef _sendDart =
    int Function(int, ffi.Pointer<Utf8>, ffi.Pointer<ffi.Uint8>, int);
typedef _recvNative = ffi.Pointer<_QuicRecv> Function(ffi.Uint64);
typedef _recvDart = ffi.Pointer<_QuicRecv> Function(int);
typedef _fetchPeerCertNative =
    ffi.Pointer<Utf8> Function(ffi.Pointer<Utf8>, ffi.Pointer<Utf8>);
typedef _fetchPeerCertDart =
    ffi.Pointer<Utf8> Function(ffi.Pointer<Utf8>, ffi.Pointer<Utf8>);
typedef _freeStringNative = ffi.Void Function(ffi.Pointer<Utf8>);
typedef _freeStringDart = void Function(ffi.Pointer<Utf8>);
typedef _freeRecvNative = ffi.Void Function(ffi.Pointer<_QuicRecv>);
typedef _freeRecvDart = void Function(ffi.Pointer<_QuicRecv>);
typedef _metricsNative = ffi.Int32 Function(
  ffi.Uint64,
  ffi.Pointer<_QuicMetrics>,
);
typedef _metricsDart = int Function(int, ffi.Pointer<_QuicMetrics>);
typedef _stopNative = ffi.Void Function(ffi.Uint64);
typedef _stopDart = void Function(int);

final class _QuicRecv extends ffi.Struct {
  external ffi.Pointer<Utf8> peer;

  external ffi.Pointer<ffi.Uint8> data;

  @ffi.IntPtr()
  external int len;
}

final class _QuicMetrics extends ffi.Struct {
  @ffi.Uint64()
  external int outboundQueued;
  @ffi.Uint64()
  external int sendAttempts;
  @ffi.Uint64()
  external int sendSuccess;
  @ffi.Uint64()
  external int sendErrors;
  @ffi.Uint64()
  external int inboundReceived;
  @ffi.Uint64()
  external int inboundDropped;
}
