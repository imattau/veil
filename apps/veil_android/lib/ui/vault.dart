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
              const SizedBox(height: 12),
              FilledButton.icon(
                onPressed: () => _openNewMessage(context, controller),
                icon: const Icon(Icons.chat_bubble_outline),
                label: const Text('New message'),
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        Panel(
          title: 'Private Contacts',
          child: Column(
            children: [
              _ContactAddRow(controller: controller),
              const SizedBox(height: 8),
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
        const SizedBox(height: 16),
        Panel(
          title: 'Recent DMs',
          child: Column(
            children: controller.privateMessages
                .take(8)
                .map(
                  (msg) => ListTile(
                    dense: true,
                    leading: Icon(
                      msg.incoming ? Icons.inbox : Icons.send,
                      size: 18,
                    ),
                    title: Text(
                      msg.body,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                    ),
                    subtitle: Text(
                      msg.incoming ? 'From ${msg.from}' : 'To ${msg.to}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                )
                .toList(),
          ),
        ),
      ],
    );
  }
}

class _ContactAddRow extends StatefulWidget {
  final VeilAppController controller;

  const _ContactAddRow({required this.controller});

  @override
  State<_ContactAddRow> createState() => _ContactAddRowState();
}

class _ContactAddRowState extends State<_ContactAddRow> {
  final _controller = TextEditingController();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Expanded(
          child: TextField(
            controller: _controller,
            decoration: const InputDecoration(
              labelText: 'Add contact ID',
              hintText: 'tag:HEX or HEX',
            ),
          ),
        ),
        const SizedBox(width: 8),
        IconButton(
          onPressed: () {
            widget.controller.addPrivateContact(_controller.text);
            _controller.clear();
            ScaffoldMessenger.of(context).showSnackBar(
              const SnackBar(
                content: Text('Private contact added'),
                behavior: SnackBarBehavior.floating,
              ),
            );
          },
          icon: const Icon(Icons.add_circle_outline),
        ),
      ],
    );
  }
}

void _openNewMessage(BuildContext context, VeilAppController controller) {
  showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    showDragHandle: true,
    backgroundColor: Colors.transparent,
    builder: (context) => _NewMessageSheet(controller: controller),
  );
}

class _NewMessageSheet extends StatefulWidget {
  final VeilAppController controller;

  const _NewMessageSheet({required this.controller});

  @override
  State<_NewMessageSheet> createState() => _NewMessageSheetState();
}

class _NewMessageSheetState extends State<_NewMessageSheet> {
  final _bodyController = TextEditingController();
  final _toController = TextEditingController();

  @override
  void dispose() {
    _bodyController.dispose();
    _toController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final controller = widget.controller;
    final contacts = controller.privateContacts;
    return SafeArea(
      child: ClipRRect(
        borderRadius: const BorderRadius.vertical(top: Radius.circular(24)),
        child: Container(
          color: Theme.of(context).colorScheme.surface.withOpacity(0.95),
          padding: EdgeInsets.only(
            left: 16,
            right: 16,
            top: 16,
            bottom: 16 + MediaQuery.of(context).viewInsets.bottom,
          ),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'New private message',
                style: Theme.of(context).textTheme.titleLarge,
              ),
              const SizedBox(height: 12),
              DropdownButtonFormField<String>(
                value: contacts.isNotEmpty ? contacts.first : null,
                items: contacts
                    .map(
                      (c) => DropdownMenuItem(
                        value: c,
                        child: Text(c, overflow: TextOverflow.ellipsis),
                      ),
                    )
                    .toList(),
                onChanged: (value) {
                  if (value != null) {
                    _toController.text = value;
                  }
                },
                decoration: const InputDecoration(labelText: 'To'),
              ),
              const SizedBox(height: 8),
              TextField(
                controller: _toController,
                decoration: const InputDecoration(
                  labelText: 'Or paste private ID',
                ),
              ),
              const SizedBox(height: 12),
              TextField(
                controller: _bodyController,
                maxLines: 4,
                decoration: const InputDecoration(labelText: 'Message'),
              ),
              const SizedBox(height: 12),
              FilledButton.icon(
                onPressed: () async {
                  await controller.sendDirectMessage(
                    _toController.text,
                    _bodyController.text,
                  );
                  if (!context.mounted) return;
                  Navigator.of(context).pop();
                  ScaffoldMessenger.of(context).showSnackBar(
                    const SnackBar(
                      content: Text('Message queued'),
                      behavior: SnackBarBehavior.floating,
                    ),
                  );
                },
                icon: const Icon(Icons.send),
                label: const Text('Send'),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
