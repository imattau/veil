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

class ProfileView extends StatefulWidget {
  final NodeService service;
  final SocialController controller;
  final ListController? listController;
  final PreferencesController? preferencesController;
  final String? targetPubkey;

  const ProfileView({
    super.key,
    required this.service,
    required this.controller,
    this.listController,
    this.preferencesController,
    this.targetPubkey,
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
        title: const Text('Export Identity'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text('Public Key:'),
            SelectableText(
              result['public_key_hex'],
              style: const TextStyle(fontFamily: 'monospace', fontSize: 12),
            ),
            const SizedBox(height: 12),
            const Text(
              'Secret Key (KEEP PRIVATE!):',
              style: TextStyle(color: Colors.red, fontWeight: FontWeight.bold),
            ),
            SelectableText(
              result['secret_key_hex'],
              style: const TextStyle(fontFamily: 'monospace', fontSize: 12),
            ),
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
        final pubkey = widget.targetPubkey ?? widget.service.state.identityHex ?? 'Unknown';
        final isSelf = pubkey == widget.service.state.identityHex;
        final profile = widget.service.profiles[pubkey];
        final displayName =
            profile?.data['display_name'] as String? ?? 'Set Name';
        final bio =
            profile?.data['bio'] as String? ?? 'Add a bio to your profile';
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
                    GestureDetector(
                      onTap: () {
                        Clipboard.setData(ClipboardData(text: pubkey));
                        HapticFeedback.mediumImpact();
                        ScaffoldMessenger.of(context).showSnackBar(
                          const SnackBar(
                            content: Text('Public Key copied to clipboard'),
                          ),
                        );
                      },
                      child: const Icon(
                        Icons.copy,
                        size: 14,
                        color: VeilTheme.accent,
                      ),
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
                      onTap: () {},
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
                  _ConnectionsCard(
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
                TextButton(
                  onPressed: widget.service.stop,
                  style: TextButton.styleFrom(foregroundColor: Colors.red),
                  child: const Text('Stop Veil Node'),
                ),
              ],
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

class _ConnectionsCard extends StatefulWidget {
  final NodeService service;
  final SocialController controller;

  const _ConnectionsCard({required this.service, required this.controller});

  @override
  State<_ConnectionsCard> createState() => _ConnectionsCardState();
}

class _ConnectionsCardState extends State<_ConnectionsCard> {
  final TextEditingController _pubkeyController = TextEditingController();
  String _selectedAction = 'follow';

  @override
  void dispose() {
    _pubkeyController.dispose();
    super.dispose();
  }

  Future<void> _applyAction() async {
    final pubkey = _pubkeyController.text.trim().toLowerCase();
    if (!RegExp(r'^[0-9a-f]{64}$').hasMatch(pubkey)) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Enter a valid 64-char hex pubkey')),
      );
      return;
    }
    switch (_selectedAction) {
      case 'follow':
        await widget.service.followPubkey(pubkey);
        break;
      case 'unfollow':
        await widget.service.unfollowPubkey(pubkey);
        break;
      case 'mute':
        await widget.service.mutePubkey(pubkey);
        break;
      case 'unmute':
        await widget.service.unmutePubkey(pubkey);
        break;
      case 'block':
        await widget.service.blockPubkey(pubkey);
        break;
      case 'unblock':
        await widget.service.unblockPubkey(pubkey);
        break;
    }
    if (!mounted) return;
    final err = widget.service.state.lastError;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(
          err ??
              switch (_selectedAction) {
                'follow' => 'Followed',
                'unfollow' => 'Unfollowed',
                'mute' => 'Muted',
                'unmute' => 'Unmuted',
                'block' => 'Blocked',
                _ => 'Unblocked',
              },
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final following = widget.controller.followedPubkeys.toList()..sort();
    final muted = widget.controller.mutedPubkeys.toList()..sort();
    final blocked = widget.controller.blockedPubkeys.toList()..sort();

    return Card(
      elevation: 0,
      color: VeilTheme.surface,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text(
              'Manage follow/mute/block',
              style: TextStyle(fontWeight: FontWeight.w600),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: _pubkeyController,
              style: const TextStyle(fontFamily: 'monospace', fontSize: 12),
              decoration: const InputDecoration(
                labelText: 'Pubkey (hex)',
                hintText: '64 hex characters',
                border: OutlineInputBorder(),
                isDense: true,
              ),
            ),
            const SizedBox(height: 10),
            Row(
              children: [
                Expanded(
                  child: DropdownButtonFormField<String>(
                    value: _selectedAction,
                    items: const [
                      DropdownMenuItem(value: 'follow', child: Text('Follow')),
                      DropdownMenuItem(
                        value: 'unfollow',
                        child: Text('Unfollow'),
                      ),
                      DropdownMenuItem(value: 'mute', child: Text('Mute')),
                      DropdownMenuItem(value: 'unmute', child: Text('Unmute')),
                      DropdownMenuItem(value: 'block', child: Text('Block')),
                      DropdownMenuItem(
                        value: 'unblock',
                        child: Text('Unblock'),
                      ),
                    ],
                    onChanged: widget.service.state.busy
                        ? null
                        : (value) {
                            if (value != null) {
                              setState(() => _selectedAction = value);
                            }
                          },
                    decoration: const InputDecoration(
                      border: OutlineInputBorder(),
                      isDense: true,
                    ),
                  ),
                ),
                const SizedBox(width: 10),
                ElevatedButton(
                  onPressed: widget.service.state.busy ? null : _applyAction,
                  child: const Text('Apply'),
                ),
              ],
            ),
            const SizedBox(height: 14),
            _PubkeyList(
              title: 'Following',
              pubkeys: following,
              onRemove: (value) => widget.service.unfollowPubkey(value),
            ),
            const SizedBox(height: 8),
            _PubkeyList(
              title: 'Muted',
              pubkeys: muted,
              onRemove: (value) => widget.service.unmutePubkey(value),
            ),
            const SizedBox(height: 8),
            _PubkeyList(
              title: 'Blocked',
              pubkeys: blocked,
              onRemove: (value) => widget.service.unblockPubkey(value),
            ),
          ],
        ),
      ),
    );
  }
}

class _PubkeyList extends StatelessWidget {
  final String title;
  final List<String> pubkeys;
  final Future<void> Function(String pubkey) onRemove;

  const _PubkeyList({
    required this.title,
    required this.pubkeys,
    required this.onRemove,
  });

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title, style: const TextStyle(fontWeight: FontWeight.w600)),
        const SizedBox(height: 6),
        if (pubkeys.isEmpty)
          const Text(
            'None',
            style: TextStyle(color: VeilTheme.textSecondary, fontSize: 12),
          )
        else
          Wrap(
            spacing: 6,
            runSpacing: 6,
            children: pubkeys
                .map(
                  (value) => InputChip(
                    label: Text(
                      value.length > 12
                          ? '${value.substring(0, 12)}...'
                          : value,
                      style: const TextStyle(fontFamily: 'monospace'),
                    ),
                    onPressed: () {
                      Clipboard.setData(ClipboardData(text: value));
                      ScaffoldMessenger.of(context).showSnackBar(
                        const SnackBar(content: Text('Pubkey copied')),
                      );
                    },
                    onDeleted: () async => await onRemove(value),
                  ),
                )
                .toList(),
          ),
      ],
    );
  }
}

class _NodeContactsCard extends StatelessWidget {
  final NodeService service;

  const _NodeContactsCard({required this.service});

  @override
  Widget build(BuildContext context) {
    final contacts = service.contacts;
    return Card(
      elevation: 0,
      color: VeilTheme.surface,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                const Expanded(
                  child: Text(
                    'Connected Nodes',
                    style: TextStyle(fontWeight: FontWeight.w700),
                  ),
                ),
                IconButton(
                  tooltip: 'Add node',
                  onPressed: () => _showNodeContactDialog(context, service),
                  icon: const Icon(Icons.add_circle_outline),
                ),
              ],
            ),
            if (contacts.isEmpty)
              const Text(
                'No nodes configured. Add a VPS node to enable outbound relay.',
                style: TextStyle(color: VeilTheme.textSecondary, fontSize: 12),
              )
            else
              ...contacts.map(
                (contact) => _NodeContactTile(
                  contact: contact,
                  onEdit: () => _showNodeContactDialog(
                    context,
                    service,
                    existing: contact,
                  ),
                  onDelete: () async {
                    final peerId = (contact['peer_id'] as String?) ?? '';
                    await service.deleteContact(peerId);
                  },
                ),
              ),
          ],
        ),
      ),
    );
  }
}

class _NodeContactTile extends StatelessWidget {
  final Map<String, dynamic> contact;
  final VoidCallback onEdit;
  final Future<void> Function() onDelete;

  const _NodeContactTile({
    required this.contact,
    required this.onEdit,
    required this.onDelete,
  });

  @override
  Widget build(BuildContext context) {
    final peerId = (contact['peer_id'] as String?) ?? '';
    final wsUrl = (contact['ws_url'] as String?) ?? '';
    final quicAddr = (contact['quic_addr'] as String?) ?? '';
    return ListTile(
      contentPadding: EdgeInsets.zero,
      title: Text(peerId.isEmpty ? '(unnamed)' : peerId),
      subtitle: Text(
        [
          if (wsUrl.isNotEmpty) 'WS: $wsUrl',
          if (quicAddr.isNotEmpty) 'QUIC: $quicAddr',
        ].join('\n'),
      ),
      isThreeLine: wsUrl.isNotEmpty && quicAddr.isNotEmpty,
      trailing: Wrap(
        spacing: 0,
        children: [
          IconButton(
            tooltip: 'Edit',
            onPressed: onEdit,
            icon: const Icon(Icons.edit_outlined),
          ),
          IconButton(
            tooltip: 'Delete',
            onPressed: () async => await onDelete(),
            icon: const Icon(Icons.delete_outline),
          ),
        ],
      ),
    );
  }
}

Future<void> _showNodeContactDialog(
  BuildContext context,
  NodeService service, {
  Map<String, dynamic>? existing,
}) async {
  final existingWsUrl = (existing?['ws_url'] as String?) ?? '';
  final existingQuicAddr = (existing?['quic_addr'] as String?) ?? '';
  final existingPeerId = (existing?['peer_id'] as String?) ?? '';
  final initialAddress = (() {
    final wsUri = Uri.tryParse(existingWsUrl);
    if (wsUri != null && wsUri.host.isNotEmpty) {
      return wsUri.hasPort ? '${wsUri.host}:${wsUri.port}' : wsUri.host;
    }
    if (existingPeerId.isNotEmpty) {
      return existingPeerId;
    }
    if (existingQuicAddr.isNotEmpty) {
      return existingQuicAddr;
    }
    return '';
  })();
  final addressController = TextEditingController(text: initialAddress);

  final saved = await showDialog<bool>(
    context: context,
    builder: (ctx) => AlertDialog(
      title: Text(existing == null ? 'Add Node' : 'Edit Node'),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              controller: addressController,
              decoration: const InputDecoration(
                labelText: 'Node Address',
                hintText: 'veilnode.3nostr.com',
                helperText:
                    'Just enter address. WS/QUIC settings are auto-configured.',
              ),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(ctx, false),
          child: const Text('Cancel'),
        ),
        ElevatedButton(
          onPressed: () => Navigator.pop(ctx, true),
          child: const Text('Save'),
        ),
      ],
    ),
  );

  if (saved == true) {
    final derived = deriveNodeContactConfig(addressController.text);
    if (derived == null) {
      if (!context.mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(const SnackBar(content: Text('Invalid node address')));
      return;
    }
    await service.saveContact(
      peerId: derived.peerId,
      wsUrl: derived.wsUrl,
      quicAddr: derived.quicAddr,
    );
    if (!context.mounted) return;
    final err = service.state.lastError;
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(err ?? 'Node saved')));
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
