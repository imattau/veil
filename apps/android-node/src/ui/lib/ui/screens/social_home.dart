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
import '../components/entrance_fader.dart';

class SocialHome extends StatefulWidget {
  final NodeService service;
  final PreferencesController preferencesController;

  const SocialHome({
    super.key,
    required this.service,
    required this.preferencesController,
  });

  @override
  State<SocialHome> createState() => _SocialHomeState();
}

class _SocialHomeState extends State<SocialHome> {
  late final NodeService _service;
  late final SocialController _controller;
  late final MessagingController _messagingController;
  late final ListController _listController;
  int _currentIndex = 0;
  bool _hideBackupReminder = false;
  String? _lastShownErrorKey;
  DateTime? _lastShownErrorAt;
  static const Duration _errorToastCooldown = Duration(seconds: 20);
  final List<ScrollController> _scrollControllers = List.generate(
    4,
    (_) => ScrollController(),
  );

  @override
  void initState() {
    super.initState();
    _service = widget.service;
    _controller = SocialController(_service);
    _messagingController = MessagingController(_service);
    _listController = ListController(_service);
    _service.addListener(_handleError);
  }

  void _handleError() {
    final error = _service.state.lastError;
    if (error != null && error.isNotEmpty) {
      _service.clearError();
      if (mounted) {
        if (_shouldShowErrorToast(error)) {
          ScaffoldMessenger.of(context)
            ..hideCurrentSnackBar()
            ..showSnackBar(SnackBar(content: Text(error)));
        }
      }
    }
  }

  bool _shouldShowErrorToast(String message) {
    final now = DateTime.now();
    final key = _errorToastKey(message);
    if (_lastShownErrorKey == key &&
        _lastShownErrorAt != null &&
        now.difference(_lastShownErrorAt!) < _errorToastCooldown) {
      return false;
    }
    _lastShownErrorKey = key;
    _lastShownErrorAt = now;
    return true;
  }

  String _errorToastKey(String message) {
    if (message.startsWith('Publish failed')) return 'publish_failed';
    if (message.startsWith('Publish dropped')) return 'publish_dropped';
    if (message.startsWith('Cannot reach node')) return 'cannot_reach_node';
    final normalized = message
        .replaceAll(RegExp(r'\(attempt \d+\)'), '(attempt)')
        .replaceAll(RegExp(r'retry in \d+ms'), 'retry in Xms');
    return normalized;
  }

  @override
  void dispose() {
    _service.removeListener(_handleError);
    for (final sc in _scrollControllers) {
      sc.dispose();
    }
    _controller.dispose();
    _messagingController.dispose();
    _listController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: _service,
      builder: (context, _) {
        final mediaQuery = MediaQuery.of(context);
        final theme = Theme.of(context);
        final surface = theme.colorScheme.surface;
        final secondaryText =
            theme.textTheme.labelSmall?.color ?? VeilTheme.textSecondary;
        final topInset = mediaQuery.padding.top + kToolbarHeight + 16;
        final tabBottomInset =
            kBottomNavigationBarHeight + mediaQuery.padding.bottom + 80;
        final profileBottomInset =
            kBottomNavigationBarHeight + mediaQuery.padding.bottom + 24;

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
                  backgroundColor: surface.withValues(alpha: 0.72),
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
                onDismissReminder: () =>
                    setState(() => _hideBackupReminder = true),
                onBackup: () =>
                    setState(() => _currentIndex = 3), // Profile tab
                onDiscover: () => setState(() => _currentIndex = 1),
                scrollController: _scrollControllers[0],
                topInset: topInset,
                bottomInset: tabBottomInset,
              ),
              ExploreView(
                service: _service,
                scrollController: _scrollControllers[1],
                topInset: topInset,
                bottomInset: tabBottomInset,
                onAddChannel: _showAddChannelDialog,
              ),
              InboxView(
                controller: _messagingController,
                socialController: _controller,
                scrollController: _scrollControllers[2],
                topInset: topInset,
                bottomInset: tabBottomInset,
              ),
              ProfileView(
                service: _service,
                controller: _controller,
                listController: _listController,
                preferencesController: widget.preferencesController,
                scrollController: _scrollControllers[3],
                topInset: topInset,
                bottomInset: profileBottomInset,
              ),
            ],
          ),
          bottomNavigationBar: ClipRect(
            child: BackdropFilter(
              filter: ImageFilter.blur(sigmaX: 10, sigmaY: 10),
              child: BottomNavigationBar(
                currentIndex: _currentIndex,
                onTap: (i) {
                  if (i == _currentIndex) {
                    // Re-tap: scroll to top
                    if (_scrollControllers[i].hasClients) {
                      _scrollControllers[i].animateTo(
                        0,
                        duration: const Duration(milliseconds: 300),
                        curve: Curves.easeOut,
                      );
                    }
                  } else {
                    setState(() => _currentIndex = i);
                  }
                },
                type: BottomNavigationBarType.fixed,
                backgroundColor: surface.withValues(alpha: 0.72),
                selectedItemColor: theme.colorScheme.primary,
                unselectedItemColor: secondaryText.withValues(alpha: 0.88),
                showSelectedLabels: true,
                showUnselectedLabels: true,
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
    final theme = Theme.of(context);
    final fabForeground =
        ThemeData.estimateBrightnessForColor(theme.colorScheme.primary) ==
            Brightness.dark
        ? Colors.white
        : Colors.black;

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
        final preferredChannel = widget.preferencesController.defaultChannel
            .trim()
            .replaceFirst(RegExp(r'^#'), '');
        Navigator.push(
          context,
          MaterialPageRoute(
            builder: (context) => ComposerView(
              service: _service,
              initialChannel: preferredChannel.isEmpty
                  ? null
                  : preferredChannel,
            ),
            fullscreenDialog: true,
          ),
        );
      };
    }

    final label = switch (_currentIndex) {
      2 => 'New Message',
      1 => 'Add Channel',
      _ => 'Post',
    };

    return FloatingActionButton.extended(
      onPressed: onPressed,
      backgroundColor: theme.colorScheme.primary,
      foregroundColor: fabForeground,
      icon: Icon(icon),
      label: Text(label, style: const TextStyle(fontWeight: FontWeight.w700)),
    );
  }

  Future<void> _showAddChannelDialog() async {
    final controller = TextEditingController();
    final result = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(VeilTheme.radiusM),
        ),
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
    controller.dispose();

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
  final VoidCallback onDiscover;
  final VoidCallback onDismissReminder;
  final bool showBackupReminder;
  final ScrollController scrollController;
  final double topInset;
  final double bottomInset;

  const _FeedView({
    required this.controller,
    required this.listController,
    required this.onBackup,
    required this.onDiscover,
    required this.onDismissReminder,
    required this.showBackupReminder,
    required this.scrollController,
    required this.topInset,
    required this.bottomInset,
  });

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final feed = controller.feed;
        final state = controller.nodeService.state;

        if (state.busy && feed.isEmpty) {
          return const FeedShimmer();
        }

        if (!state.busy && feed.isEmpty) {
          return RefreshIndicator(
            onRefresh: controller.nodeService.refresh,
            child: SingleChildScrollView(
              controller: scrollController,
              physics: const AlwaysScrollableScrollPhysics(),
              padding: EdgeInsets.fromLTRB(16, topInset, 16, bottomInset),
              child: SizedBox(
                height: MediaQuery.of(context).size.height * 0.7,
                child: EmptyState(
                  icon: Icons.bubble_chart_outlined,
                  title: 'Welcome to the Veil',
                  message:
                      'Your personal social node is active and syncing. Follow people or channels to see content.',
                  onAction: onDiscover,
                  actionLabel: 'Explore Channels',
                ),
              ),
            ),
          );
        }

        return RefreshIndicator(
          onRefresh: controller.nodeService.refresh,
          child: ListView.builder(
            controller: scrollController,
            padding: EdgeInsets.fromLTRB(16, topInset, 16, bottomInset),
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
              final item = event.isPoll
                  ? PollWidget(event: event, controller: controller)
                  : VeilPostCard(
                      event: event,
                      controller: controller,
                      listController: listController,
                    );

              return EntranceFader(
                delay: Duration(milliseconds: (index * 50).clamp(0, 400)),
                child: item,
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
        color: Colors.amber.withValues(alpha: 0.1),
        borderRadius: BorderRadius.circular(VeilTheme.radiusS),
        border: Border.all(color: Colors.amber.withValues(alpha: 0.3)),
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
                child: const Text(
                  'BACK UP NOW',
                  style: TextStyle(
                    color: Colors.amber,
                    fontWeight: FontWeight.bold,
                    fontSize: 12,
                  ),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
