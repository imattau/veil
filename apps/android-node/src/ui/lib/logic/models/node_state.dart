import 'package:flutter/foundation.dart';

@immutable
class NodeState {
  final bool running;
  final Set<String> activeOperations;
  final String? lastError;
  final String? identityHex;
  final String? authToken;
  final Map<String, dynamic> statusPayload;
  final Map<String, dynamic> healthPayload;
  final Map<String, dynamic> policySummary;
  final List<String> subscriptions;
  final bool hasBackedUp;
  final bool isOffline;
  final DateTime? lastUpdated;

  bool get busy => activeOperations.isNotEmpty;

  const NodeState({
    required this.running,
    required this.activeOperations,
    required this.lastError,
    required this.identityHex,
    required this.authToken,
    required this.statusPayload,
    required this.healthPayload,
    required this.policySummary,
    required this.subscriptions,
    required this.hasBackedUp,
    required this.isOffline,
    required this.lastUpdated,
  });

  factory NodeState.initial() => const NodeState(
    running: false,
    activeOperations: {},
    lastError: null,
    identityHex: null,
    authToken: null,
    statusPayload: {},
    healthPayload: {},
    policySummary: {},
    subscriptions: [],
    hasBackedUp: false,
    isOffline: false,
    lastUpdated: null,
  );

  NodeState copyWith({
    bool? running,
    Set<String>? activeOperations,
    String? lastError,
    String? identityHex,
    String? authToken,
    Map<String, dynamic>? statusPayload,
    Map<String, dynamic>? healthPayload,
    Map<String, dynamic>? policySummary,
    List<String>? subscriptions,
    bool? hasBackedUp,
    bool? isOffline,
    DateTime? lastUpdated,
  }) {
    return NodeState(
      running: running ?? this.running,
      activeOperations: activeOperations ?? this.activeOperations,
      lastError: lastError ?? this.lastError,
      identityHex: identityHex ?? this.identityHex,
      authToken: authToken ?? this.authToken,
      statusPayload: statusPayload ?? this.statusPayload,
      healthPayload: healthPayload ?? this.healthPayload,
      policySummary: policySummary ?? this.policySummary,
      subscriptions: subscriptions ?? this.subscriptions,
      hasBackedUp: hasBackedUp ?? this.hasBackedUp,
      isOffline: isOffline ?? this.isOffline,
      lastUpdated: lastUpdated ?? this.lastUpdated,
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is NodeState &&
          runtimeType == other.runtimeType &&
          running == other.running &&
          setEquals(activeOperations, other.activeOperations) &&
          lastError == other.lastError &&
          identityHex == other.identityHex &&
          authToken == other.authToken &&
          mapEquals(statusPayload, other.statusPayload) &&
          mapEquals(healthPayload, other.healthPayload) &&
          mapEquals(policySummary, other.policySummary) &&
          listEquals(subscriptions, other.subscriptions) &&
          hasBackedUp == other.hasBackedUp &&
          isOffline == other.isOffline &&
          lastUpdated == other.lastUpdated;

  @override
  int get hashCode =>
      running.hashCode ^
      activeOperations.hashCode ^
      lastError.hashCode ^
      identityHex.hashCode ^
      authToken.hashCode ^
      statusPayload.hashCode ^
      healthPayload.hashCode ^
      policySummary.hashCode ^
      subscriptions.hashCode ^
      hasBackedUp.hashCode ^
      isOffline.hashCode ^
      lastUpdated.hashCode;
}
