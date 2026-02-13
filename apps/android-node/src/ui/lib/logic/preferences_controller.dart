import 'package:flutter/foundation.dart';
import './node_service.dart';

class PreferencesController extends ChangeNotifier {
  static const String appId = 'veil-social-android';
  final NodeService nodeService;

  PreferencesController(this.nodeService) {
    nodeService.addListener(_onNodeServiceChange);
  }

  void _onNodeServiceChange() {
    notifyListeners();
  }

  Map<String, dynamic> get _myPrefs {
    final self = nodeService.state.identityHex;
    if (self == null) return {};
    final event = nodeService.latestPrefs['$self:$appId'];
    return event?.preferencesJson ?? {};
  }

  // Example settings
  String get theme => _myPrefs['theme'] as String? ?? 'dark';
  bool get notificationsEnabled => _myPrefs['notifications_enabled'] as bool? ?? true;
  String get defaultChannel => _myPrefs['default_channel'] as String? ?? 'general';

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
    await nodeService.publishAppPreferences(
      appId: appId,
      preferencesJson: prefs,
    );
  }

  @override
  void dispose() {
    nodeService.removeListener(_onNodeServiceChange);
    super.dispose();
  }
}
