import 'dart:typed_data';

import 'package:flutter/material.dart';

void openImageViewer(BuildContext context, Uint8List bytes, String title) {
  showDialog(
    context: context,
    builder: (context) => Dialog(
      backgroundColor: Colors.black,
      insetPadding: const EdgeInsets.all(12),
      child: Stack(
        children: [
          InteractiveViewer(
            child: Image.memory(bytes, fit: BoxFit.contain),
          ),
          Positioned(
            top: 8,
            right: 8,
            child: IconButton(
              icon: const Icon(Icons.close, color: Colors.white),
              onPressed: () => Navigator.of(context).pop(),
            ),
          ),
          Positioned(
            left: 12,
            bottom: 12,
            child: Text(
              title,
              style: const TextStyle(color: Colors.white70),
            ),
          ),
        ],
      ),
    ),
  );
}
