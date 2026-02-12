import 'dart:ui';
import 'package:flutter/material.dart';
import '../../logic/node_service.dart';
import '../../logic/social_controller.dart';
import '../components/veil_post_card.dart';
import '../components/poll_widget.dart';
import '../components/live_status_banner.dart';
import '../components/empty_state.dart';
import '../components/feed_shimmer.dart';
import '../components/network_pulse.dart';
import '../theme/veil_theme.dart';
import './profile_view.dart';
import './composer_view.dart';
import './inbox_view.dart';
import './explore_view.dart';
import '../components/new_message_dialog.dart';
import '../../logic/messaging_controller.dart';

class SocialHome extends StatefulWidget {
  const SocialHome({super.key});

  @override
  State<SocialHome> createState() => _SocialHomeState();
}

class _SocialHomeState extends State<SocialHome> {
  final NodeService _service = NodeService();
  late final SocialController _controller;
  late final MessagingController _messagingController;
  int _currentIndex = 0;

  @override
  void initState() {
    super.initState();
    _controller = SocialController(_service);
    _messagingController = MessagingController(_service);
    _service.start();
  }

  @override
  void dispose() {
    _controller.dispose();
    _messagingController.dispose();
    _service.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      extendBody: true,
      extendBodyBehindAppBar: true,
      appBar: PreferredSize(
        preferredSize: const Size.fromHeight(kToolbarHeight),
        child: ClipRect(
          child: BackdropFilter(
            filter: ImageFilter.blur(sigmaX: 10, sigmaY: 10),
            child: AppBar(
              backgroundColor: VeilTheme.background.withOpacity(0.7),
              title: Row(
                children: [
                  Image.asset('assets/veil_logo.png', height: 24),
                  const SizedBox(width: 8),
                  const Text('Social'),
                ],
              ),
              actions: [
                Padding(
                  padding: const EdgeInsets.only(right: 20),
                  child: Center(child: NetworkPulse(service: _service)),
                ),
              ],
            ),
          ),
        ),
      ),
      body: IndexedStack(
        index: _currentIndex,
        children: [
          _FeedView(controller: _controller),
          const ExploreView(),
          InboxView(
            controller: _messagingController,
            socialController: _controller,
          ),
          ProfileView(service: _service, controller: _controller),
        ],
      ),
      bottomNavigationBar: ClipRect(
        child: BackdropFilter(
          filter: ImageFilter.blur(sigmaX: 10, sigmaY: 10),
          child: BottomNavigationBar(
            currentIndex: _currentIndex,
            onTap: (i) => setState(() => _currentIndex = i),
            type: BottomNavigationBarType.fixed,
            backgroundColor: VeilTheme.background.withOpacity(0.7),
            selectedItemColor: VeilTheme.accent,
            unselectedItemColor: VeilTheme.textSecondary,
            showSelectedLabels: false,
            showUnselectedLabels: false,
            items: const [
              BottomNavigationBarItem(icon: Icon(Icons.home_filled), label: 'Home'),
              BottomNavigationBarItem(icon: Icon(Icons.search), label: 'Explore'),
              BottomNavigationBarItem(icon: Icon(Icons.mail_outline), label: 'Inbox'),
              BottomNavigationBarItem(icon: Icon(Icons.person_outline), label: 'Profile'),
            ],
          ),
        ),
      ),
      floatingActionButton: _buildContextualFAB(),
    );
  }

  Widget? _buildContextualFAB() {
    if (_currentIndex == 3) return null; // Profile tab

    IconData icon = Icons.add;
    VoidCallback? onPressed;

    if (_currentIndex == 2) {
      // Inbox tab
      icon = Icons.mail;
      onPressed = () {
        showDialog(
          context: context,
          builder: (context) => NewMessageDialog(controller: _messagingController),
        );
      };
    } else {
      // Home/Explore tab
      onPressed = () {
        Navigator.push(
          context,
          MaterialPageRoute(
            builder: (context) => ComposerView(service: _service),
            fullscreenDialog: true,
          ),
        );
      };
    }

    return FloatingActionButton(
      onPressed: onPressed,
      backgroundColor: VeilTheme.accent,
      child: Icon(icon, color: Colors.black),
    );
  }
}

class _FeedView extends StatelessWidget {
  final SocialController controller;

  const _FeedView({required this.controller});

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final feed = controller.feed;
        final state = controller.nodeService.state;
        debugPrint('[FeedView] Rendering ${feed.length} posts');

        if (state.busy && feed.isEmpty) {
          return const FeedShimmer();
        }

        if (!state.busy && feed.isEmpty) {
          return RefreshIndicator(
            onRefresh: controller.nodeService.refresh,
            child: SingleChildScrollView(
              physics: const AlwaysScrollableScrollPhysics(),
              child: SizedBox(
                height: MediaQuery.of(context).size.height * 0.7,
                child: EmptyState(
                  icon: Icons.bubble_chart_outlined,
                  title: 'Welcome to the Veil',
                  message: 'Your personal social node is active and syncing. Follow people or channels to see content!',
                  onAction: () => controller.nodeService.refresh(),
                  actionLabel: 'Check for Updates',
                ),
              ),
            ),
          );
        }
        
        return RefreshIndicator(
          onRefresh: controller.nodeService.refresh,
          child: ListView.builder(
            padding: const EdgeInsets.all(16),
            itemCount: feed.length + 1,
            itemBuilder: (context, index) {
              if (index == 0) {
                return Column(
                  children: [
                    if (!state.hasBackedUp)
                      const _BackupReminder(),
                    LiveStatusBanner(controller: controller),
                  ],
                );
              }
              final event = feed[index - 1];
              if (event.isPoll) {
                return PollWidget(event: event, controller: controller);
              }
              return VeilPostCard(event: event, controller: controller);
            },
          ),
        );
      },
    );
  }
}

class _BackupReminder extends StatelessWidget {
  const _BackupReminder();

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.only(bottom: 16),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: Colors.amber.withOpacity(0.1),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: Colors.amber.withOpacity(0.3)),
      ),
      child: Row(
        children: [
          const Icon(Icons.warning_amber_rounded, color: Colors.amber),
          const SizedBox(width: 12),
          const Expanded(
            child: Text(
              'Identity not backed up! Save your secret keys to avoid losing access.',
              style: TextStyle(fontSize: 13, color: Colors.amber),
            ),
          ),
        ],
      ),
    );
  }
}
