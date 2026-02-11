import 'package:flutter/foundation.dart';

@immutable
class NodeEvent {
  final int seq;
  final String event;
  final Map<String, dynamic> data;

  const NodeEvent({
    required this.seq,
    required this.event,
    required this.data,
  });

  factory NodeEvent.fromJson(Map<String, dynamic> json) {
    return NodeEvent(
      seq: (json['seq'] as num?)?.toInt() ?? 0,
      event: json['event'] as String? ?? 'unknown',
      data: (json['data'] as Map<String, dynamic>?) ?? const {},
    );
  }
}
