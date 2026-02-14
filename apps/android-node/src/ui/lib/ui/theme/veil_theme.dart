import 'package:flutter/material.dart';
import 'package:google_fonts/google_fonts.dart';

class VeilTheme {
  static const Color background = Color(0xFF0F1419);
  static const Color surface = Color(0xFF19232B);
  static const Color accent = Color(0xFF00F5D4); // Emerald Ghost
  static const Color accentSubtle = Color(0x3300F5D4); // 20% opacity
  static const Color textPrimary = Colors.white;
  static const Color textSecondary = Color(0xFF8B98A5);
  static const Color dividerColor = Colors.white10;
  static const Color surfaceHighlight = Color(0x0DFFFFFF); // 5% white

  static ThemeData get dark {
    final baseTextTheme = GoogleFonts.interTextTheme(ThemeData.dark().textTheme);
    final displayFont = GoogleFonts.spaceGrotesk();

    return ThemeData(
      brightness: Brightness.dark,
      scaffoldBackgroundColor: background,
      primaryColor: accent,
      useMaterial3: true,
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
      appBarTheme: AppBarTheme(
        backgroundColor: background,
        elevation: 0,
        centerTitle: false,
        titleTextStyle: displayFont.copyWith(
          color: textPrimary,
          fontSize: 20,
          fontWeight: FontWeight.bold,
        ),
      ),
      textTheme: baseTextTheme.copyWith(
        displayLarge: displayFont.copyWith(
          color: textPrimary,
          fontWeight: FontWeight.bold,
        ),
        displayMedium: displayFont.copyWith(
          color: textPrimary,
          fontWeight: FontWeight.bold,
        ),
        headlineMedium: displayFont.copyWith(
          color: textPrimary,
          fontWeight: FontWeight.bold,
        ),
        titleLarge: displayFont.copyWith(
          color: textPrimary,
          fontWeight: FontWeight.bold,
        ),
        titleMedium: baseTextTheme.titleMedium?.copyWith(
          color: textPrimary,
          fontWeight: FontWeight.bold,
          fontSize: 16,
        ),
        bodyMedium: baseTextTheme.bodyMedium?.copyWith(
          color: textPrimary,
          fontSize: 15,
          height: 1.4,
        ),
        labelSmall: baseTextTheme.labelSmall?.copyWith(
          color: textSecondary,
          fontSize: 12,
        ),
      ),
    );
  }
}
