import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../logic/node_service.dart';
import '../../logic/social_controller.dart';
import '../../logic/list_controller.dart';
import '../../logic/preferences_controller.dart';
import '../../logic/node_contact_config.dart';
import '../theme/veil_theme.dart';
import './profile_edit_view.dart';
import './bookmarks_view.dart';
import './settings_view.dart';
import './connections_view.dart';

part 'profile_view/profile_view_widgets.dart';
part 'profile_view/profile_view_identity_export.dart';

class ProfileView extends StatefulWidget {
  final NodeService service;
  final SocialController controller;
  final ListController? listController;
  final PreferencesController? preferencesController;
  final String? targetPubkey;
  final ScrollController? scrollController;
  final double? topInset;
  final double? bottomInset;

  const ProfileView({
    super.key,
    required this.service,
    required this.controller,
    this.listController,
    this.preferencesController,
    this.targetPubkey,
    this.scrollController,
    this.topInset,
    this.bottomInset,
  });

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
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(VeilTheme.radiusM),
        ),
        title: const Text('Export Identity'),
        content: _ExportIdentityContent(
          publicKeyHex: result['public_key_hex'],
          secretKeyHex: result['secret_key_hex'],
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
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(VeilTheme.radiusM),
        ),
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
        final pubkey =
            widget.targetPubkey ??
            widget.service.state.identityHex ??
            'Unknown';
        final isSelf = pubkey == widget.service.state.identityHex;
        final profile = widget.service.profiles[pubkey];
        final displayName = profile?.displayName ?? 'Set Name';
        final bio = profile?.bio ?? 'Add a bio to your profile';
        final avatarRoot = profile?.avatarMediaRoot;

        final shortPubkey = pubkey.length > 16
            ? '${pubkey.substring(0, 8)}...${pubkey.substring(pubkey.length - 8)}'
            : pubkey;
        final mediaQuery = MediaQuery.of(context);
        final resolvedTopInset =
            widget.topInset ?? (mediaQuery.padding.top + 16);
        final resolvedBottomInset =
            widget.bottomInset ?? (mediaQuery.padding.bottom + 24);

        return ListView(
          controller: widget.scrollController,
          padding: EdgeInsets.fromLTRB(
            24,
            resolvedTopInset,
            24,
            resolvedBottomInset,
          ),
          children: [
            Center(
              child: CircleAvatar(
                radius: 50,
                backgroundColor: VeilTheme.surface,
                backgroundImage:
                    avatarRoot != null &&
                        widget.controller.imageCache.containsKey(avatarRoot)
                    ? MemoryImage(widget.controller.imageCache[avatarRoot]!)
                    : null,
                child:
                    (avatarRoot == null ||
                        !widget.controller.imageCache.containsKey(avatarRoot))
                    ? const Icon(
                        Icons.person,
                        size: 50,
                        color: VeilTheme.accent,
                      )
                    : null,
              ),
            ),
            const SizedBox(height: 16),
            Center(
              child: Text(
                displayName,
                style: const TextStyle(
                  fontSize: 20,
                  fontWeight: FontWeight.bold,
                ),
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
                  IconButton(
                    tooltip: 'Copy public key',
                    constraints: const BoxConstraints(
                      minWidth: VeilTheme.minTapTarget,
                      minHeight: VeilTheme.minTapTarget,
                    ),
                    padding: EdgeInsets.zero,
                    icon: const Icon(
                      Icons.copy,
                      size: 16,
                      color: VeilTheme.accent,
                    ),
                    onPressed: () {
                      Clipboard.setData(ClipboardData(text: pubkey));
                      HapticFeedback.mediumImpact();
                      ScaffoldMessenger.of(context).showSnackBar(
                        const SnackBar(
                          content: Text('Public Key copied to clipboard'),
                        ),
                      );
                    },
                  ),
                ],
              ),
            ),
            const SizedBox(height: 8),
            Center(
              child: Text(
                bio,
                textAlign: TextAlign.center,
                style: const TextStyle(
                  fontSize: 14,
                  color: VeilTheme.textSecondary,
                ),
              ),
            ),
            const SizedBox(height: 32),
            if (isSelf)
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
                        MaterialPageRoute(
                          builder: (context) => ProfileEditView(
                            service: widget.service,
                            controller: widget.controller,
                          ),
                        ),
                      );
                    },
                  ),
                  _ProfileTile(
                    icon: Icons.bolt_outlined,
                    title: 'Lightning Address',
                    subtitle: 'Configure your zap address',
                    onTap: () {
                      Navigator.push(
                        context,
                        MaterialPageRoute(
                          builder: (context) => ProfileEditView(
                            service: widget.service,
                            controller: widget.controller,
                          ),
                        ),
                      );
                    },
                  ),
                  if (widget.listController != null)
                    _ProfileTile(
                      icon: Icons.bookmark_outline,
                      title: 'Bookmarks',
                      subtitle: 'Your saved posts',
                      onTap: () {
                        Navigator.push(
                          context,
                          MaterialPageRoute(
                            builder: (context) => BookmarksView(
                              controller: widget.controller,
                              listController: widget.listController!,
                            ),
                          ),
                        );
                      },
                    ),
                ],
              ),
            if (isSelf) const SizedBox(height: 24),
            _ProfileSection(
              title: 'Connections',
              children: [
                _ConnectionsSummaryTile(
                  service: widget.service,
                  controller: widget.controller,
                ),
              ],
            ),
            if (isSelf) ...[
              const SizedBox(height: 24),
              _ProfileSection(
                title: 'Nodes',
                children: [_NodeContactsCard(service: widget.service)],
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
                  if (widget.preferencesController != null)
                    _ProfileTile(
                      icon: Icons.settings_outlined,
                      title: 'App Settings',
                      subtitle: 'Theme and preferences',
                      onTap: () {
                        Navigator.push(
                          context,
                          MaterialPageRoute(
                            builder: (context) => SettingsView(
                              controller: widget.preferencesController!,
                            ),
                          ),
                        );
                      },
                    ),
                  _ProfileTile(
                    icon: Icons.restore_outlined,
                    title: 'Import Identity',
                    subtitle: 'Restore from secret key',
                    onTap: _importIdentity,
                  ),
                ],
              ),
              const SizedBox(height: 40),
              const Divider(color: Colors.white10),
              Center(
                child: TextButton.icon(
                  onPressed: widget.service.stop,
                  style: TextButton.styleFrom(
                    foregroundColor: Colors.redAccent,
                    padding: const EdgeInsets.symmetric(
                      horizontal: 24,
                      vertical: 12,
                    ),
                  ),
                  icon: const Icon(Icons.power_settings_new, size: 18),
                  label: const Text(
                    'Stop Veil Node',
                    style: TextStyle(fontWeight: FontWeight.bold),
                  ),
                ),
              ),
            ],
          ],
        );
      },
    );
  }
}
