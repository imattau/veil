import 'dart:ui';
import 'package:flutter/material.dart';
import '../../logic/social_controller.dart';
import '../../logic/list_controller.dart';
import '../components/veil_post_card.dart';
import '../components/empty_state.dart';
import '../theme/veil_theme.dart';

class BookmarksView extends StatelessWidget {
  final SocialController controller;
  final ListController listController;

  const BookmarksView({
    super.key,
    required this.controller,
    required this.listController,
  });

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      extendBodyBehindAppBar: true,
      appBar: PreferredSize(
        preferredSize: const Size.fromHeight(kToolbarHeight),
        child: ClipRect(
          child: BackdropFilter(
            filter: ImageFilter.blur(sigmaX: 10, sigmaY: 10),
            child: AppBar(
              backgroundColor: VeilTheme.background.withOpacity(0.7),
              title: const Text('Bookmarks'),
            ),
          ),
        ),
      ),
      body: ListenableBuilder(
        listenable: listController,
        builder: (context, _) {
          final bookmarkRoots = listController.bookmarkRoots;
          final bookmarkedEvents = controller.nodeService.feedEvents
              .where((e) => bookmarkRoots.contains(e.objectRoot))
              .toList();

          if (bookmarkedEvents.isEmpty) {
            return EmptyState(
              icon: Icons.bookmark_border,
              title: 'No bookmarks yet',
              message: 'Save interesting posts to read them later.',
              onAction: () => Navigator.pop(context),
              actionLabel: 'Go Back',
            );
          }

          return ListView.builder(
            padding: const EdgeInsets.fromLTRB(16, 100, 16, 16),
            itemCount: bookmarkedEvents.length,
            itemBuilder: (context, index) {
              return VeilPostCard(
                event: bookmarkedEvents[index],
                controller: controller,
                listController: listController,
              );
            },
          );
        },
      ),
    );
  }
}
