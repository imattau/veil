class NodeContactConfig {
  final String peerId;
  final String wsUrl;
  final String quicAddr;

  const NodeContactConfig({
    required this.peerId,
    required this.wsUrl,
    required this.quicAddr,
  });
}

NodeContactConfig? deriveNodeContactConfig(String rawInput) {
  final input = rawInput.trim();
  if (input.isEmpty) {
    return null;
  }

  final profileCfg = _fromVeilProfile(input);
  if (profileCfg != null) {
    return profileCfg;
  }

  final explicitUri = Uri.tryParse(input);
  if (explicitUri != null && explicitUri.hasScheme) {
    return _fromUri(explicitUri);
  }

  final normalized = input.contains('/') ? input : '$input/';
  final inferredUri = Uri.tryParse('https://$normalized');
  if (inferredUri == null || inferredUri.host.isEmpty) {
    return null;
  }

  final host = inferredUri.host;
  final wsPort = inferredUri.hasPort ? inferredUri.port : null;
  final wsHostPort = wsPort == null ? host : '$host:$wsPort';
  return NodeContactConfig(
    peerId: host,
    wsUrl: 'wss://$wsHostPort/ws',
    quicAddr: '$host:5000',
  );
}

NodeContactConfig? _fromVeilProfile(String input) {
  final uri = Uri.tryParse(input);
  if (uri == null || uri.scheme != 'veil' || uri.host != 'vps') {
    return null;
  }
  final wsRaw = uri.queryParameters['ws'];
  final quicRaw = uri.queryParameters['quic'];
  if (wsRaw == null || wsRaw.trim().isEmpty) {
    return null;
  }

  final wsUri = Uri.tryParse(wsRaw);
  if (wsUri == null || wsUri.host.isEmpty) {
    return null;
  }
  final host = wsUri.host;
  final wsPath = (wsUri.path.isEmpty || wsUri.path == '/') ? '/ws' : wsUri.path;
  final wsScheme = wsUri.scheme == 'ws' || wsUri.scheme == 'wss'
      ? wsUri.scheme
      : 'wss';
  final wsHostPort = wsUri.hasPort ? '$host:${wsUri.port}' : host;
  final wsUrl = '$wsScheme://$wsHostPort$wsPath';

  String quicAddr = '$host:5000';
  if (quicRaw != null && quicRaw.trim().isNotEmpty) {
    final quicUri = Uri.tryParse(quicRaw.trim());
    if (quicUri != null && quicUri.host.isNotEmpty) {
      final quicPort = quicUri.hasPort ? quicUri.port : 5000;
      quicAddr = '${quicUri.host}:$quicPort';
    }
  }

  return NodeContactConfig(peerId: host, wsUrl: wsUrl, quicAddr: quicAddr);
}

NodeContactConfig? _fromUri(Uri uri) {
  final scheme = uri.scheme.toLowerCase();
  final host = uri.host;
  if (host.isEmpty) {
    return null;
  }

  if (scheme == 'quic') {
    final quicPort = uri.hasPort ? uri.port : 5000;
    return NodeContactConfig(
      peerId: host,
      wsUrl: 'wss://$host/ws',
      quicAddr: '$host:$quicPort',
    );
  }

  if (scheme == 'http' ||
      scheme == 'https' ||
      scheme == 'ws' ||
      scheme == 'wss') {
    final secure = scheme == 'https' || scheme == 'wss';
    final wsScheme = secure ? 'wss' : 'ws';
    final wsPath = (uri.path.isEmpty || uri.path == '/') ? '/ws' : uri.path;
    final wsHostPort = uri.hasPort ? '$host:${uri.port}' : host;
    return NodeContactConfig(
      peerId: host,
      wsUrl: '$wsScheme://$wsHostPort$wsPath',
      quicAddr: '$host:5000',
    );
  }

  return null;
}
