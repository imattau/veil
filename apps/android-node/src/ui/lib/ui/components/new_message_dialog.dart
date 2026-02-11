import 'package:flutter/material.dart';
import '../../logic/messaging_controller.dart';
import '../theme/veil_theme.dart';

class NewMessageDialog extends StatefulWidget {
  final MessagingController controller;

  const NewMessageDialog({super.key, required this.controller});

  @override
  State<NewMessageDialog> createState() => _NewMessageDialogState();
}

class _NewMessageDialogState extends State<NewMessageDialog> {
  final TextEditingController _pubkeyController = TextEditingController();
  final TextEditingController _messageController = TextEditingController();
  bool _isSending = false;

  Future<void> _handleSend() async {
    final pubkey = _pubkeyController.text.trim();
    final text = _messageController.text.trim();
    if (pubkey.isEmpty || text.isEmpty) return;

    setState(() => _isSending = true);
    try {
      await widget.controller.publishDirectMessage(
        recipientPubkey: pubkey,
        text: text,
      );
      if (mounted) Navigator.pop(context);
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Failed to send message: $e')),
        );
      }
    } finally {
      if (mounted) setState(() => _isSending = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('New Message'),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          TextField(
            controller: _pubkeyController,
            decoration: const InputDecoration(
              labelText: 'Recipient Public Key',
              hintText: '64 hex characters',
              labelStyle: TextStyle(color: VeilTheme.textSecondary),
            ),
          ),
          const SizedBox(height: 16),
          TextField(
            controller: _messageController,
            maxLines: 3,
            decoration: const InputDecoration(
              labelText: 'Message',
              labelStyle: TextStyle(color: VeilTheme.textSecondary),
            ),
          ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: const Text('Cancel'),
        ),
        ElevatedButton(
          onPressed: _isSending ? null : _handleSend,
          style: ElevatedButton.styleFrom(
            backgroundColor: VeilTheme.accent,
            foregroundColor: Colors.black,
          ),
          child: _isSending
              ? const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2))
              : const Text('Send'),
        ),
      ],
    );
  }
}
