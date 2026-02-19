import 'package:flutter_test/flutter_test.dart';
import 'package:veil_social/logic/retry_backoff.dart';

void main() {
  group('RetryBackoff.exponential', () {
    test('scales exponentially from base and caps at max', () {
      expect(
        RetryBackoff.exponential(
          attempts: 1,
          base: const Duration(seconds: 1),
          max: const Duration(seconds: 60),
        ),
        const Duration(seconds: 1),
      );
      expect(
        RetryBackoff.exponential(
          attempts: 2,
          base: const Duration(seconds: 1),
          max: const Duration(seconds: 60),
        ),
        const Duration(seconds: 2),
      );
      expect(
        RetryBackoff.exponential(
          attempts: 8,
          base: const Duration(seconds: 5),
          max: const Duration(seconds: 600),
        ),
        const Duration(seconds: 600),
      );
    });

    test('normalizes invalid attempts to first attempt', () {
      expect(
        RetryBackoff.exponential(
          attempts: 0,
          base: const Duration(seconds: 5),
          max: const Duration(seconds: 600),
        ),
        const Duration(seconds: 5),
      );
    });
  });

  group('RetryBackoff.linear', () {
    test('scales linearly with attempts', () {
      expect(
        RetryBackoff.linear(
          attempts: 1,
          step: const Duration(milliseconds: 500),
        ),
        const Duration(milliseconds: 500),
      );
      expect(
        RetryBackoff.linear(
          attempts: 5,
          step: const Duration(milliseconds: 500),
        ),
        const Duration(milliseconds: 2500),
      );
    });

    test('normalizes invalid attempts to first attempt', () {
      expect(
        RetryBackoff.linear(
          attempts: -3,
          step: const Duration(milliseconds: 500),
        ),
        const Duration(milliseconds: 500),
      );
    });
  });
}
