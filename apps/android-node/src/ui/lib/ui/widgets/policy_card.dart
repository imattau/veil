import 'package:flutter/material.dart';

class PolicyCard extends StatefulWidget {
  final Map<String, dynamic> summary;
  final bool busy;
  final Future<void> Function(String action, String pubkeyHex) onAction;
  final Future<Map<String, dynamic>?> Function(String pubkeyHex) onExplain;

  const PolicyCard({
    super.key,
    required this.summary,
    required this.busy,
    required this.onAction,
    required this.onExplain,
  });

  @override
  State<PolicyCard> createState() => _PolicyCardState();
}

class _PolicyCardState extends State<PolicyCard> {
  final TextEditingController _controller = TextEditingController();
  String _action = 'trust';
  Map<String, dynamic>? _explanation;

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final trusted = widget.summary['trusted']?.toString() ?? '0';
    final muted = widget.summary['muted']?.toString() ?? '0';
    final blocked = widget.summary['blocked']?.toString() ?? '0';
    final endorsements = widget.summary['endorsements']?.toString() ?? '0';

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
              'Trust Policy',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 12),
            _MetricRow(label: 'Trusted', value: trusted),
            _MetricRow(label: 'Muted', value: muted),
            _MetricRow(label: 'Blocked', value: blocked),
            _MetricRow(label: 'Endorsements', value: endorsements),
            const Divider(height: 24),
            TextField(
              controller: _controller,
              decoration: const InputDecoration(
                hintText: 'Pubkey hex (64 chars)',
                border: OutlineInputBorder(),
              ),
            ),
            const SizedBox(height: 12),
            Row(
              children: [
                Expanded(
                  child: DropdownButtonFormField<String>(
                    initialValue: _action,
                    decoration: const InputDecoration(
                      border: OutlineInputBorder(),
                    ),
                    items: const [
                      DropdownMenuItem(value: 'trust', child: Text('Trust')),
                      DropdownMenuItem(value: 'untrust', child: Text('Untrust')),
                      DropdownMenuItem(value: 'mute', child: Text('Mute')),
                      DropdownMenuItem(value: 'unmute', child: Text('Unmute')),
                      DropdownMenuItem(value: 'block', child: Text('Block')),
                      DropdownMenuItem(value: 'unblock', child: Text('Unblock')),
                    ],
                    onChanged: widget.busy
                        ? null
                        : (value) {
                            if (value != null) {
                              setState(() {
                                _action = value;
                              });
                            }
                          },
                  ),
                ),
                const SizedBox(width: 12),
                ElevatedButton(
                  onPressed: widget.busy
                      ? null
                      : () async {
                          final pubkey = _controller.text.trim();
                          await widget.onAction(_action, pubkey);
                        },
                  child: const Text('Apply'),
                ),
              ],
            ),
            const SizedBox(height: 12),
            Align(
              alignment: Alignment.centerRight,
              child: TextButton(
                onPressed: widget.busy
                    ? null
                    : () async {
                        final pubkey = _controller.text.trim();
                        final result = await widget.onExplain(pubkey);
                        setState(() {
                          _explanation = result;
                        });
                      },
                child: const Text('Explain'),
              ),
            ),
            if (_explanation != null) ...[
              const Divider(height: 24),
              Text(
                'Explanation',
                style: Theme.of(context).textTheme.labelLarge,
              ),
              const SizedBox(height: 8),
              Text('Tier: ${_explanation!['tier']}'),
              Text('Score: ${_explanation!['score']}'),
              Text(
                'Direct endorsers: ${_explanation!['direct_endorser_count']}',
              ),
              Text(
                'Second-hop endorsers: ${_explanation!['second_hop_endorser_count']}',
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _MetricRow extends StatelessWidget {
  final String label;
  final String value;

  const _MetricRow({required this.label, required this.value});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Row(
        children: [
          SizedBox(
            width: 120,
            child: Text(
              label,
              style: const TextStyle(fontWeight: FontWeight.w600),
            ),
          ),
          Text(value),
        ],
      ),
    );
  }
}
