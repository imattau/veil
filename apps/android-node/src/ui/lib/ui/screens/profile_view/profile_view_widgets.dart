part of '../profile_view.dart';

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

class _ConnectionsSummaryTile extends StatelessWidget {
  final NodeService service;
  final SocialController controller;

  const _ConnectionsSummaryTile({
    required this.service,
    required this.controller,
  });

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final followCount = controller.followedPubkeys.length;
        final mutedCount = controller.mutedPubkeys.length;
        final blockedCount = controller.blockedPubkeys.length;

        return Card(
          elevation: 0,
          color: VeilTheme.surface,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(VeilTheme.radiusS),
            side: BorderSide(color: Colors.white.withValues(alpha: 0.04)),
          ),
          child: InkWell(
            borderRadius: BorderRadius.circular(VeilTheme.radiusS),
            onTap: () {
              Navigator.push(
                context,
                MaterialPageRoute(
                  builder: (context) =>
                      ConnectionsView(service: service, controller: controller),
                ),
              );
            },
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 14),
              child: Row(
                children: [
                  Container(
                    padding: const EdgeInsets.all(8),
                    decoration: BoxDecoration(
                      color: VeilTheme.surfaceHighlight,
                      borderRadius: BorderRadius.circular(VeilTheme.radiusS),
                    ),
                    child: const Icon(
                      Icons.people_outline,
                      color: VeilTheme.textSecondary,
                      size: 20,
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const Text(
                          'Manage Connections',
                          style: TextStyle(
                            fontWeight: FontWeight.w600,
                            fontSize: 14,
                          ),
                        ),
                        const SizedBox(height: 2),
                        Text(
                          '$followCount following \u00B7 $mutedCount muted \u00B7 $blockedCount blocked',
                          style: const TextStyle(
                            color: VeilTheme.textSecondary,
                            fontSize: 12,
                          ),
                        ),
                      ],
                    ),
                  ),
                  const Icon(
                    Icons.chevron_right,
                    color: VeilTheme.textSecondary,
                    size: 20,
                  ),
                ],
              ),
            ),
          ),
        );
      },
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
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(VeilTheme.radiusS),
      ),
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
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(VeilTheme.radiusM),
      ),
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
      rpcUrl: derived.rpcUrl,
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
          color: VeilTheme.surfaceHighlight,
          borderRadius: BorderRadius.circular(VeilTheme.radiusS),
        ),
        child: Icon(icon, color: VeilTheme.textSecondary, size: 20),
      ),
      title: Text(title, style: const TextStyle(fontWeight: FontWeight.w600)),
      subtitle: Text(subtitle, style: Theme.of(context).textTheme.labelSmall),
      onTap: onTap,
    );
  }
}
