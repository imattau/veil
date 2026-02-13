import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';
import '../../logic/social_controller.dart';
import '../theme/veil_theme.dart';
import '../screens/profile_view.dart';

class RichTextView extends StatelessWidget {
  final String text;
  final TextStyle? style;
  final SocialController controller;

  const RichTextView({
    super.key,
    required this.text,
    required this.controller,
    this.style,
  });

  @override
  Widget build(BuildContext context) {
    final effectiveStyle = style ?? Theme.of(context).textTheme.bodyMedium;
    if (!text.contains('#') && !text.contains('@') && !text.contains('http')) {
      return Text(text, style: effectiveStyle, softWrap: true);
    }

    final tokenPattern = RegExp(
      r'(https?:\/\/[^\s]+|#[A-Za-z0-9_]+|@[A-Za-z0-9_]+)',
    );
    final List<InlineSpan> spans = [];
    var cursor = 0;
    for (final match in tokenPattern.allMatches(text)) {
      if (match.start > cursor) {
        spans.add(
          TextSpan(
            text: text.substring(cursor, match.start),
            style: effectiveStyle,
          ),
        );
      }
      final token = match.group(0)!;
      if (token.startsWith('#')) {
        spans.add(
          TextSpan(
            text: token,
            style: const TextStyle(
              color: VeilTheme.accent,
              fontWeight: FontWeight.bold,
            ),
            recognizer: TapGestureRecognizer()
              ..onTap = () async {
                final channel = token.substring(1);
                await controller.nodeService.subscribeTag(channel);
                if (context.mounted) {
                  ScaffoldMessenger.of(context).showSnackBar(
                    SnackBar(content: Text('Joined #$channel')),
                  );
                }
              },
          ),
        );
      } else if (token.startsWith('@')) {
        spans.add(
          TextSpan(
            text: token,
            style: const TextStyle(color: VeilTheme.accent),
            recognizer: TapGestureRecognizer()
              ..onTap = () {
                final pubkey = token.substring(1);
                if (pubkey.length == 64) {
                  Navigator.push(
                    context,
                    MaterialPageRoute(
                      builder: (context) => ProfileView(
                        service: controller.nodeService,
                        controller: controller,
                        targetPubkey: pubkey,
                      ),
                    ),
                  );
                }
              },
          ),
        );
      } else {
        spans.add(
          TextSpan(
            text: token,
            style: const TextStyle(
              color: VeilTheme.accent,
              decoration: TextDecoration.underline,
            ),
            recognizer: TapGestureRecognizer()
              ..onTap = () async {
                final uri = Uri.tryParse(token);
                if (uri != null) {
                  await launchUrl(uri, mode: LaunchMode.externalApplication);
                }
              },
          ),
        );
      }
      cursor = match.end;
    }
    if (cursor < text.length) {
      spans.add(TextSpan(text: text.substring(cursor), style: effectiveStyle));
    }

    return Text.rich(
      TextSpan(children: spans, style: effectiveStyle),
      softWrap: true,
    );
  }
}
