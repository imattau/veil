import 'package:flutter/material.dart';
import '../../logic/preferences_controller.dart';
import '../theme/veil_theme.dart';

class SettingsView extends StatelessWidget {
  final PreferencesController controller;

  const SettingsView({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Settings'),
      ),
      body: ListenableBuilder(
        listenable: controller,
        builder: (context, _) {
          return ListView(
            padding: const EdgeInsets.all(24),
            children: [
              _SettingsSection(
                title: 'Appearance',
                children: [
                  ListTile(
                    title: const Text('Theme'),
                    subtitle: Text(controller.theme.toUpperCase()),
                    trailing: const Icon(Icons.chevron_right),
                    onTap: () => _showThemeDialog(context),
                  ),
                ],
              ),
              const SizedBox(height: 24),
              _SettingsSection(
                title: 'Notifications',
                children: [
                  SwitchListTile(
                    title: const Text('Enable Notifications'),
                    value: controller.notificationsEnabled,
                    onChanged: (val) => controller.updateNotifications(val),
                    activeColor: VeilTheme.accent,
                  ),
                ],
              ),
              const SizedBox(height: 24),
              _SettingsSection(
                title: 'Feed',
                children: [
                  ListTile(
                    title: const Text('Default Channel'),
                    subtitle: Text('#${controller.defaultChannel}'),
                    trailing: const Icon(Icons.chevron_right),
                    onTap: () => _showChannelDialog(context),
                  ),
                ],
              ),
            ],
          );
        },
      ),
    );
  }

  void _showThemeDialog(BuildContext context) {
    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Choose Theme'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: ['dark', 'light', 'amoled'].map((t) {
            return RadioListTile<String>(
              title: Text(t.toUpperCase()),
              value: t,
              groupValue: controller.theme,
              onChanged: (val) {
                if (val != null) controller.updateTheme(val);
                Navigator.pop(context);
              },
            );
          }).toList(),
        ),
      ),
    );
  }

  void _showChannelDialog(BuildContext context) {
    final textController = TextEditingController(text: controller.defaultChannel);
    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Default Channel'),
        content: TextField(
          controller: textController,
          decoration: const InputDecoration(hintText: 'e.g. general'),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () {
              controller.updateDefaultChannel(textController.text.trim());
              Navigator.pop(context);
            },
            child: const Text('Save'),
          ),
        ],
      ),
    );
  }
}

class _SettingsSection extends StatelessWidget {
  final String title;
  final List<Widget> children;

  const _SettingsSection({required this.title, required this.children});

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          title.toUpperCase(),
          style: Theme.of(context).textTheme.labelSmall?.copyWith(
            letterSpacing: 1.2,
            fontWeight: FontWeight.bold,
            color: VeilTheme.accent,
          ),
        ),
        const SizedBox(height: 8),
        Card(
          color: VeilTheme.surface,
          margin: EdgeInsets.zero,
          child: Column(children: children),
        ),
      ],
    );
  }
}
