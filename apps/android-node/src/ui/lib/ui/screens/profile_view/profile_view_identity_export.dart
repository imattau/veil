part of '../profile_view.dart';

class _ExportIdentityContent extends StatefulWidget {
  final String publicKeyHex;
  final String secretKeyHex;

  const _ExportIdentityContent({
    required this.publicKeyHex,
    required this.secretKeyHex,
  });

  @override
  State<_ExportIdentityContent> createState() => _ExportIdentityContentState();
}

class _ExportIdentityContentState extends State<_ExportIdentityContent> {
  bool _revealed = false;

  @override
  Widget build(BuildContext context) {
    final obscured = '\u2022' * 16;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const Text('Public Key:'),
        SelectableText(
          widget.publicKeyHex,
          style: const TextStyle(fontFamily: 'monospace', fontSize: 12),
        ),
        const SizedBox(height: 12),
        const Text(
          'Secret Key (KEEP PRIVATE!):',
          style: TextStyle(color: Colors.red, fontWeight: FontWeight.bold),
        ),
        Row(
          children: [
            Expanded(
              child: _revealed
                  ? SelectableText(
                      widget.secretKeyHex,
                      style: const TextStyle(
                        fontFamily: 'monospace',
                        fontSize: 12,
                      ),
                    )
                  : Text(
                      obscured,
                      style: const TextStyle(
                        fontFamily: 'monospace',
                        fontSize: 12,
                      ),
                    ),
            ),
            IconButton(
              icon: Icon(
                _revealed ? Icons.visibility_off : Icons.visibility,
                size: 20,
              ),
              onPressed: () => setState(() => _revealed = !_revealed),
              tooltip: _revealed ? 'Hide' : 'Reveal',
            ),
          ],
        ),
      ],
    );
  }
}
