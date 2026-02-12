import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../logic/node_service.dart';
import '../../logic/social_controller.dart';
import '../theme/veil_theme.dart';
import './profile_edit_view.dart';

class ProfileView extends StatefulWidget {
  final NodeService service;
  final SocialController controller;

  const ProfileView({super.key, required this.service, required this.controller});

  @override
  State<ProfileView> createState() => _ProfileViewState();
}

class _ProfileViewState extends State<ProfileView> {
  Future<void> _exportIdentity() async {
    final result = await widget.service.exportIdentity();
    if (result == null) return;
    if (!mounted) return;

    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Export Identity'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text('Public Key:'),
            SelectableText(result['public_key_hex'],
                style: const TextStyle(fontFamily: 'monospace', fontSize: 12)),
            const SizedBox(height: 12),
            const Text('Secret Key (KEEP PRIVATE!):',
                style: TextStyle(color: Colors.red, fontWeight: FontWeight.bold)),
            SelectableText(result['secret_key_hex'],
                style: const TextStyle(fontFamily: 'monospace', fontSize: 12)),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Close'),
          ),
        ],
      ),
    );
  }

  Future<void> _importIdentity() async {
    final controller = TextEditingController();
    final result = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Import Identity'),
        content: TextField(
          controller: controller,
          decoration: const InputDecoration(
            labelText: 'Secret Key (Hex)',
            hintText: '64 hex characters',
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.pop(context, controller.text),
            child: const Text('Import'),
          ),
        ],
      ),
    );

    if (result != null && result.isNotEmpty) {
      await widget.service.importIdentity(result);
    }
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: widget.service,
      builder: (context, _) {
        final pubkey = widget.service.state.identityHex ?? 'Unknown';
        final profile = widget.service.profiles[pubkey];
        final displayName = profile?.data['display_name'] as String? ?? 'Set Name';
        final bio = profile?.data['bio'] as String? ?? 'Add a bio to your profile';
        final avatarRoot = profile?.data['avatar_media_root'] as String?;

        final shortPubkey = pubkey.length > 16 
            ? '${pubkey.substring(0, 8)}...${pubkey.substring(pubkey.length - 8)}'
            : pubkey;

        return SafeArea(
          bottom: false,
          child: ListView(
            padding: const EdgeInsets.fromLTRB(24, 16, 24, 100),
            children: [
              Center(
                child: CircleAvatar(
                  radius: 50,
                  backgroundColor: VeilTheme.surface,
                  backgroundImage: avatarRoot != null && widget.controller.imageCache.containsKey(avatarRoot)
                    ? MemoryImage(widget.controller.imageCache[avatarRoot]!)
                    : null,
                  child: (avatarRoot == null || !widget.controller.imageCache.containsKey(avatarRoot))
                    ? const Icon(Icons.person, size: 50, color: VeilTheme.accent)
                    : null,
                ),
              ),
              const SizedBox(height: 16),
              Center(
                child: Text(
                  displayName,
                  style: const TextStyle(fontSize: 20, fontWeight: FontWeight.bold),
                ),
              ),
              Center(
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      shortPubkey,
                      style: const TextStyle(
                        fontFamily: 'monospace',
                        fontSize: 12,
                        color: VeilTheme.textSecondary,
                      ),
                    ),
                    const SizedBox(width: 8),
                    GestureDetector(
                      onTap: () {
                        Clipboard.setData(ClipboardData(text: pubkey));
                        HapticFeedback.mediumImpact();
                        ScaffoldMessenger.of(context).showSnackBar(
                          const SnackBar(content: Text('Public Key copied to clipboard')),
                        );
                      },
                      child: const Icon(Icons.copy, size: 14, color: VeilTheme.accent),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 8),
              Center(
                child: Text(
                  bio,
                  textAlign: TextAlign.center,
                  style: const TextStyle(fontSize: 14, color: VeilTheme.textSecondary),
                ),
              ),
              const SizedBox(height: 32),
              _ProfileSection(
                title: 'Account',
                children: [
                  _ProfileTile(
                    icon: Icons.badge_outlined,
                    title: 'Edit Profile',
                    subtitle: 'Set your name and bio',
                    onTap: () {
                      Navigator.push(
                        context,
                        MaterialPageRoute(builder: (context) => ProfileEditView(service: widget.service)),
                      );
                    },
                  ),
                  _ProfileTile(
                    icon: Icons.bolt_outlined,
                    title: 'Lightning Address',
                    subtitle: 'Configure your zap address',
                    onTap: () {},
                  ),
                ],
              ),
              const SizedBox(height: 24),
              _ProfileSection(
                title: 'Security',
                children: [
                  _ProfileTile(
                    icon: Icons.key_outlined,
                    title: 'Backup Identity',
                    subtitle: 'Export your secret keys',
                    onTap: _exportIdentity,
                  ),
                  _ProfileTile(
                    icon: Icons.settings_outlined,
                    title: 'Import Identity',
                    subtitle: 'Restore from secret key',
                    onTap: _importIdentity,
                  ),
                ],
              ),
              const SizedBox(height: 40),
              TextButton(
                onPressed: widget.service.stop,
                style: TextButton.styleFrom(foregroundColor: Colors.red),
                child: const Text('Stop Veil Node'),
              ),
            ],
          ),
        );
      },
    );
  }
}

class _ProfileSection extends StatelessWidget {
  final String title;
  final List<Widget> children;

  const _ProfileSection({required this.title, required this.children});

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
          ),
        ),
        const SizedBox(height: 8),
        ...children,
      ],
    );
  }
}

class _ProfileTile extends StatelessWidget {
  final IconData icon;
  final String title;
  final String subtitle;
  final VoidCallback onTap;

  const _ProfileTile({
    required this.icon,
    required this.title,
    required this.subtitle,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return ListTile(
      contentPadding: EdgeInsets.zero,
      leading: Container(
        padding: const EdgeInsets.all(8),
        decoration: BoxDecoration(
          color: Colors.white.withOpacity(0.05),
          borderRadius: BorderRadius.circular(8),
        ),
        child: Icon(icon, color: VeilTheme.textSecondary, size: 20),
      ),
      title: Text(title, style: const TextStyle(fontWeight: FontWeight.w600)),
      subtitle: Text(subtitle, style: Theme.of(context).textTheme.labelSmall),
      onTap: onTap,
    );
  }
}
