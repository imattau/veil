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
  bool _closed = false;

  late final int _handle;

  QuicLane({
    required this.endpoint,
    required this.peerId,
    this.bindAddr = "0.0.0.0:0",
    this.trustedPeerCertHex,
  }) {
    _handle = _bindings.start(
      bindAddr,
      _serverNameFromEndpoint(endpoint),
      trustedPeerCertHex ?? "",
    );
    if (_handle == 0) {
      _outboundErr += 1;
    }
  }

  static bool isSupportedSync() {
    return _bindings.isSupported();
  }

  static Future<bool> isSupported() async {
    return isSupportedSync();
  }

  static Future<String?> fetchPinnedCertHex(String endpoint) async {
    final serverName = _serverNameFromEndpoint(endpoint);
    final cert = _bindings.fetchPeerCert(endpoint, serverName);
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
    final result = _bindings.send(peer, bytes);
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
    final msg = _bindings.recv();
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
    _bindings.stop(_handle);
  }
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
      _stop = _lib.lookupFunction<_stopNative, _stopDart>("veil_quic_stop");

  final ffi.DynamicLibrary _lib;
  final _isSupportedDart _isSupported;
  final _startDart _start;
  final _sendDart _send;
  final _recvDart _recv;
  final _fetchPeerCertDart _fetchPeerCert;
  final _freeStringDart _freeString;
  final _freeRecvDart _freeRecv;
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

  int send(String peer, List<int> bytes) {
    final peerPtr = peer.toNativeUtf8();
    final dataPtr = malloc<ffi.Uint8>(bytes.length);
    dataPtr.asTypedList(bytes.length).setAll(0, bytes);
    final result = _send(peerPtr, dataPtr, bytes.length);
    malloc.free(peerPtr);
    malloc.free(dataPtr);
    return result;
  }

  LaneMessage? recv() {
    final ptr = _recv();
    if (ptr == ffi.nullptr) {
      return null;
    }
    final recv = ptr.ref;
    final peer = recv.peer.cast<Utf8>().toDartString();
    final bytes = Uint8List.fromList(recv.data.asTypedList(recv.len));
    _freeRecv(ptr);
    return LaneMessage(peer: peer, bytes: bytes);
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

final _QuicBindings _bindings = _QuicBindings(_openLib());

ffi.DynamicLibrary _openLib() {
  if (Platform.isAndroid) {
    return ffi.DynamicLibrary.open("libveil_sdk_bridge.so");
  }
  if (Platform.isIOS) {
    return ffi.DynamicLibrary.process();
  }
  if (Platform.isMacOS) {
    return ffi.DynamicLibrary.open("libveil_sdk_bridge.dylib");
  }
  if (Platform.isWindows) {
    return ffi.DynamicLibrary.open("veil_sdk_bridge.dll");
  }
  return ffi.DynamicLibrary.open("libveil_sdk_bridge.so");
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
    ffi.Int32 Function(ffi.Pointer<Utf8>, ffi.Pointer<ffi.Uint8>, ffi.IntPtr);
typedef _sendDart =
    int Function(ffi.Pointer<Utf8>, ffi.Pointer<ffi.Uint8>, int);
typedef _recvNative = ffi.Pointer<_QuicRecv> Function();
typedef _recvDart = ffi.Pointer<_QuicRecv> Function();
typedef _fetchPeerCertNative =
    ffi.Pointer<Utf8> Function(ffi.Pointer<Utf8>, ffi.Pointer<Utf8>);
typedef _fetchPeerCertDart =
    ffi.Pointer<Utf8> Function(ffi.Pointer<Utf8>, ffi.Pointer<Utf8>);
typedef _freeStringNative = ffi.Void Function(ffi.Pointer<Utf8>);
typedef _freeStringDart = void Function(ffi.Pointer<Utf8>);
typedef _freeRecvNative = ffi.Void Function(ffi.Pointer<_QuicRecv>);
typedef _freeRecvDart = void Function(ffi.Pointer<_QuicRecv>);
typedef _stopNative = ffi.Void Function(ffi.Uint64);
typedef _stopDart = void Function(int);

final class _QuicRecv extends ffi.Struct {
  external ffi.Pointer<Utf8> peer;

  external ffi.Pointer<ffi.Uint8> data;

  @ffi.IntPtr()
  external int len;
}
