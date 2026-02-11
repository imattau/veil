import 'package:flutter/material.dart';

class PublishCard extends StatefulWidget {
  final bool busy;
  final Future<void> Function(String payload) onPublish;

  const PublishCard({
    super.key,
    required this.busy,
    required this.onPublish,
  });

  @override
  State<PublishCard> createState() => _PublishCardState();
}

class _PublishCardState extends State<PublishCard> {
  final TextEditingController _controller = TextEditingController();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Card(
      elevation: 0,
      color: Colors.white,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Publish',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _controller,
              minLines: 2,
              maxLines: 4,
              decoration: const InputDecoration(
                hintText: 'Write a test payload to publish…',
                border: OutlineInputBorder(),
              ),
            ),
            const SizedBox(height: 12),
            Align(
              alignment: Alignment.centerRight,
              child: ElevatedButton.icon(
                onPressed: widget.busy
                    ? null
                    : () async {
                        final payload = _controller.text.trim();
                        await widget.onPublish(payload);
                      },
                icon: const Icon(Icons.send),
                label: const Text('Queue Publish'),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
