part of '../social_controller.dart';

class _OptimisticReaction {
  final String objectRoot;
  final String action;
  final String? authorPubkey;
  final int createdAtMs;

  const _OptimisticReaction({
    required this.objectRoot,
    required this.action,
    required this.authorPubkey,
    required this.createdAtMs,
  });
}

class _OptimisticRepost {
  final String objectRoot;
  final String? authorPubkey;
  final int createdAtMs;

  const _OptimisticRepost({
    required this.objectRoot,
    required this.authorPubkey,
    required this.createdAtMs,
  });
}
