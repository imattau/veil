import 'dart:async';
import 'package:flutter/foundation.dart';
import './node_service.dart';

class PreferencesController extends ChangeNotifier {
  static const String appId = 'veil-social-android';
  final NodeService nodeService;
  Future<void>? _pendingOp;
  bool _isDisposed = false;
  Map<String, dynamic>? _optimisticPrefs;

  PreferencesController(this.nodeService) {
    nodeService.addListener(_onNodeServiceChange);
  }

  void _onNodeServiceChange() {
    if (_isDisposed) return;
    notifyListeners();
  }

  @override
  void notifyListeners() {
    if (_isDisposed) return;
    super.notifyListeners();
  }

  Map<String, dynamic> get _myPrefs {
    final self = nodeService.state.identityHex;
    if (self == null) return {};
    final event = nodeService.latestPrefs['$self:$appId'];
    return event?.preferencesJson ?? {};
  }

  Map<String, dynamic> get _effectivePrefs {
    final merged = Map<String, dynamic>.from(_myPrefs);
    if (_optimisticPrefs != null) {
      merged.addAll(_optimisticPrefs!);
    }
    return merged;
  }

  // Example settings
  String get theme => _effectivePrefs['theme'] as String? ?? 'dark';
  bool get notificationsEnabled =>
      _effectivePrefs['notifications_enabled'] as bool? ?? true;
  String get defaultChannel =>
      _effectivePrefs['default_channel'] as String? ?? 'general';

  Future<void> updateTheme(String theme) async {
    final prefs = Map<String, dynamic>.from(_myPrefs);
    prefs['theme'] = theme;
    await _save(prefs);
  }

  Future<void> updateNotifications(bool enabled) async {
    final prefs = Map<String, dynamic>.from(_myPrefs);
    prefs['notifications_enabled'] = enabled;
    await _save(prefs);
  }

  Future<void> updateDefaultChannel(String channel) async {
    final prefs = Map<String, dynamic>.from(_myPrefs);
    prefs['default_channel'] = channel;
    await _save(prefs);
  }

  Future<void> _save(Map<String, dynamic> prefs) async {
    final completer = Completer<void>();
    final previous = _pendingOp;
    _pendingOp = completer.future;

    if (previous != null) {
      await previous;
    }

    try {
      if (_isDisposed) return;
      _optimisticPrefs = Map<String, dynamic>.from(prefs);
      notifyListeners();
      final ok = await nodeService.publishAppPreferences(
        appId: appId,
        preferencesJson: prefs,
      );
      if (ok) {
        _optimisticPrefs = null;
        notifyListeners();
      }
    } finally {
      completer.complete();
    }
  }

  @override
  void dispose() {
    _isDisposed = true;
    nodeService.removeListener(_onNodeServiceChange);
    super.dispose();
  }
}
