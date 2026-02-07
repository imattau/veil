import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';

final RegExp _hashtagPattern = RegExp(
  r'(#[A-Za-z0-9_][A-Za-z0-9_-]{0,49})',
);

Widget buildHashtagText(
  BuildContext context,
  String text,
  ValueChanged<String> onTapHashtag,
) {
  final matches = _hashtagPattern.allMatches(text).toList();
  if (matches.isEmpty) {
    return Text(text);
  }
  final spans = <InlineSpan>[];
  var cursor = 0;
  for (final match in matches) {
    if (match.start > cursor) {
      spans.add(TextSpan(text: text.substring(cursor, match.start)));
    }
    final tag = match.group(0) ?? '';
    spans.add(
      TextSpan(
        text: tag,
        style: Theme.of(context).textTheme.bodyMedium?.copyWith(
              color: const Color(0xFF7DD3FC),
              fontWeight: FontWeight.w600,
            ),
        recognizer: TapGestureRecognizer()
          ..onTap = () {
            final cleaned = tag.substring(1);
            if (cleaned.isEmpty) return;
            onTapHashtag(cleaned);
          },
      ),
    );
    cursor = match.end;
  }
  if (cursor < text.length) {
    spans.add(TextSpan(text: text.substring(cursor)));
  }
  return Text.rich(
    TextSpan(style: Theme.of(context).textTheme.bodyMedium, children: spans),
  );
}
