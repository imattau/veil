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
import '../components/network_status_drawer.dart';
import '../theme/veil_theme.dart';
import './profile_view.dart';
import './composer_view.dart';
import './inbox_view.dart';
import './explore_view.dart';
import '../components/new_message_dialog.dart';
import '../../logic/messaging_controller.dart';
import '../../logic/list_controller.dart';
import '../../logic/preferences_controller.dart';

class SocialHome extends StatefulWidget {
  const SocialHome({super.key});

  @override
  State<SocialHome> createState() => _SocialHomeState();
}

class _SocialHomeState extends State<SocialHome> {
  final NodeService _service = NodeService();
  late final SocialController _controller;
  late final MessagingController _messagingController;
  late final ListController _listController;
  late final PreferencesController _preferencesController;
  int _currentIndex = 0;
  bool _hideBackupReminder = false;

  @override
  void initState() {
    super.initState();
    _controller = SocialController(_service);
    _messagingController = MessagingController(_service);
    _listController = ListController(_service);
    _preferencesController = PreferencesController(_service);
    _service.start();
  }

  @override
  void dispose() {
    _controller.dispose();
    _messagingController.dispose();
    _listController.dispose();
    _preferencesController.dispose();
    _service.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: _service,
      builder: (context, _) {
        final error = _service.state.lastError;
        if (error != null && error.isNotEmpty) {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (mounted) {
              ScaffoldMessenger.of(context).showSnackBar(
                SnackBar(content: Text(error)),
              );
              _service.clearError();
            }
          });
        }
        return Scaffold(
          extendBody: true,
      extendBodyBehindAppBar: true,
      endDrawer: NetworkStatusDrawer(service: _service),
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
                Builder(
                  builder: (context) => Padding(
                    padding: const EdgeInsets.only(right: 10),
                    child: IconButton(
                      tooltip: 'Network status',
                      onPressed: () => Scaffold.of(context).openEndDrawer(),
                      icon: NetworkPulse(service: _service),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
      body: IndexedStack(
        index: _currentIndex,
        children: [
          _FeedView(
            controller: _controller,
            listController: _listController,
            showBackupReminder: !_hideBackupReminder,
            onDismissReminder: () => setState(() => _hideBackupReminder = true),
            onBackup: () => setState(() => _currentIndex = 3), // Profile tab
          ),
          ExploreView(service: _service),
          InboxView(
            controller: _messagingController,
            socialController: _controller,
          ),
          ProfileView(
            service: _service,
            controller: _controller,
            listController: _listController,
            preferencesController: _preferencesController,
          ),
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
              BottomNavigationBarItem(
                icon: Icon(Icons.home_filled),
                label: 'Home',
              ),
              BottomNavigationBarItem(
                icon: Icon(Icons.search),
                label: 'Explore',
              ),
              BottomNavigationBarItem(
                icon: Icon(Icons.mail_outline),
                label: 'Inbox',
              ),
              BottomNavigationBarItem(
                icon: Icon(Icons.person_outline),
                label: 'Profile',
              ),
            ],
          ),
        ),
      ),
      floatingActionButton: _buildContextualFAB(),
    );
      },
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
          builder: (context) => NewMessageDialog(
            controller: _messagingController,
            socialController: _controller,
          ),
        );
      };
    } else if (_currentIndex == 1) {
      // Explore tab
      icon = Icons.tag;
      onPressed = _showAddChannelDialog;
    } else {
      // Home tab
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

  Future<void> _showAddChannelDialog() async {
    final controller = TextEditingController();
    final result = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Add Channel'),
        content: TextField(
          controller: controller,
          autofocus: true,
          decoration: const InputDecoration(
            labelText: 'Channel name',
            hintText: 'general',
            prefixText: '#',
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.pop(context, controller.text.trim()),
            child: const Text('Add'),
          ),
        ],
      ),
    );

    final channel = result?.replaceFirst(RegExp(r'^#'), '').trim() ?? '';
    if (channel.isEmpty || !mounted) return;
    final ok = await _service.subscribeTag(channel);
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(
          ok
              ? 'Joined #$channel'
              : (_service.state.lastError ?? 'Failed to add channel'),
        ),
      ),
    );
  }
}

class _FeedView extends StatelessWidget {
  final SocialController controller;
  final ListController listController;
  final VoidCallback onBackup;
  final VoidCallback onDismissReminder;
  final bool showBackupReminder;

  const _FeedView({
    required this.controller,
    required this.listController,
    required this.onBackup,
    required this.onDismissReminder,
    required this.showBackupReminder,
  });

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
                  message:
                      'Your personal social node is active and syncing. Follow people or channels to see content!',
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
                    if (showBackupReminder && !state.hasBackedUp)
                      _BackupReminder(
                        onBackup: onBackup,
                        onDismiss: onDismissReminder,
                      ),
                    LiveStatusBanner(controller: controller),
                  ],
                );
              }
              final event = feed[index - 1];
              if (event.isPoll) {
                return PollWidget(event: event, controller: controller);
              }
              return VeilPostCard(
                event: event,
                controller: controller,
                listController: listController,
              );
            },
          ),
        );
      },
    );
  }
}

class _BackupReminder extends StatelessWidget {
  final VoidCallback onDismiss;
  final VoidCallback onBackup;

  const _BackupReminder({required this.onDismiss, required this.onBackup});

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
      child: Column(
        children: [
          Row(
            children: [
              const Icon(Icons.warning_amber_rounded, color: Colors.amber),
              const SizedBox(width: 12),
              const Expanded(
                child: Text(
                  'Identity not backed up! Save your secret keys to avoid losing access.',
                  style: TextStyle(fontSize: 13, color: Colors.amber),
                ),
              ),
              IconButton(
                icon: const Icon(Icons.close, size: 18, color: Colors.amber),
                onPressed: onDismiss,
              ),
            ],
          ),
          const SizedBox(height: 8),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              TextButton(
                onPressed: onBackup,
                child: const Text('BACK UP NOW',
                    style: TextStyle(
                        color: Colors.amber,
                        fontWeight: FontWeight.bold,
                        fontSize: 12)),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
