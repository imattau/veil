import 'package:flutter/material.dart';
import 'package:google_fonts/google_fonts.dart';

class VeilTheme {
  static const Color background = Color(0xFF0F1419);
  static const Color amoledBackground = Color(0xFF050607);
  static const Color surface = Color(0xFF19232B);
  static const Color lightSurface = Color(0xFFF4F7FA);
  static const Color accent = Color(0xFF00F5D4); // Emerald Ghost
  static const Color accentSubtle = Color(0x3300F5D4); // 20% opacity
  static const Color textPrimary = Colors.white;
  static const Color lightTextPrimary = Color(0xFF0D1A21);
  static const Color textSecondary = Color(0xFF8B98A5);
  static const Color lightTextSecondary = Color(0xFF5A6B78);
  static const Color dividerColor = Colors.white10;
  static const Color surfaceHighlight = Color(0x0DFFFFFF); // 5% white
  static const double radiusS = 12;
  static const double radiusM = 16;
  static const double radiusL = 24;
  static const double minTapTarget = 48;
  static const EdgeInsets buttonPadding = EdgeInsets.symmetric(
    horizontal: 16,
    vertical: 12,
  );

  static DialogThemeData _dialogTheme(Color borderColor) {
    return DialogThemeData(
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(radiusM),
        side: BorderSide(color: borderColor),
      ),
    );
  }

  static BottomSheetThemeData _bottomSheetTheme(Color backgroundColor) {
    return BottomSheetThemeData(
      backgroundColor: backgroundColor,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(radiusL)),
      ),
      clipBehavior: Clip.antiAlias,
    );
  }

  static TextButtonThemeData _textButtonTheme(Color foreground) {
    return TextButtonThemeData(
      style: TextButton.styleFrom(
        foregroundColor: foreground,
        minimumSize: const Size(minTapTarget, minTapTarget),
        padding: buttonPadding,
      ),
    );
  }

  static ElevatedButtonThemeData _elevatedButtonTheme(
    Color backgroundColor,
    Color foregroundColor,
  ) {
    return ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: backgroundColor,
        foregroundColor: foregroundColor,
        minimumSize: const Size(minTapTarget, minTapTarget),
        padding: buttonPadding,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(radiusL),
        ),
      ),
    );
  }

  static FilledButtonThemeData _filledButtonTheme(
    Color backgroundColor,
    Color foregroundColor,
  ) {
    return FilledButtonThemeData(
      style: FilledButton.styleFrom(
        backgroundColor: backgroundColor,
        foregroundColor: foregroundColor,
        minimumSize: const Size(minTapTarget, minTapTarget),
        padding: buttonPadding,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(radiusM),
        ),
      ),
    );
  }

  static IconButtonThemeData _iconButtonTheme(Color foreground) {
    return IconButtonThemeData(
      style: IconButton.styleFrom(
        foregroundColor: foreground,
        minimumSize: const Size(minTapTarget, minTapTarget),
      ),
    );
  }

  static ThemeData get dark {
    final baseTextTheme = GoogleFonts.interTextTheme(
      ThemeData.dark().textTheme,
    );
    final displayFont = GoogleFonts.spaceGrotesk();

    return ThemeData(
      brightness: Brightness.dark,
      scaffoldBackgroundColor: background,
      primaryColor: accent,
      useMaterial3: true,
      materialTapTargetSize: MaterialTapTargetSize.padded,
      focusColor: accentSubtle,
      colorScheme: const ColorScheme.dark(
        primary: accent,
        surface: surface,
        onSurface: textPrimary,
      ),
      dialogTheme: _dialogTheme(Colors.white.withValues(alpha: 0.08)),
      bottomSheetTheme: _bottomSheetTheme(surface),
      textButtonTheme: _textButtonTheme(textPrimary),
      elevatedButtonTheme: _elevatedButtonTheme(accent, Colors.black),
      filledButtonTheme: _filledButtonTheme(accentSubtle, accent),
      iconButtonTheme: _iconButtonTheme(textPrimary),
      cardTheme: CardThemeData(
        color: surface,
        elevation: 0,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(radiusM),
          side: BorderSide(color: Colors.white.withValues(alpha: 0.05)),
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

  static ThemeData get amoled {
    final base = dark;
    return base.copyWith(
      scaffoldBackgroundColor: amoledBackground,
      colorScheme: base.colorScheme.copyWith(surface: amoledBackground),
      appBarTheme: base.appBarTheme.copyWith(backgroundColor: amoledBackground),
      cardTheme: base.cardTheme.copyWith(
        color: const Color(0xFF0A0D10),
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(radiusM),
          side: BorderSide(color: Colors.white.withValues(alpha: 0.06)),
        ),
      ),
    );
  }

  static ThemeData get light {
    final baseTextTheme = GoogleFonts.interTextTheme(
      ThemeData.light().textTheme,
    );
    final displayFont = GoogleFonts.spaceGrotesk();

    return ThemeData(
      brightness: Brightness.light,
      scaffoldBackgroundColor: const Color(0xFFEFF3F6),
      primaryColor: accent,
      useMaterial3: true,
      materialTapTargetSize: MaterialTapTargetSize.padded,
      focusColor: accentSubtle,
      colorScheme: const ColorScheme.light(
        primary: accent,
        surface: lightSurface,
        onSurface: lightTextPrimary,
      ),
      dialogTheme: _dialogTheme(Colors.black.withValues(alpha: 0.08)),
      bottomSheetTheme: _bottomSheetTheme(lightSurface),
      textButtonTheme: _textButtonTheme(lightTextPrimary),
      elevatedButtonTheme: _elevatedButtonTheme(accent, Colors.black),
      filledButtonTheme: _filledButtonTheme(accentSubtle, accent),
      iconButtonTheme: _iconButtonTheme(lightTextPrimary),
      cardTheme: CardThemeData(
        color: lightSurface,
        elevation: 0,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(radiusM),
          side: BorderSide(color: Colors.black.withValues(alpha: 0.06)),
        ),
      ),
      appBarTheme: AppBarTheme(
        backgroundColor: const Color(0xFFEFF3F6),
        elevation: 0,
        centerTitle: false,
        titleTextStyle: displayFont.copyWith(
          color: lightTextPrimary,
          fontSize: 20,
          fontWeight: FontWeight.bold,
        ),
      ),
      textTheme: baseTextTheme.copyWith(
        displayLarge: displayFont.copyWith(
          color: lightTextPrimary,
          fontWeight: FontWeight.bold,
        ),
        displayMedium: displayFont.copyWith(
          color: lightTextPrimary,
          fontWeight: FontWeight.bold,
        ),
        headlineMedium: displayFont.copyWith(
          color: lightTextPrimary,
          fontWeight: FontWeight.bold,
        ),
        titleLarge: displayFont.copyWith(
          color: lightTextPrimary,
          fontWeight: FontWeight.bold,
        ),
        titleMedium: baseTextTheme.titleMedium?.copyWith(
          color: lightTextPrimary,
          fontWeight: FontWeight.bold,
          fontSize: 16,
        ),
        bodyMedium: baseTextTheme.bodyMedium?.copyWith(
          color: lightTextPrimary,
          fontSize: 15,
          height: 1.4,
        ),
        labelSmall: baseTextTheme.labelSmall?.copyWith(
          color: lightTextSecondary,
          fontSize: 12,
        ),
      ),
    );
  }
}
