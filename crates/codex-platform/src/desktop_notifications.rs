/// Best-effort native alert for a completed background chat.
pub struct BackgroundCompletionNotifier {
    #[cfg(windows)]
    inner: Option<windows::WindowsBackgroundCompletionNotifier>,
}

impl BackgroundCompletionNotifier {
    #[must_use]
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            inner: windows::WindowsBackgroundCompletionNotifier::new(),
        }
    }

    /// Queues one completion alert without blocking the UI thread.
    pub fn notify_completed(&self) {
        #[cfg(windows)]
        if let Some(inner) = &self.inner {
            inner.notify_completed();
        }
    }

    /// Updates the notification-area activity count without blocking the UI thread.
    pub fn set_background_chat_count(&self, count: usize) {
        #[cfg(windows)]
        if let Some(inner) = &self.inner {
            inner.set_background_chat_count(count);
        }
        #[cfg(not(windows))]
        let _ = count;
    }
}

impl Default for BackgroundCompletionNotifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
mod windows {
    use std::{
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        thread::{self, JoinHandle},
    };

    use crossbeam_channel::{Receiver, Sender, bounded};
    use winsafe::{
        self as w, gui,
        prelude::{GuiEvents, GuiParent, GuiWindow, Handle, user_Hwnd},
    };

    const COMMAND_CHANNEL_CAPACITY: usize = 1;
    const TRAY_ICON_ID: u32 = 1;
    const NOTIFICATION_TITLE: &str = "codexRS";
    const NOTIFICATION_BODY: &str = "A background chat completed.";

    pub(super) struct WindowsBackgroundCompletionNotifier {
        commands: Sender<()>,
        ready: Receiver<w::HWND>,
        hwnd: Mutex<Option<w::HWND>>,
        background_chat_count: Arc<AtomicUsize>,
        background_chat_count_wake_pending: Arc<AtomicBool>,
        shutdown: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
    }

    impl WindowsBackgroundCompletionNotifier {
        pub(super) fn new() -> Option<Self> {
            let (commands, command_receiver) = bounded(COMMAND_CHANNEL_CAPACITY);
            let (ready_sender, ready_receiver) = bounded(1);
            let background_chat_count = Arc::new(AtomicUsize::new(0));
            let background_chat_count_wake_pending = Arc::new(AtomicBool::new(false));
            let shutdown = Arc::new(AtomicBool::new(false));
            let host_background_chat_count = Arc::clone(&background_chat_count);
            let host_background_chat_count_wake_pending =
                Arc::clone(&background_chat_count_wake_pending);
            let host_shutdown = Arc::clone(&shutdown);
            let thread = thread::Builder::new()
                .name("codex-background-completion-notification".to_owned())
                .spawn(move || {
                    run_host(
                        command_receiver,
                        ready_sender,
                        host_background_chat_count,
                        host_background_chat_count_wake_pending,
                        host_shutdown,
                    )
                })
                .ok()?;

            Some(Self {
                commands,
                ready: ready_receiver,
                hwnd: Mutex::new(None),
                background_chat_count,
                background_chat_count_wake_pending,
                shutdown,
                thread: Some(thread),
            })
        }

        pub(super) fn notify_completed(&self) {
            if self.commands.try_send(()).is_ok() {
                self.post_wake();
            }
        }

        pub(super) fn set_background_chat_count(&self, count: usize) {
            self.background_chat_count.store(count, Ordering::Release);
            if !self
                .background_chat_count_wake_pending
                .swap(true, Ordering::AcqRel)
            {
                self.post_wake();
            }
        }

        fn post_wake(&self) {
            let Ok(mut hwnd) = self.hwnd.lock() else {
                return;
            };
            if hwnd.is_none() {
                *hwnd = self.ready.try_recv().ok();
            }
            if let Some(hwnd) = hwnd.as_ref() {
                let _ = hwnd.PostMessage(w::msg::WndMsg::new(w::co::WM::APP, 0, 0));
            }
        }
    }

    impl Drop for WindowsBackgroundCompletionNotifier {
        fn drop(&mut self) {
            self.shutdown.store(true, Ordering::Release);
            let _ = self.commands.try_send(());
            self.post_wake();
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    fn run_host(
        commands: Receiver<()>,
        ready: Sender<w::HWND>,
        background_chat_count: Arc<AtomicUsize>,
        background_chat_count_wake_pending: Arc<AtomicBool>,
        shutdown: Arc<AtomicBool>,
    ) {
        let window = gui::WindowMain::new(gui::WindowMainOpts {
            class_name: "CodexBackgroundCompletionNotificationHost".to_owned(),
            class_icon: gui::Icon::Idi(w::co::IDI::INFORMATION),
            class_cursor: gui::Cursor::None,
            class_bg_brush: gui::Brush::None,
            title: NOTIFICATION_TITLE.to_owned(),
            size: (1, 1),
            style: w::co::WS::POPUP,
            ex_style: w::co::WS_EX::NOACTIVATE | w::co::WS_EX::TOOLWINDOW,
            ..Default::default()
        });

        let icon_registered = Arc::new(AtomicBool::new(false));
        let create_window = window.clone();
        let create_registered = Arc::clone(&icon_registered);
        let create_shutdown = Arc::clone(&shutdown);
        window.on().wm_create(move |_| {
            let request_shutdown = || {
                create_shutdown.store(true, Ordering::Release);
                let _ = create_window
                    .hwnd()
                    .PostMessage(w::msg::WndMsg::new(w::co::WM::APP, 0, 0));
            };
            if create_shutdown.load(Ordering::Acquire) {
                request_shutdown();
                return Ok(0);
            }
            let Some(icon_hwnd) = create_window.hwnd().GetAncestor(w::co::GA::ROOT) else {
                request_shutdown();
                return Ok(0);
            };
            if add_notification_icon(icon_hwnd).is_err() {
                request_shutdown();
                return Ok(0);
            }
            create_registered.store(true, Ordering::Release);
            if create_shutdown.load(Ordering::Acquire) {
                if let Some(cleanup_hwnd) = create_window.hwnd().GetAncestor(w::co::GA::ROOT) {
                    remove_notification_icon(cleanup_hwnd);
                }
                create_registered.store(false, Ordering::Release);
                request_shutdown();
                return Ok(0);
            }
            let Some(ready_hwnd) = create_window.hwnd().GetAncestor(w::co::GA::ROOT) else {
                create_registered.store(false, Ordering::Release);
                request_shutdown();
                return Ok(0);
            };
            if ready.try_send(ready_hwnd).is_err() {
                if create_registered.swap(false, Ordering::AcqRel)
                    && let Some(cleanup_hwnd) = create_window.hwnd().GetAncestor(w::co::GA::ROOT)
                {
                    remove_notification_icon(cleanup_hwnd);
                }
                request_shutdown();
                return Ok(0);
            }
            if let Some(wake_hwnd) = create_window.hwnd().GetAncestor(w::co::GA::ROOT) {
                let _ = wake_hwnd.PostMessage(w::msg::WndMsg::new(w::co::WM::APP, 0, 0));
            }
            Ok(0)
        });

        let wake_window = window.clone();
        let wake_registered = Arc::clone(&icon_registered);
        let wake_background_chat_count = Arc::clone(&background_chat_count);
        let wake_background_chat_count_wake_pending =
            Arc::clone(&background_chat_count_wake_pending);
        let displayed_background_chat_count = std::cell::Cell::new(0);
        window.on().wm(w::co::WM::APP, move |_| {
            if shutdown.load(Ordering::Acquire) {
                if wake_registered.swap(false, Ordering::AcqRel)
                    && let Some(hwnd) = wake_window.hwnd().GetAncestor(w::co::GA::ROOT)
                {
                    remove_notification_icon(hwnd);
                }
                wake_window.hwnd().DestroyWindow()?;
                return Ok(Some(0));
            }
            wake_background_chat_count_wake_pending.store(false, Ordering::Release);
            let current_background_chat_count = wake_background_chat_count.load(Ordering::Acquire);
            if current_background_chat_count != displayed_background_chat_count.get()
                && wake_registered.load(Ordering::Acquire)
                && let Some(hwnd) = wake_window.hwnd().GetAncestor(w::co::GA::ROOT)
                && update_notification_tooltip(hwnd, current_background_chat_count).is_ok()
            {
                displayed_background_chat_count.set(current_background_chat_count);
            }
            if commands.try_recv().is_ok()
                && wake_registered.load(Ordering::Acquire)
                && let Some(hwnd) = wake_window.hwnd().GetAncestor(w::co::GA::ROOT)
            {
                let _ = show_notification(hwnd);
            }
            Ok(Some(0))
        });

        let _ = window.run_main(Some(w::co::SW::HIDE));
    }

    fn add_notification_icon(hwnd: w::HWND) -> Result<(), w::co::ERROR> {
        let icon = gui::Icon::Idi(w::co::IDI::INFORMATION).as_hicon(&w::HINSTANCE::NULL)?;
        let mut data = w::NOTIFYICONDATA::default();
        data.hWnd = hwnd;
        data.uID = TRAY_ICON_ID;
        data.uFlags = w::co::NIF::ICON | w::co::NIF::TIP;
        data.hIcon = icon;
        data.set_szTip(&background_chat_tooltip(0));
        w::Shell_NotifyIcon(w::co::NIM::ADD, &mut data)
    }

    fn update_notification_tooltip(hwnd: w::HWND, count: usize) -> Result<(), w::co::ERROR> {
        let mut data = w::NOTIFYICONDATA::default();
        data.hWnd = hwnd;
        data.uID = TRAY_ICON_ID;
        data.uFlags = w::co::NIF::TIP;
        data.set_szTip(&background_chat_tooltip(count));
        w::Shell_NotifyIcon(w::co::NIM::MODIFY, &mut data)
    }

    fn background_chat_tooltip(count: usize) -> String {
        match count {
            0 => NOTIFICATION_TITLE.to_owned(),
            1 => format!("{NOTIFICATION_TITLE} · 1 chat running"),
            count => format!("{NOTIFICATION_TITLE} · {count} chats running"),
        }
    }

    fn show_notification(hwnd: w::HWND) -> Result<(), w::co::ERROR> {
        let mut data = w::NOTIFYICONDATA::default();
        data.hWnd = hwnd;
        data.uID = TRAY_ICON_ID;
        data.uFlags = w::co::NIF::INFO;
        data.dwInfoFlags = w::co::NIIF::INFO | w::co::NIIF::RESPECT_QUIET_TIME;
        data.set_szInfoTitle(NOTIFICATION_TITLE);
        data.set_szInfo(NOTIFICATION_BODY);
        w::Shell_NotifyIcon(w::co::NIM::MODIFY, &mut data)
    }

    fn remove_notification_icon(hwnd: w::HWND) {
        let mut data = w::NOTIFYICONDATA::default();
        data.hWnd = hwnd;
        data.uID = TRAY_ICON_ID;
        let _ = w::Shell_NotifyIcon(w::co::NIM::DELETE, &mut data);
    }

    #[cfg(test)]
    mod tests {
        use super::{NOTIFICATION_BODY, NOTIFICATION_TITLE, background_chat_tooltip};

        #[test]
        fn completion_notification_copy_fits_shell_bounds() {
            assert_eq!(NOTIFICATION_TITLE, "codexRS");
            assert_eq!(NOTIFICATION_BODY, "A background chat completed.");
            assert!(NOTIFICATION_TITLE.len() < 64);
            assert!(NOTIFICATION_BODY.len() < 256);
        }

        #[test]
        fn background_chat_tooltip_uses_the_bounded_title_wording() {
            assert_eq!(background_chat_tooltip(0), "codexRS");
            assert_eq!(background_chat_tooltip(1), "codexRS · 1 chat running");
            assert_eq!(background_chat_tooltip(2), "codexRS · 2 chats running");
            assert!(background_chat_tooltip(usize::MAX).len() < 128);
        }
    }
}
