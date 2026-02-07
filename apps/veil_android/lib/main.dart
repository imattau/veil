import 'package:flutter/material.dart';

import 'app_controller.dart';
import 'ui/compose.dart';
import 'ui/discovery.dart';
import 'ui/home_feed.dart';
import 'ui/network.dart';
import 'ui/onboarding.dart';
import 'ui/profile.dart';
import 'ui/settings.dart';
import 'ui/vault.dart';
import 'ui/widgets.dart';

void main() {
  runApp(const VeilAndroidApp());
}

class VeilAndroidApp extends StatelessWidget {
  const VeilAndroidApp({super.key});

  @override
  Widget build(BuildContext context) {
    final colorSchemeDark = ColorScheme.fromSeed(
      seedColor: const Color(0xFFFB7C31),
      brightness: Brightness.dark,
      primary: const Color(0xFFFFA45C),
      secondary: const Color(0xFF3DD6A8),
      surface: const Color(0xFF0B111B),
      surfaceContainerHighest: const Color(0xFF101827),
    );
    final colorSchemeLight = ColorScheme.fromSeed(
      seedColor: const Color(0xFFFB7C31),
      brightness: Brightness.light,
      primary: const Color(0xFFEF6C00),
      secondary: const Color(0xFF0E9F6E),
      surface: const Color(0xFFF8FAFC),
      surfaceContainerHighest: const Color(0xFFE2E8F0),
    );
    return MaterialApp(
      title: 'VEIL Android',
      themeMode: ThemeMode.system,
      theme: _buildTheme(colorSchemeLight, Brightness.light),
      darkTheme: _buildTheme(colorSchemeDark, Brightness.dark),
      home: const RootShell(),
    );
  }
}

ThemeData _buildTheme(ColorScheme scheme, Brightness brightness) {
  final isDark = brightness == Brightness.dark;
  final background = isDark ? const Color(0xFF0B0F17) : const Color(0xFFF8FAFC);
  final onSurface = isDark ? Colors.white : const Color(0xFF0F172A);
  final border = isDark ? const Color(0xFF1F2937) : const Color(0xFFE2E8F0);
  return ThemeData(
    colorScheme: scheme,
    useMaterial3: true,
    scaffoldBackgroundColor: background,
    appBarTheme: AppBarTheme(
      backgroundColor: background,
      elevation: 0,
      foregroundColor: onSurface,
    ),
    textTheme: (isDark ? ThemeData.dark() : ThemeData.light()).textTheme.copyWith(
          headlineMedium: const TextStyle(
            fontSize: 28,
            fontWeight: FontWeight.w700,
            letterSpacing: -0.4,
          ),
          titleLarge: const TextStyle(
            fontSize: 22,
            fontWeight: FontWeight.w600,
          ),
          titleMedium: const TextStyle(
            fontSize: 18,
            fontWeight: FontWeight.w600,
          ),
          bodyMedium: const TextStyle(
            fontSize: 14,
            height: 1.35,
          ),
        ),
    inputDecorationTheme: InputDecorationTheme(
      filled: true,
      fillColor: isDark ? const Color(0xFF111827) : const Color(0xFFF1F5F9),
      border: OutlineInputBorder(
        borderRadius: BorderRadius.circular(14),
        borderSide: BorderSide(color: border),
      ),
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(14),
        borderSide: BorderSide(color: border),
      ),
      focusedBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(14),
        borderSide: BorderSide(color: scheme.primary, width: 1.2),
      ),
      labelStyle: TextStyle(
        color: isDark ? const Color(0xFF9CA3AF) : const Color(0xFF475569),
      ),
      hintStyle: TextStyle(
        color: isDark ? const Color(0xFF6B7280) : const Color(0xFF64748B),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: scheme.primary,
        foregroundColor: isDark ? const Color(0xFF0B0F17) : Colors.white,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(14),
        ),
        padding: const EdgeInsets.symmetric(vertical: 14, horizontal: 18),
      ),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        side: const BorderSide(color: Color(0xFF1F2937)),
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(14),
        ),
        padding: const EdgeInsets.symmetric(vertical: 14, horizontal: 18),
      ),
    ),
    chipTheme: ChipThemeData(
      backgroundColor: isDark ? const Color(0xFF0F172A) : const Color(0xFFE2E8F0),
      side: const BorderSide(color: Color(0xFF1F2937)),
      labelStyle: TextStyle(color: onSurface),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(999),
      ),
    ),
    navigationBarTheme: NavigationBarThemeData(
      backgroundColor: background,
      indicatorColor: isDark ? const Color(0xFF1F2937) : const Color(0xFFE2E8F0),
      labelTextStyle: WidgetStateProperty.all(
        const TextStyle(fontWeight: FontWeight.w600),
      ),
    ),
  );
}

class RootShell extends StatefulWidget {
  const RootShell({super.key});

  @override
  State<RootShell> createState() => _RootShellState();
}

class _RootShellState extends State<RootShell> {
  final _controller = VeilAppController();
  int _tabIndex = 0;
  bool _showProtocolDetails = false;
  late final Future<void> _initFuture;

  @override
  void initState() {
    super.initState();
    _initFuture = _controller.init();
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _openCompose() {
    Navigator.of(context).push(
      MaterialPageRoute(
        fullscreenDialog: true,
        builder: (context) => ComposeScreen(
          channelLabel: _controller.channelLabel.isNotEmpty
              ? _controller.channelLabel
              : _controller.tagHex.isNotEmpty
                  ? 'Custom tag'
                  : 'General',
          onPublish: (text, attachments) {
            _controller.publishLocalPost(text, attachments: attachments);
            Navigator.of(context).pop();
          },
        ),
      ),
    );
  }

  void _openProfile() {
    showModalBottomSheet(
      context: context,
      showDragHandle: true,
      isScrollControlled: true,
      backgroundColor: Colors.transparent,
      builder: (context) => ProfileSheet(
        controller: _controller,
        onEdit: () {
          Navigator.of(context).pop();
          _openProfileEdit();
        },
      ),
    );
  }

  void _openProfileEdit() {
    showModalBottomSheet(
      context: context,
      showDragHandle: true,
      isScrollControlled: true,
      backgroundColor: Colors.transparent,
      builder: (context) => ProfileEditSheet(controller: _controller),
    );
  }

  void _openSettings() {
    showModalBottomSheet(
      context: context,
      showDragHandle: true,
      isScrollControlled: true,
      backgroundColor: Colors.transparent,
      builder: (context) => SettingsSheet(
        controller: _controller,
        showProtocolDetails: _showProtocolDetails,
        onToggleDetails: (value) {
          setState(() => _showProtocolDetails = value);
        },
        ghostMode: _controller.ghostMode,
        onToggleGhostMode: (value) {
          setState(() => _controller.setGhostMode(value));
        },
        requireSignedPublic: _controller.requireSignedPublic,
        onToggleRequireSigned: (value) {
          setState(() => _controller.setRequireSignedPublic(value));
        },
        clockSkewSeconds: _controller.clockSkewSeconds,
        onClockSkewChanged: (value) {
          setState(() => _controller.setClockSkewSeconds(value));
        },
        maxCacheEntries: _controller.maxCacheEntries,
        maxPublishQueue: _controller.maxPublishQueue,
        onMaxCacheEntriesChanged: (value) {
          setState(() => _controller.setMaxCacheEntries(value));
        },
        onMaxPublishQueueChanged: (value) {
          setState(() => _controller.setMaxPublishQueue(value));
        },
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<void>(
      future: _initFuture,
      builder: (context, snapshot) {
        if (snapshot.connectionState != ConnectionState.done) {
          return const Scaffold(
            body: Center(child: CircularProgressIndicator()),
          );
        }
        if (!_controller.onboardingComplete) {
          return OnboardingScreen(
            controller: _controller,
            onComplete: () => setState(() {}),
          );
        }
        return Scaffold(
          appBar: AppBar(
            titleSpacing: 12,
            title: GestureDetector(
              onTap: _openProfile,
              child: Row(
                children: [
                  _controller.profileAvatar?.isImage == true
                      ? CircleAvatar(
                          radius: 16,
                          backgroundImage:
                              MemoryImage(_controller.profileAvatar!.bytes),
                        )
                      : Image.asset(
                          'assets/veil_logo.png',
                          width: 28,
                          height: 28,
                        ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          _controller.displayName.isEmpty
                              ? 'Operator'
                              : _controller.displayName,
                          style: Theme.of(context).textTheme.titleMedium,
                          overflow: TextOverflow.ellipsis,
                        ),
                        Text(
                          _controller.namespaceChoice,
                          style: Theme.of(context)
                              .textTheme
                              .bodySmall
                              ?.copyWith(color: Colors.white70),
                          overflow: TextOverflow.ellipsis,
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
            actions: [
              Chip(
                label: Text(_controller.connectionStatus),
                backgroundColor: _controller.connectionStatus == 'LIVE'
                    ? const Color(0xFF134E4A)
                    : _controller.connectionStatus == 'DEGRADED'
                        ? const Color(0xFF3F2F0B)
                        : const Color(0xFF3B1D1D),
              ),
              const SizedBox(width: 8),
              IconButton(
                icon: const Icon(Icons.tune),
                onPressed: _openSettings,
              ),
            ],
          ),
          body: AnimatedBuilder(
            animation: _controller,
            builder: (context, _) {
              return IndexedStack(
                index: _tabIndex,
                children: [
                  HomeFeed(
                    controller: _controller,
                    showProtocolDetails: _showProtocolDetails,
                  ),
                  VaultView(controller: _controller),
                  NetworkView(controller: _controller),
                  DiscoveryView(controller: _controller),
                ],
              );
            },
          ),
          floatingActionButton: FloatingActionButton.extended(
            onPressed: _openCompose,
            icon: const Icon(Icons.edit),
            label: const Text('Compose'),
          ),
          bottomNavigationBar: NavigationBar(
            selectedIndex: _tabIndex,
            onDestinationSelected: (index) =>
                setState(() => _tabIndex = index),
            destinations: const [
              NavigationDestination(
                icon: Icon(Icons.dynamic_feed),
                label: 'Feed',
              ),
              NavigationDestination(
                icon: Icon(Icons.lock),
                label: 'Vault',
              ),
              NavigationDestination(
                icon: Icon(Icons.network_check),
                label: 'Network',
              ),
              NavigationDestination(
                icon: Icon(Icons.explore),
                label: 'Discovery',
              ),
            ],
          ),
        );
      },
    );
  }
}
