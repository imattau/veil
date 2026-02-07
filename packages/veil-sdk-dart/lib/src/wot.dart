enum TrustTier { trusted, known, unknown, muted, blocked }

class LocalWotPolicy {
  final Set<String> _trusted = {};
  final Set<String> _muted = {};
  final Set<String> _blocked = {};

  List<String> get trusted => List.unmodifiable(_trusted);
  List<String> get muted => List.unmodifiable(_muted);
  List<String> get blocked => List.unmodifiable(_blocked);

  void trust(String id) {
    final key = _normalize(id);
    _blocked.remove(key);
    _muted.remove(key);
    _trusted.add(key);
  }

  void mute(String id) {
    final key = _normalize(id);
    _muted.add(key);
    _trusted.remove(key);
  }

  void block(String id) {
    final key = _normalize(id);
    _blocked.add(key);
    _trusted.remove(key);
    _muted.remove(key);
  }

  void untrust(String id) {
    _trusted.remove(_normalize(id));
  }

  void unmute(String id) {
    _muted.remove(_normalize(id));
  }

  void unblock(String id) {
    _blocked.remove(_normalize(id));
  }

  TrustTier classify(String id) {
    final key = _normalize(id);
    if (_blocked.contains(key)) return TrustTier.blocked;
    if (_trusted.contains(key)) return TrustTier.trusted;
    if (_muted.contains(key)) return TrustTier.muted;
    return TrustTier.unknown;
  }

  Map<String, dynamic> exportJson() => {
        "trusted": trusted,
        "muted": muted,
        "blocked": blocked,
      };

  static LocalWotPolicy fromJson(Map<String, dynamic> json) {
    final policy = LocalWotPolicy();
    final trusted = json["trusted"];
    final muted = json["muted"];
    final blocked = json["blocked"];
    if (trusted is List) {
      for (final item in trusted) {
        if (item is String) policy.trust(item);
      }
    }
    if (muted is List) {
      for (final item in muted) {
        if (item is String) policy.mute(item);
      }
    }
    if (blocked is List) {
      for (final item in blocked) {
        if (item is String) policy.block(item);
      }
    }
    return policy;
  }
}

String _normalize(String input) => input.trim().toLowerCase();
