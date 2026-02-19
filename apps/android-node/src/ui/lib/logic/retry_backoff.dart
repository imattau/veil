class RetryBackoff {
  const RetryBackoff._();

  static Duration exponential({
    required int attempts,
    required Duration base,
    required Duration max,
  }) {
    final normalizedAttempts = attempts < 1 ? 1 : attempts;
    final exponent = normalizedAttempts - 1;
    final multiplier = 1 << exponent.clamp(0, 30);
    final rawMicros = base.inMicroseconds * multiplier;
    final minMicros = base.inMicroseconds;
    final maxMicros = max.inMicroseconds < minMicros
        ? minMicros
        : max.inMicroseconds;
    final clamped = rawMicros.clamp(minMicros, maxMicros) as int;
    return Duration(microseconds: clamped);
  }

  static Duration linear({
    required int attempts,
    required Duration step,
    Duration? min,
    Duration? max,
  }) {
    final normalizedAttempts = attempts < 1 ? 1 : attempts;
    var micros = step.inMicroseconds * normalizedAttempts;
    if (min != null && micros < min.inMicroseconds) {
      micros = min.inMicroseconds;
    }
    if (max != null && micros > max.inMicroseconds) {
      micros = max.inMicroseconds;
    }
    return Duration(microseconds: micros);
  }
}
