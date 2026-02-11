import 'package:flutter/material.dart';

class VeilTheme {
  static const Color background = Color(0xFF0F1419);
  static const Color surface = Color(0xFF19232B);
  static const Color accent = Color(0xFF00F5D4); // Emerald Ghost
  static const Color textPrimary = Colors.white;
  static const Color textSecondary = Color(0xFF8B98A5);

  static ThemeData get dark {
    return ThemeData(
      brightness: Brightness.dark,
      scaffoldBackgroundColor: background,
      primaryColor: accent,
      colorScheme: const ColorScheme.dark(
        primary: accent,
        surface: surface,
        onSurface: textPrimary,
        background: background,
      ),
      cardTheme: CardThemeData(
        color: surface,
        elevation: 0,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(16),
          side: BorderSide(color: Colors.white.withOpacity(0.05)),
        ),
      ),
      appBarTheme: const AppBarTheme(
        backgroundColor: background,
        elevation: 0,
        centerTitle: false,
        titleTextStyle: TextStyle(
          color: textPrimary,
          fontSize: 20,
          fontWeight: FontWeight.bold,
        ),
      ),
      textTheme: const TextTheme(
        titleMedium: TextStyle(
          color: textPrimary,
          fontWeight: FontWeight.bold,
          fontSize: 16,
        ),
        bodyMedium: TextStyle(
          color: textPrimary,
          fontSize: 15,
          height: 1.4,
        ),
        labelSmall: TextStyle(
          color: textSecondary,
          fontSize: 12,
        ),
      ),
    );
  }
}
