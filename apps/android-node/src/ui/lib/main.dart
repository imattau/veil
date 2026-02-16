import 'package:flutter/material.dart';
import './logic/node_service.dart';
import './logic/preferences_controller.dart';
import './ui/theme/veil_theme.dart';
import './ui/screens/social_home.dart';

void main() {
  runApp(const VeilApp());
}

class VeilApp extends StatefulWidget {
  final NodeService? service;
  final PreferencesController? preferencesController;

  const VeilApp({super.key, this.service, this.preferencesController});

  @override
  State<VeilApp> createState() => _VeilAppState();
}

class _VeilAppState extends State<VeilApp> {
  late final NodeService _service;
  late final PreferencesController _preferencesController;
  late final bool _ownsService;
  late final bool _ownsPreferences;

  @override
  void initState() {
    super.initState();
    _ownsService = widget.service == null;
    _service = widget.service ?? NodeService();
    _ownsPreferences = widget.preferencesController == null;
    _preferencesController =
        widget.preferencesController ?? PreferencesController(_service);
    _preferencesController.addListener(_handlePreferenceChange);
    _service.start();
  }

  void _handlePreferenceChange() {
    if (!mounted) return;
    setState(() {});
  }

  @override
  void dispose() {
    _preferencesController.removeListener(_handlePreferenceChange);
    if (_ownsPreferences) {
      _preferencesController.dispose();
    }
    if (_ownsService) {
      _service.dispose();
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final preference = _preferencesController.theme.toLowerCase();
    final usesLightTheme = preference == 'light';
    final resolvedDarkTheme = preference == 'amoled'
        ? VeilTheme.amoled
        : VeilTheme.dark;

    return MaterialApp(
      title: 'VEIL Social',
      debugShowCheckedModeBanner: false,
      theme: VeilTheme.light,
      darkTheme: resolvedDarkTheme,
      themeMode: usesLightTheme ? ThemeMode.light : ThemeMode.dark,
      home: SocialHome(
        service: _service,
        preferencesController: _preferencesController,
      ),
    );
  }
}
