import 'dart:ui';

import 'package:flutter/material.dart';

import '../app_controller.dart';
class _ProfileEditor extends StatefulWidget {
  final VeilAppController controller;

  const _ProfileEditor({required this.controller});

  @override
  State<_ProfileEditor> createState() => _ProfileEditorState();
}

class ProfileSheet extends StatelessWidget {
  final VeilAppController controller;
  final VoidCallback onEdit;

  const ProfileSheet({super.key, required this.controller, required this.onEdit});

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: controller,
      builder: (context, _) {
        return SafeArea(
          child: ClipRRect(
            borderRadius: const BorderRadius.vertical(top: Radius.circular(24)),
            child: BackdropFilter(
              filter: ImageFilter.blur(sigmaX: 16, sigmaY: 16),
              child: Container(
                color: Theme.of(context).colorScheme.surface.withOpacity(0.9),
                child: DraggableScrollableSheet(
                  initialChildSize: 0.7,
                  minChildSize: 0.45,
                  maxChildSize: 0.95,
                  expand: false,
                  builder: (context, scrollController) {
                    return ListView(
                      controller: scrollController,
                      padding: const EdgeInsets.all(16),
                      children: [
                        Row(
                          children: [
                            CircleAvatar(
                              radius: 30,
                              backgroundColor: const Color(0xFF1E293B),
                              backgroundImage:
                                  controller.profileAvatar?.isImage == true
                                      ? MemoryImage(
                                          controller.profileAvatar!.bytes,
                                        )
                                      : null,
                              child: controller.profileAvatar == null
                                  ? const Icon(Icons.person, size: 28)
                                  : null,
                            ),
                            const SizedBox(width: 12),
                            Expanded(
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Text(
                                    controller.displayName.isEmpty
                                        ? 'Operator'
                                        : controller.displayName,
                                    style:
                                        Theme.of(context).textTheme.titleLarge,
                                  ),
                                  const SizedBox(height: 4),
                                  Text(
                                    controller.namespaceChoice,
                                    style: Theme.of(context)
                                        .textTheme
                                        .bodyMedium
                                        ?.copyWith(color: Colors.white70),
                                  ),
                                ],
                              ),
                            ),
                            IconButton(
                              tooltip: 'Edit profile',
                              icon: const Icon(Icons.edit),
                              onPressed: onEdit,
                            ),
                          ],
                        ),
                        const SizedBox(height: 16),
                        if (controller.profileBio.isNotEmpty)
                          Text(
                            controller.profileBio,
                            style: Theme.of(context).textTheme.bodyMedium,
                          ),
                        if (controller.profileBio.isNotEmpty)
                          const SizedBox(height: 12),
                        if (controller.profileWebsite.isNotEmpty)
                          Row(
                            children: [
                              const Icon(
                                Icons.link,
                                size: 18,
                                color: Colors.white70,
                              ),
                              const SizedBox(width: 6),
                              Flexible(
                                child: Text(
                                  controller.profileWebsite,
                                  style: Theme.of(context)
                                      .textTheme
                                      .bodySmall
                                      ?.copyWith(color: Colors.white70),
                                ),
                              ),
                            ],
                          ),
                        if (controller.profileWebsite.isNotEmpty)
                          const SizedBox(height: 8),
                        if (controller.profileLocation.isNotEmpty)
                          Row(
                            children: [
                              const Icon(
                                Icons.place,
                                size: 18,
                                color: Colors.white70,
                              ),
                              const SizedBox(width: 6),
                              Text(
                                controller.profileLocation,
                                style: Theme.of(context)
                                    .textTheme
                                    .bodySmall
                                    ?.copyWith(color: Colors.white70),
                              ),
                            ],
                          ),
                        if (controller.profileLastPublished != null)
                          const SizedBox(height: 12),
                        if (controller.profilePublishing)
                          Row(
                            children: [
                              const SizedBox(
                                width: 14,
                                height: 14,
                                child: CircularProgressIndicator(strokeWidth: 2),
                              ),
                              const SizedBox(width: 8),
                              Text(
                                'Publishing profile…',
                                style: Theme.of(context)
                                    .textTheme
                                    .bodySmall
                                    ?.copyWith(color: Colors.white54),
                              ),
                            ],
                          ),
                        if (!controller.profilePublishing &&
                            controller.profileLastPublished != null)
                          Text(
                            'Last published ${controller.profileLastPublished}',
                            style: Theme.of(context)
                                .textTheme
                                .bodySmall
                                ?.copyWith(color: Colors.white54),
                          ),
                      ],
                    );
                  },
                ),
              ),
            ),
          ),
        );
      },
    );
  }
}

class ProfileEditSheet extends StatelessWidget {
  final VeilAppController controller;

  const ProfileEditSheet({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: ClipRRect(
        borderRadius: const BorderRadius.vertical(top: Radius.circular(24)),
        child: BackdropFilter(
          filter: ImageFilter.blur(sigmaX: 16, sigmaY: 16),
          child: Container(
            color: Theme.of(context).colorScheme.surface.withOpacity(0.9),
            child: DraggableScrollableSheet(
              initialChildSize: 0.8,
              minChildSize: 0.5,
              maxChildSize: 0.95,
              expand: false,
              builder: (context, scrollController) {
                return ListView(
                  controller: scrollController,
                  padding: const EdgeInsets.all(16),
                  children: [
                    Text(
                      'EDIT PROFILE',
                      style: Theme.of(context).textTheme.labelLarge?.copyWith(
                        letterSpacing: 2,
                        color: Colors.white70,
                      ),
                    ),
                    const SizedBox(height: 12),
                    _ProfileEditor(controller: controller),
                  ],
                );
              },
            ),
          ),
        ),
      ),
    );
  }
}

class _ProfileEditorState extends State<_ProfileEditor> {
  late final TextEditingController _bioController;
  late final TextEditingController _websiteController;
  late final TextEditingController _locationController;

  @override
  void initState() {
    super.initState();
    _bioController = TextEditingController(text: widget.controller.profileBio);
    _websiteController =
        TextEditingController(text: widget.controller.profileWebsite);
    _locationController =
        TextEditingController(text: widget.controller.profileLocation);
  }

  @override
  void dispose() {
    _bioController.dispose();
    _websiteController.dispose();
    _locationController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: widget.controller,
      builder: (context, _) {
        final controller = widget.controller;
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                CircleAvatar(
                  radius: 28,
                  backgroundColor: const Color(0xFF1E293B),
                  backgroundImage: controller.profileAvatar?.isImage == true
                      ? MemoryImage(controller.profileAvatar!.bytes)
                      : null,
                  child: controller.profileAvatar == null
                      ? const Icon(Icons.person, size: 28)
                      : null,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        controller.displayName.isEmpty
                            ? 'Unnamed'
                            : controller.displayName,
                        style: Theme.of(context).textTheme.titleMedium,
                      ),
                      const SizedBox(height: 4),
                      Wrap(
                        spacing: 8,
                        children: [
                          OutlinedButton.icon(
                            onPressed: controller.pickProfileAvatar,
                            icon: const Icon(Icons.image_outlined),
                            label: const Text('Avatar'),
                          ),
                          if (controller.profileAvatar != null)
                            TextButton(
                              onPressed: controller.clearProfileAvatar,
                              child: const Text('Remove'),
                            ),
                        ],
                      ),
                    ],
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _bioController,
              maxLines: 3,
              decoration: const InputDecoration(
                labelText: 'Bio',
                hintText: 'Tell people who you are',
              ),
              onChanged: (value) => controller.updateProfileDetails(bio: value),
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _websiteController,
              decoration: const InputDecoration(
                labelText: 'Website',
                hintText: 'https://',
              ),
              onChanged: (value) =>
                  controller.updateProfileDetails(website: value),
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _locationController,
              decoration: const InputDecoration(
                labelText: 'Location',
              ),
              onChanged: (value) =>
                  controller.updateProfileDetails(location: value),
            ),
            const SizedBox(height: 12),
            ElevatedButton.icon(
              onPressed: controller.profilePublishing
                  ? null
                  : () {
                      controller.publishProfile();
                      ScaffoldMessenger.of(context).showSnackBar(
                        const SnackBar(
                          content: Text('Profile update queued'),
                          behavior: SnackBarBehavior.floating,
                        ),
                      );
                      Navigator.of(context).pop();
                    },
              icon: controller.profilePublishing
                  ? const SizedBox(
                      width: 18,
                      height: 18,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.publish),
              label: Text(
                controller.profilePublishing ? 'Publishing…' : 'Publish profile',
              ),
            ),
          ],
        );
      },
    );
  }
}
