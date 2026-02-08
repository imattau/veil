import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../app_controller.dart';
import 'widgets.dart';
class VaultView extends StatelessWidget {
  final VeilAppController controller;
  final VoidCallback onFindPeers;

  const VaultView({
    super.key,
    required this.controller,
    required this.onFindPeers,
  });

  @override
  Widget build(BuildContext context) {
    final remaining = controller.epochRemainingSeconds;
    final hours = (remaining ~/ 3600).toString().padLeft(2, '0');
    final minutes = ((remaining % 3600) ~/ 60).toString().padLeft(2, '0');
    final seconds = (remaining % 60).toString().padLeft(2, '0');
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Panel(
          title: 'Private Vault',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Private circles use rotating IDs to protect your conversations.',
              ),
              const SizedBox(height: 12),
              Text(
                'Next privacy rotation in $hours:$minutes:$seconds',
                style: Theme.of(
                  context,
                ).textTheme.bodyMedium?.copyWith(color: Colors.white70),
              ),
              if (controller.epochOverlapActive) ...[
                const SizedBox(height: 8),
                Row(
                  children: [
                    const Icon(
                      Icons.swap_horiz,
                      size: 16,
                      color: Color(0xFF38BDF8),
                    ),
                    const SizedBox(width: 6),
                    Text(
                      'Smooth transition window active',
                      style: Theme.of(
                        context,
                      ).textTheme.bodySmall?.copyWith(color: Colors.white60),
                    ),
                  ],
                ),
              ],
              const SizedBox(height: 12),
              Row(
                children: [
                  Expanded(
                    child: OutlinedButton.icon(
                      onPressed: () async {
                        await Clipboard.setData(
                          ClipboardData(text: controller.privateIdHex),
                        );
                        if (!context.mounted) return;
                        ScaffoldMessenger.of(context).showSnackBar(
                          const SnackBar(
                            content: Text('Private ID copied'),
                            behavior: SnackBarBehavior.floating,
                          ),
                        );
                      },
                      icon: const Icon(Icons.vpn_key),
                      label: const Text('Copy Private ID'),
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: FilledButton.icon(
                      onPressed: onFindPeers,
                      icon: const Icon(Icons.person_add),
                      label: const Text('Find peers'),
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        Panel(
          title: 'Private Contacts',
          child: Column(
            children: [
              if (controller.privateContacts.isEmpty)
                Text(
                  'No private contacts yet. Share your Private ID to start.',
                  style: Theme.of(context)
                      .textTheme
                      .bodySmall
                      ?.copyWith(color: Colors.white70),
                )
              else
                ...controller.privateContacts.map(
                  (contact) => ListTile(
                    dense: true,
                    leading: const Icon(Icons.lock, size: 18),
                    title: Text(contact),
                    trailing: IconButton(
                      icon: const Icon(Icons.close),
                      onPressed: () => controller.removePrivateContact(contact),
                    ),
                  ),
                ),
            ],
          ),
        ),
      ],
    );
  }
}
