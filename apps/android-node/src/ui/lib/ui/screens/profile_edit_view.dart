import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:image_picker/image_picker.dart';
import '../../logic/node_service.dart';
import '../../logic/social_controller.dart';
import '../theme/veil_theme.dart';

class ProfileEditView extends StatefulWidget {
  final NodeService service;
  final SocialController controller;

  const ProfileEditView({
    super.key,
    required this.service,
    required this.controller,
  });

  @override
  State<ProfileEditView> createState() => _ProfileEditViewState();
}

class _ProfileEditViewState extends State<ProfileEditView> {
  late final TextEditingController _nameController;
  late final TextEditingController _bioController;
  late final TextEditingController _lnController;
  String? _avatarMediaRoot;
  Uint8List? _selectedImageBytes;
  bool _isSaving = false;
  final ImagePicker _picker = ImagePicker();

  @override
  void initState() {
    super.initState();
    // Pre-fill if current profile exists
    final profile = widget.service.profiles[widget.service.state.identityHex];
    _nameController = TextEditingController(
      text: profile?.data['display_name'] as String?,
    );
    _bioController = TextEditingController(
      text: profile?.data['bio'] as String?,
    );
    _lnController = TextEditingController(
      text: profile?.data['lightning_address'] as String?,
    );
    _avatarMediaRoot = profile?.data['avatar_media_root'] as String?;
  }

  Future<void> _pickImage() async {
    final image = await _picker.pickImage(
      source: ImageSource.gallery,
      maxWidth: 512,
      maxHeight: 512,
      imageQuality: 80,
    );
    if (image != null) {
      final bytes = await image.readAsBytes();
      setState(() {
        _selectedImageBytes = bytes;
      });
    }
  }

  Future<void> _handleSave() async {
    final name = _nameController.text.trim();
    final bio = _bioController.text.trim();
    final ln = _lnController.text.trim();
    if (name.isEmpty) return;

    setState(() => _isSaving = true);
    try {
      String? finalAvatarRoot = _avatarMediaRoot;
      if (_selectedImageBytes != null) {
        final root = await widget.service.uploadMedia(_selectedImageBytes!);
        if (root == null) {
          throw Exception(
            widget.service.state.lastError ?? 'Avatar upload failed',
          );
        }
        finalAvatarRoot = root;
      }

      final ok = await widget.service.publishProfile(
        displayName: name,
        bio: bio,
        lightningAddress: ln,
        avatarMediaRoot: finalAvatarRoot,
      );
      if (ok) {
        if (mounted) Navigator.pop(context);
      } else {
        throw Exception(widget.service.state.lastError ?? 'Save failed');
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('Failed to save profile: $e')));
      }
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Edit Profile'),
        actions: [
          TextButton(
            onPressed: _isSaving ? null : _handleSave,
            child: _isSaving
                ? const SizedBox(
                    width: 20,
                    height: 20,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Text(
                    'Save',
                    style: TextStyle(
                      color: VeilTheme.accent,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
          ),
        ],
      ),
      body: SafeArea(
        child: ListView(
          padding: const EdgeInsets.all(24),
          children: [
            Center(
              child: Stack(
                children: [
                  CircleAvatar(
                    radius: 50,
                    backgroundColor: VeilTheme.surface,
                    backgroundImage: _selectedImageBytes != null
                        ? MemoryImage(_selectedImageBytes!)
                        : (_avatarMediaRoot != null &&
                                widget.controller.imageCache
                                    .containsKey(_avatarMediaRoot))
                            ? MemoryImage(widget.controller
                                .imageCache[_avatarMediaRoot!]!)
                            : null,
                    child: _selectedImageBytes == null &&
                            (_avatarMediaRoot == null ||
                                !widget.controller.imageCache
                                    .containsKey(_avatarMediaRoot))
                        ? const Icon(
                            Icons.person,
                            size: 50,
                            color: VeilTheme.accent,
                          )
                        : null,
                  ),
                  Positioned(
                    right: 0,
                    bottom: 0,
                    child: CircleAvatar(
                      radius: 18,
                      backgroundColor: VeilTheme.accent,
                      child: IconButton(
                        icon: const Icon(
                          Icons.camera_alt,
                          size: 18,
                          color: Colors.black,
                        ),
                        onPressed: _pickImage,
                      ),
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 32),
            TextField(
              controller: _nameController,
              decoration: const InputDecoration(
                labelText: 'Display Name',
                labelStyle: TextStyle(color: VeilTheme.textSecondary),
                enabledBorder: UnderlineInputBorder(
                  borderSide: BorderSide(color: Colors.white10),
                ),
              ),
            ),
            const SizedBox(height: 24),
            TextField(
              controller: _bioController,
              maxLines: 3,
              decoration: const InputDecoration(
                labelText: 'Bio',
                labelStyle: TextStyle(color: VeilTheme.textSecondary),
                enabledBorder: UnderlineInputBorder(
                  borderSide: BorderSide(color: Colors.white10),
                ),
              ),
            ),
            const SizedBox(height: 24),
            TextField(
              controller: _lnController,
              decoration: const InputDecoration(
                labelText: 'Lightning Address (Optional)',
                hintText: 'user@domain.com',
                labelStyle: TextStyle(color: VeilTheme.textSecondary),
                enabledBorder: UnderlineInputBorder(
                  borderSide: BorderSide(color: Colors.white10),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
