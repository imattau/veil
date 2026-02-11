import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import '../theme/veil_theme.dart';

class RichTextView extends StatelessWidget {
  final String text;
  final TextStyle? style;

  const RichTextView({super.key, required this.text, this.style});

  @override
  Widget build(BuildContext context) {
    if (!text.contains('#') && !text.contains('@') && !text.contains('http')) {
      return Text(text, style: style ?? Theme.of(context).textTheme.bodyMedium);
    }

    final List<InlineSpan> spans = [];
    final words = text.split(RegExp(r'(\s+)'));

    for (final word in words) {
      if (word.startsWith('#') && word.length > 1) {
        spans.add(TextSpan(
          text: word,
          style: const TextStyle(color: VeilTheme.accent, fontWeight: FontWeight.bold),
          recognizer: TapGestureRecognizer()
            ..onTap = () {
              debugPrint('Tapped hashtag: $word');
              // TODO: Navigate to channel/search
            },
        ));
      } else if (word.startsWith('@') && word.length > 1) {
        spans.add(TextSpan(
          text: word,
          style: const TextStyle(color: VeilTheme.accent),
          recognizer: TapGestureRecognizer()
            ..onTap = () {
              debugPrint('Tapped mention: $word');
              // TODO: Navigate to profile
            },
        ));
      } else if (Uri.tryParse(word)?.hasAbsolutePath ?? false) {
        spans.add(TextSpan(
          text: word,
          style: const TextStyle(
            color: VeilTheme.accent,
            decoration: TextDecoration.underline,
          ),
          recognizer: TapGestureRecognizer()
            ..onTap = () {
              debugPrint('Tapped link: $word');
              // TODO: Launch URL
            },
        ));
      } else {
        spans.add(TextSpan(text: word, style: style));
      }
    }

    return Text.rich(
      TextSpan(
        children: spans,
        style: style ?? Theme.of(context).textTheme.bodyMedium,
      ),
    );
  }
}
