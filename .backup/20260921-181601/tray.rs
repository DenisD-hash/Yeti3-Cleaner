#![cfg(target_os = "macos")]
#![deny(unsafe_op_in_unsafe_fn)]

#[path = "../config/mod.rs"]
mod config;

#[path = "../history/mod.rs"]
mod history;

#[path = "../ui/mod.rs"]
mod ui;

use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    process::{Child, Command, Stdio},
};

use config::settings::Settings;

use serde::Deserialize;
use ui::{
    about, launcher,
    menu::{PRODUCT_NAME, PRODUCT_TAGLINE, TrayState},
    result, settings as settings_ui, statistics, theme,
};

use objc2::{
    AnyThread, DefinedClass, MainThreadOnly, class, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, Sel},
};

use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSButton, NSColor,
    NSControlStateValueOff, NSControlStateValueOn, NSImage, NSImageView, NSMenu, NSMenuItem,
    NSScrollView, NSStatusBar, NSStatusBarButton, NSTextField, NSVariableStatusItemLength, NSView,
    NSWindow, NSWindowStyleMask,
};

use objc2_foundation::{
    MainThreadMarker, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};

thread_local! {
    static SETTINGS_WINDOW: RefCell<Option<Retained<NSWindow>>> =
        const { RefCell::new(None) };

    static STATISTICS_WINDOW: RefCell<Option<Retained<NSWindow>>> =
        const { RefCell::new(None) };

    static RESULT_WINDOW: RefCell<Option<Retained<NSWindow>>> =
        const { RefCell::new(None) };

    static LAUNCH_WINDOW: RefCell<Option<Retained<NSWindow>>> =
        const { RefCell::new(None) };

    static LAUNCH_IMAGE_VIEW: RefCell<Option<Retained<NSImageView>>> =
        const { RefCell::new(None) };
}

#[derive(Debug, Default)]
struct ControllerIvars {
    child: RefCell<Option<Child>>,
    paused: Cell<bool>,
    animation_frame: Cell<usize>,
    launch_frame: Cell<usize>,
    result_before_run: Cell<i64>,
    status_button: RefCell<Option<Retained<NSStatusBarButton>>>,
    start_item: RefCell<Option<Retained<NSMenuItem>>>,
    stop_item: RefCell<Option<Retained<NSMenuItem>>>,
}

#[derive(Debug, Deserialize)]
struct CleanupResultCategory {
    category: String,
    bytes: u64,
}

#[derive(Debug, Deserialize)]
struct CleanupResult {
    id: i64,
    free_before: u64,
    free_after: u64,
    discovered_bytes: u64,
    reclaimed_bytes: u64,
    files_deleted: u64,
    dirs_deleted: u64,
    skipped: u64,
    errors: u64,
    duration_ms: u64,
    status: String,
    #[serde(default)]
    categories: Vec<CleanupResultCategory>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ControllerIvars]
    struct Controller;

    unsafe impl NSObjectProtocol for Controller {}

    impl Controller {
        #[unsafe(method(startPause:))]
        fn start_pause(&self, _sender: Option<&AnyObject>) {
            self.reap_finished();

            let mut child_slot = self.ivars().child.borrow_mut();

            if let Some(child) = child_slot.as_mut() {
                let pid = child.id() as i32;

                if self.ivars().paused.get() {
                    unsafe {
                        libc::kill(pid, libc::SIGCONT);
                    }

                    self.ivars().paused.set(false);
                    self.ivars().animation_frame.set(0);
                    self.set_start_title("Пауза");
                    self.set_status_image("cleaning-00.png");
                } else {
                    unsafe {
                        libc::kill(pid, libc::SIGSTOP);
                    }

                    self.ivars().paused.set(true);
                    self.set_start_title("Продолжить");
                    self.set_status_image("paused.png");
                }

                self.set_stop_enabled(true);
                return;
            }

            let executable = cleaner_path();

            self.ivars()
                .result_before_run
                .set(latest_result_id());

            match Command::new(&executable)
                .args(["clean", "--max", "--yes"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(child) => {
                    *child_slot = Some(child);
                    self.ivars().paused.set(false);
                    self.ivars().animation_frame.set(0);
                    self.set_start_title("Пауза");
                    self.set_stop_enabled(true);
                    self.set_status_image("cleaning-00.png");
                }

                Err(error) => {
                    eprintln!(
                        "Yeti3-Cleaner: cannot start {}: {}",
                        executable.display(),
                        error
                    );

                    self.set_start_title("Старт");
                    self.set_stop_enabled(false);
                    self.set_status_image("error.png");
                }
            }
        }

        #[unsafe(method(stop:))]
        fn stop_action(&self, _sender: Option<&AnyObject>) {
            self.stop_child();
        }

        #[unsafe(method(resetStatusIcon:))]
        fn reset_status_icon(&self, _sender: Option<&AnyObject>) {
            if self.ivars().child.borrow().is_none() {
                self.set_status_image("idle.png");
            }
        }

        #[unsafe(method(animationTick:))]
        fn animation_tick(&self, _sender: Option<&AnyObject>) {
            self.reap_finished();

            if self.ivars().child.borrow().is_none() {
                return;
            }

            if self.ivars().paused.get() {
                self.set_status_image("paused.png");
                return;
            }

            let frame = self.ivars().animation_frame.get() % 12;

            self.set_status_image(
                &format!("cleaning-{frame:02}.png")
            );

            self.ivars()
                .animation_frame
                .set((frame + 1) % 12);
        }

        #[unsafe(method(launchTick:))]
        fn launch_tick(&self, _sender: Option<&AnyObject>) {
            let frame = self.ivars().launch_frame.get();

            if frame >= 24 {
                LAUNCH_WINDOW.with(|slot| {
                    if let Some(window) = slot.borrow_mut().take() {
                        window.orderOut(None);
                        window.close();
                    }
                });

                LAUNCH_IMAGE_VIEW.with(|slot| {
                    slot.borrow_mut().take();
                });

                return;
            }

            let name = format!("frame-{frame:02}.png");

            if let Some(image) = load_resource_image(&name, false) {
                LAUNCH_IMAGE_VIEW.with(|slot| {
                    if let Some(view) = slot.borrow().as_ref() {
                        view.setImage(Some(&image));
                    }
                });
            }

            self.ivars().launch_frame.set(frame + 1);
        }

        #[unsafe(method(closeResultPopup:))]
        fn close_result_popup(
            &self,
            _sender: Option<&AnyObject>,
        ) {
            hide_result_popup();
            self.set_status_image("idle.png");
        }

        #[unsafe(method(openStatistics:))]
        fn open_statistics(
            &self,
            _sender: Option<&AnyObject>,
        ) {
            show_statistics_window(self);
        }

        #[unsafe(method(openSettings:))]
        fn open_settings(&self, _sender: Option<&AnyObject>) {
            show_settings_window(self);
        }

        #[unsafe(method(toggleSetting:))]
        fn toggle_setting(&self, sender: Option<&AnyObject>) {
            let Some(sender) = sender else {
                return;
            };

            let tag: isize = unsafe {
                msg_send![sender, tag]
            };

            let state: isize = unsafe {
                msg_send![sender, state]
            };

            let enabled = state == NSControlStateValueOn;

            match config::load() {
                Ok(mut settings) => {
                    set_bool_setting(&mut settings, tag, enabled);

                    if let Err(error) = config::save(&settings) {
                        eprintln!("settings save failed: {error}");
                    }
                }

                Err(error) => {
                    eprintln!("settings load failed: {error}");
                }
            }
        }

        #[unsafe(method(ageChanged:))]
        fn age_changed(&self, sender: Option<&AnyObject>) {
            let Some(sender) = sender else {
                return;
            };

            let tag: isize = unsafe {
                msg_send![sender, tag]
            };

            let value: isize = unsafe {
                msg_send![sender, integerValue]
            };

            let value = value.max(0) as u64;

            match config::load() {
                Ok(mut settings) => {
                    match tag {
                        1001 => {
                            settings.macos.temporary_min_age_days = value;
                        }
                        1002 => {
                            settings.macos.logs_min_age_days = value;
                        }
                        1003 => {
                            settings
                                .development
                                .xcode_source_packages_min_age_days = value;
                        }
                        1004 => {
                            settings.development.gradle_min_age_days = value;
                        }
                        1005 => {
                            settings.podman.stale_machine_days = value;
                        }
                        _ => {}
                    }

                    if let Err(error) = config::save(&settings) {
                        eprintln!("settings save failed: {error}");
                    }
                }

                Err(error) => {
                    eprintln!("settings load failed: {error}");
                }
            }
        }

        #[unsafe(method(resetDefaults:))]
        fn reset_defaults(&self, _sender: Option<&AnyObject>) {
            let defaults = Settings::default();

            if let Err(error) = config::save(&defaults) {
                eprintln!("settings save failed: {error}");
                return;
            }

            SETTINGS_WINDOW.with(|slot| {
                if let Some(window) = slot.borrow_mut().take() {
                    window.close();
                }
            });

            show_settings_window(self);
        }

        #[unsafe(method(closeSettings:))]
        fn close_settings(&self, _sender: Option<&AnyObject>) {
            SETTINGS_WINDOW.with(|slot| {
                if let Some(window) = slot.borrow_mut().take() {
                    window.close();
                }
            });
        }

        #[unsafe(method(about:))]
        fn about(&self, _sender: Option<&AnyObject>) {
            let version = env!("CARGO_PKG_VERSION");

            let alert: Retained<objc2_app_kit::NSAlert> = unsafe {
                msg_send![
                    objc2_app_kit::NSAlert::alloc(self.mtm()),
                    init
                ]
            };

            alert.setMessageText(&ns("YETI³ Cleaner"));

            alert.setInformativeText(&ns(
                &format!(
                    "Version {version}\n\nDeep macOS Cleanup · Apple Silicon\n\nMore space. A brighter you.\n\nyeti3.ru"
                )
            ));

            if let Some(icon) =
                load_resource_image("Yeti3-About.png", false)
            {
                unsafe {
                    alert.setIcon(Some(&icon));
                }
            }

            alert.addButtonWithTitle(&ns("Готово"));
            alert.runModal();
        }

        #[unsafe(method(quit:))]
        fn quit(&self, _sender: Option<&AnyObject>) {
            self.stop_child();

            let app = NSApplication::sharedApplication(self.mtm());
            app.terminate(None);
        }
    }
);

impl Controller {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ControllerIvars::default());

        unsafe { msg_send![super(this), init] }
    }

    fn stop_child(&self) {
        let mut slot = self.ivars().child.borrow_mut();

        if let Some(child) = slot.as_mut() {
            let pid = child.id() as i32;

            if self.ivars().paused.get() {
                unsafe {
                    libc::kill(pid, libc::SIGCONT);
                }
            }

            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }

            let _ = child.wait();
        }

        *slot = None;
        self.ivars().paused.set(false);

        self.set_start_title("Старт");
        self.set_stop_enabled(false);
        self.set_status_image("idle.png");
    }

    fn reap_finished(&self) {
        let mut slot = self.ivars().child.borrow_mut();

        let finished = match slot.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(_)) => true,
                Ok(None) => false,
                Err(_) => true,
            },

            None => false,
        };

        if finished {
            *slot = None;
            self.ivars().paused.set(false);
            self.set_start_title("Старт");
            self.set_stop_enabled(false);

            let baseline = self.ivars().result_before_run.get();

            match read_latest_result() {
                Some(result) if result.id > baseline => {
                    self.set_status_image("complete.png");
                    show_result_popup(self, &result);
                }

                _ => {
                    self.set_status_image("error.png");

                    let controller: &Controller = self;
                    let _: () = unsafe {
                        msg_send![
                            class!(NSTimer),
                            scheduledTimerWithTimeInterval: 3.0f64,
                            target: controller as &AnyObject,
                            selector: objc2::sel!(resetStatusIcon:),
                            userInfo: std::ptr::null::<AnyObject>(),
                            repeats: false
                        ]
                    };
                }
            }
        }
    }

    fn set_status_image(&self, resource: &str) {
        if let Some(button) = self.ivars().status_button.borrow().as_ref() {
            if let Some(image) = load_resource_image(resource, true) {
                button.setTitle(&ns(""));
                button.setImage(Some(&image));
                image.setSize(NSSize::new(18.0, 18.0));
            }
        }
    }

    fn set_start_title(&self, title: &str) {
        if let Some(item) = self.ivars().start_item.borrow().as_ref() {
            item.setTitle(&ns(title));
        }
    }

    fn set_stop_enabled(&self, enabled: bool) {
        if let Some(item) = self.ivars().stop_item.borrow().as_ref() {
            item.setEnabled(enabled);
        }
    }
}

fn ns(value: &str) -> Retained<NSString> {
    NSString::from_str(value)
}

fn menu_item(
    mtm: MainThreadMarker,
    title: &str,
    selector: Sel,
    target: &Controller,
) -> Retained<NSMenuItem> {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &ns(title),
            Some(selector),
            &ns(""),
        )
    };

    unsafe {
        item.setTarget(Some(target.as_ref()));
    }

    item
}

fn result_json_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;

    Some(home.join("Library/Application Support/Yeti3-Cleaner/latest-result.json"))
}

fn read_latest_result() -> Option<CleanupResult> {
    let path = result_json_path()?;
    let bytes = std::fs::read(path).ok()?;

    serde_json::from_slice(&bytes).ok()
}

fn latest_result_id() -> i64 {
    read_latest_result().map(|result| result.id).unwrap_or(0)
}

fn format_bytes(value: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * KIB;
    const GIB: f64 = 1024.0 * MIB;
    const TIB: f64 = 1024.0 * GIB;

    let value_f = value as f64;

    if value_f >= TIB {
        format!("{:.2} TB", value_f / TIB)
    } else if value_f >= GIB {
        format!("{:.2} GB", value_f / GIB)
    } else if value_f >= MIB {
        format!("{:.1} MB", value_f / MIB)
    } else if value_f >= KIB {
        format!("{:.1} KB", value_f / KIB)
    } else {
        format!("{value} B")
    }
}

fn format_duration(ms: u64) -> String {
    let seconds = ms / 1000;

    if seconds < 60 {
        format!("{seconds} сек")
    } else {
        format!("{} мин {} сек", seconds / 60, seconds % 60)
    }
}

fn popup_label(
    mtm: MainThreadMarker,
    parent: &NSView,
    text: &str,
    frame: NSRect,
    font_size: f64,
    bold: bool,
    secondary: bool,
) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&ns(text), mtm);

    label.setFrame(frame);

    unsafe {
        let font: *mut AnyObject = if bold {
            msg_send![
                class!(NSFont),
                boldSystemFontOfSize: font_size
            ]
        } else {
            msg_send![
                class!(NSFont),
                systemFontOfSize: font_size
            ]
        };

        let _: () = msg_send![
            &*label,
            setFont: font
        ];

        if secondary {
            let color: *mut AnyObject = msg_send![class!(NSColor), secondaryLabelColor];

            let _: () = msg_send![
                &*label,
                setTextColor: color
            ];
        }
    }

    parent.addSubview(&label);

    label
}

fn hide_result_popup() {
    RESULT_WINDOW.with(|slot| {
        if let Some(window) = slot.borrow_mut().take() {
            window.orderOut(None);
            window.close();
        }
    });
}

fn result_popup_origin(controller: &Controller, width: f64, height: f64) -> NSPoint {
    let Some(button) = controller.ivars().status_button.borrow().as_ref().cloned() else {
        return NSPoint::new(900.0, 500.0);
    };

    let button_object: &AnyObject = &button;

    unsafe {
        let owner_window: *mut AnyObject = msg_send![button_object, window];

        if owner_window.is_null() {
            return NSPoint::new(900.0, 500.0);
        }

        let button_frame: NSRect = msg_send![button_object, frame];

        let screen_frame: NSRect = msg_send![
            owner_window,
            convertRectToScreen: button_frame
        ];

        NSPoint::new(
            screen_frame.origin.x + screen_frame.size.width - width + 16.0,
            screen_frame.origin.y - height - 10.0,
        )
    }
}

fn show_result_popup(controller: &Controller, result: &CleanupResult) {
    hide_result_popup();

    let mtm = controller.mtm();

    let width = 390.0;
    let height = 470.0;

    let frame = NSRect::new(
        result_popup_origin(controller, width, height),
        NSSize::new(width, height),
    );

    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            NSWindowStyleMask::Borderless,
            NSBackingStoreType::Buffered,
            false,
        )
    };

    unsafe {
        window.setReleasedWhenClosed(false);
    }

    window.setOpaque(false);
    window.setHasShadow(true);
    window.setAlphaValue(0.98);

    window.setBackgroundColor(Some(&NSColor::windowBackgroundColor()));

    let Some(content) = window.contentView() else {
        return;
    };

    let mut y = 425.0;

    popup_label(
        mtm,
        &content,
        if result.status == "completed" && result.errors == 0 {
            "✓  ОЧИСТКА ЗАВЕРШЕНА"
        } else {
            "✓  ЗАВЕРШЕНО С ЗАМЕЧАНИЯМИ"
        },
        NSRect::new(NSPoint::new(24.0, y), NSSize::new(340.0, 28.0)),
        16.0,
        true,
        false,
    );

    y -= 31.0;

    popup_label(
        mtm,
        &content,
        &format!("{PRODUCT_NAME} · {PRODUCT_TAGLINE}"),
        NSRect::new(NSPoint::new(24.0, y), NSSize::new(340.0, 20.0)),
        11.0,
        false,
        true,
    );

    y -= 48.0;

    let rows = [
        ("Было свободно", format_bytes(result.free_before)),
        ("Стало свободно", format_bytes(result.free_after)),
        ("Найдено", format_bytes(result.discovered_bytes)),
    ];

    for (name, value) in rows {
        popup_label(
            mtm,
            &content,
            name,
            NSRect::new(NSPoint::new(24.0, y), NSSize::new(180.0, 20.0)),
            12.0,
            false,
            true,
        );

        popup_label(
            mtm,
            &content,
            &value,
            NSRect::new(NSPoint::new(220.0, y), NSSize::new(145.0, 20.0)),
            12.0,
            true,
            false,
        );

        y -= 24.0;
    }

    y -= 14.0;

    popup_label(
        mtm,
        &content,
        "ОСВОБОЖДЕНО",
        NSRect::new(NSPoint::new(24.0, y), NSSize::new(170.0, 22.0)),
        11.0,
        true,
        true,
    );

    popup_label(
        mtm,
        &content,
        &format_bytes(result.reclaimed_bytes),
        NSRect::new(NSPoint::new(190.0, y - 4.0), NSSize::new(175.0, 32.0)),
        24.0,
        true,
        false,
    );

    y -= 42.0;

    let reclaimed_ratio = if result.discovered_bytes == 0 {
        0.0
    } else {
        (result.reclaimed_bytes as f64 / result.discovered_bytes as f64).clamp(0.0, 1.0)
    };

    let blocks = (reclaimed_ratio * 28.0).round() as usize;

    let bar = format!(
        "{}{}",
        "━".repeat(blocks),
        "·".repeat(28usize.saturating_sub(blocks))
    );

    popup_label(
        mtm,
        &content,
        &bar,
        NSRect::new(NSPoint::new(24.0, y), NSSize::new(340.0, 18.0)),
        13.0,
        true,
        false,
    );

    y -= 38.0;

    let stats = [
        ("Файлов", result.files_deleted.to_string()),
        ("Каталогов", result.dirs_deleted.to_string()),
        ("Пропущено", result.skipped.to_string()),
        ("Ошибок", result.errors.to_string()),
        ("Время", format_duration(result.duration_ms)),
    ];

    for (name, value) in stats {
        popup_label(
            mtm,
            &content,
            name,
            NSRect::new(NSPoint::new(24.0, y), NSSize::new(150.0, 18.0)),
            11.0,
            false,
            true,
        );

        popup_label(
            mtm,
            &content,
            &value,
            NSRect::new(NSPoint::new(220.0, y), NSSize::new(145.0, 18.0)),
            11.0,
            true,
            false,
        );

        y -= 20.0;
    }

    y -= 12.0;

    popup_label(
        mtm,
        &content,
        "БОЛЬШЕ ВСЕГО ОЧИЩЕНО",
        NSRect::new(NSPoint::new(24.0, y), NSSize::new(300.0, 18.0)),
        10.0,
        true,
        true,
    );

    y -= 23.0;

    let non_zero = result
        .categories
        .iter()
        .filter(|category| category.bytes > 0)
        .take(3)
        .collect::<Vec<_>>();

    if non_zero.is_empty() {
        popup_label(
            mtm,
            &content,
            "Нет данных по категориям",
            NSRect::new(NSPoint::new(24.0, y), NSSize::new(330.0, 18.0)),
            11.0,
            false,
            true,
        );
    } else {
        let max_bytes = non_zero
            .iter()
            .map(|category| category.bytes)
            .max()
            .unwrap_or(1);

        for category in non_zero {
            popup_label(
                mtm,
                &content,
                &category.category,
                NSRect::new(NSPoint::new(24.0, y), NSSize::new(195.0, 18.0)),
                11.0,
                false,
                false,
            );

            popup_label(
                mtm,
                &content,
                &format_bytes(category.bytes),
                NSRect::new(NSPoint::new(245.0, y), NSSize::new(120.0, 18.0)),
                11.0,
                true,
                false,
            );

            y -= 16.0;

            let ratio = category.bytes as f64 / max_bytes as f64;

            let blocks = (ratio * 22.0).round() as usize;

            let category_bar = format!(
                "{}{}",
                "━".repeat(blocks),
                "·".repeat(22usize.saturating_sub(blocks))
            );

            popup_label(
                mtm,
                &content,
                &category_bar,
                NSRect::new(NSPoint::new(24.0, y), NSSize::new(330.0, 14.0)),
                9.0,
                false,
                true,
            );

            y -= 21.0;
        }
    }

    window.orderFrontRegardless();

    RESULT_WINDOW.with(|slot| {
        *slot.borrow_mut() = Some(window);
    });

    let controller_object: &AnyObject = controller;

    let _: () = unsafe {
        msg_send![
            class!(NSTimer),
            scheduledTimerWithTimeInterval: 8.0f64,
            target: controller_object,
            selector: objc2::sel!(closeResultPopup:),
            userInfo: std::ptr::null::<AnyObject>(),
            repeats: false
        ]
    };
}

fn statistics_color(red: f64, green: f64, blue: f64, alpha: f64) -> *mut AnyObject {
    unsafe {
        msg_send![
            class!(NSColor),
            colorWithCalibratedRed: red,
            green: green,
            blue: blue,
            alpha: alpha
        ]
    }
}

#[derive(Clone, Copy)]
struct StatisticsTextStyle {
    size: f64,
    bold: bool,
    secondary: bool,
}

fn statistics_label(
    mtm: MainThreadMarker,
    parent: &NSView,
    text: &str,
    frame: NSRect,
    style: StatisticsTextStyle,
) {
    let label = NSTextField::labelWithString(&ns(text), mtm);

    label.setFrame(frame);

    unsafe {
        let font: *mut AnyObject = if style.bold {
            msg_send![
                class!(NSFont),
                boldSystemFontOfSize: style.size
            ]
        } else {
            msg_send![
                class!(NSFont),
                systemFontOfSize: style.size
            ]
        };

        let _: () = msg_send![
            &*label,
            setFont: font
        ];

        let color: *mut AnyObject = if style.secondary {
            statistics_color(0.56, 0.72, 0.82, 1.0)
        } else {
            statistics_color(0.92, 0.98, 1.0, 1.0)
        };

        let _: () = msg_send![
            &*label,
            setTextColor: color
        ];
    }

    parent.addSubview(&label);
}

#[allow(clippy::too_many_arguments)]
fn statistics_progress(
    parent: &NSView,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    value: f64,
    max_value: f64,
) {
    let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(width, height));

    let indicator: *mut AnyObject = unsafe {
        let allocated: *mut AnyObject = msg_send![class!(NSProgressIndicator), alloc];

        msg_send![
            allocated,
            initWithFrame: frame
        ]
    };

    unsafe {
        let _: () = msg_send![
            indicator,
            setIndeterminate: false
        ];

        let _: () = msg_send![
            indicator,
            setMinValue: 0.0f64
        ];

        let _: () = msg_send![
            indicator,
            setMaxValue: max_value.max(1.0)
        ];

        let _: () = msg_send![
            indicator,
            setDoubleValue: value
        ];

        let _: () = msg_send![
            parent,
            addSubview: indicator
        ];
    }
}

fn statistics_card(
    mtm: MainThreadMarker,
    parent: &NSView,
    title: &str,
    value: &str,
    x: f64,
    y: f64,
    width: f64,
) {
    statistics_label(
        mtm,
        parent,
        title,
        NSRect::new(NSPoint::new(x, y + 41.0), NSSize::new(width, 18.0)),
        StatisticsTextStyle {
            size: 10.5,
            bold: true,
            secondary: true,
        },
    );

    statistics_label(
        mtm,
        parent,
        value,
        NSRect::new(NSPoint::new(x, y + 6.0), NSSize::new(width, 34.0)),
        StatisticsTextStyle {
            size: 23.0,
            bold: true,
            secondary: false,
        },
    );
}

fn show_statistics_window(controller: &Controller) {
    let mtm = controller.mtm();

    STATISTICS_WINDOW.with(|slot| {
        if let Some(window) = slot.borrow().as_ref() {
            window.makeKeyAndOrderFront(None);

            let app = NSApplication::sharedApplication(mtm);

            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);

            return;
        }

        let snapshot =
            match history::HistoryDb::open().and_then(|db| db.statistics_snapshot(Some(30))) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    eprintln!("statistics load failed: {error}");
                    return;
                }
            };

        let width = 880.0;
        let height = 690.0;

        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height)),
                NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Miniaturizable,
                NSBackingStoreType::Buffered,
                false,
            )
        };

        unsafe {
            window.setReleasedWhenClosed(false);
        }

        window.setTitle(&ns("YETI³ Cleaner · Статистика"));

        window.center();
        window.setOpaque(false);

        let background = statistics_color(0.025, 0.065, 0.095, 0.96);

        unsafe {
            let _: () = msg_send![
                &*window,
                setBackgroundColor: background
            ];
        }

        let Some(content) = window.contentView() else {
            return;
        };

        statistics_label(
            mtm,
            &content,
            "YETI³ CLEANER",
            NSRect::new(NSPoint::new(28.0, 635.0), NSSize::new(350.0, 28.0)),
            StatisticsTextStyle {
                size: 22.0,
                bold: true,
                secondary: false,
            },
        );

        statistics_label(
            mtm,
            &content,
            "СТАТИСТИКА · 30 ДНЕЙ",
            NSRect::new(NSPoint::new(28.0, 610.0), NSSize::new(350.0, 18.0)),
            StatisticsTextStyle {
                size: 10.5,
                bold: true,
                secondary: true,
            },
        );

        statistics_label(
            mtm,
            &content,
            "CODE. CLEAN. OPTIMIZE. REPEAT.",
            NSRect::new(NSPoint::new(525.0, 622.0), NSSize::new(320.0, 18.0)),
            StatisticsTextStyle {
                size: 10.0,
                bold: false,
                secondary: true,
            },
        );

        statistics_card(
            mtm,
            &content,
            "ВСЕГО ОЧИЩЕНО",
            &theme::human_bytes(snapshot.reclaimed_bytes),
            28.0,
            530.0,
            185.0,
        );

        statistics_card(
            mtm,
            &content,
            "ЗАПУСКОВ",
            &snapshot.runs.to_string(),
            235.0,
            530.0,
            145.0,
        );

        statistics_card(
            mtm,
            &content,
            "ФАЙЛОВ",
            &snapshot.files_deleted.to_string(),
            402.0,
            530.0,
            145.0,
        );

        statistics_card(
            mtm,
            &content,
            "КАТАЛОГОВ",
            &snapshot.dirs_deleted.to_string(),
            569.0,
            530.0,
            145.0,
        );

        statistics_card(
            mtm,
            &content,
            "ОШИБОК",
            &snapshot.errors.to_string(),
            736.0,
            530.0,
            110.0,
        );

        statistics_label(
            mtm,
            &content,
            "СВОБОДНОЕ МЕСТО",
            NSRect::new(NSPoint::new(28.0, 495.0), NSSize::new(300.0, 20.0)),
            StatisticsTextStyle {
                size: 12.0,
                bold: true,
                secondary: false,
            },
        );

        statistics_label(
            mtm,
            &content,
            &format!(
                "{}  →  {}",
                theme::human_bytes(snapshot.first_free_before),
                theme::human_bytes(snapshot.latest_free_after),
            ),
            NSRect::new(NSPoint::new(28.0, 466.0), NSSize::new(420.0, 26.0)),
            StatisticsTextStyle {
                size: 17.0,
                bold: true,
                secondary: false,
            },
        );

        statistics_label(
            mtm,
            &content,
            &format!(
                "Пропущено: {} · Ошибок: {}",
                snapshot.skipped, snapshot.errors
            ),
            NSRect::new(NSPoint::new(28.0, 443.0), NSSize::new(360.0, 18.0)),
            StatisticsTextStyle {
                size: 10.5,
                bold: false,
                secondary: true,
            },
        );

        statistics_label(
            mtm,
            &content,
            "ПОСЛЕДНИЕ ОЧИСТКИ",
            NSRect::new(NSPoint::new(28.0, 400.0), NSSize::new(300.0, 20.0)),
            StatisticsTextStyle {
                size: 12.0,
                bold: true,
                secondary: false,
            },
        );

        let max_run = snapshot
            .recent_runs
            .iter()
            .map(|run| run.reclaimed_bytes)
            .max()
            .unwrap_or(1);

        let mut y = 368.0;

        for run in snapshot.recent_runs.iter().take(6) {
            let delta = run.free_after.saturating_sub(run.free_before);

            statistics_label(
                mtm,
                &content,
                &format!(
                    "{} · {}",
                    theme::human_bytes(run.reclaimed_bytes),
                    if run.errors == 0 {
                        "OK"
                    } else {
                        "с замечаниями"
                    }
                ),
                NSRect::new(NSPoint::new(28.0, y), NSSize::new(260.0, 18.0)),
                StatisticsTextStyle {
                    size: 10.5,
                    bold: false,
                    secondary: false,
                },
            );

            statistics_progress(
                &content,
                292.0,
                y + 2.0,
                235.0,
                12.0,
                run.reclaimed_bytes as f64,
                max_run as f64,
            );

            statistics_label(
                mtm,
                &content,
                &format!("+{} свободно", theme::human_bytes(delta)),
                NSRect::new(NSPoint::new(540.0, y), NSSize::new(175.0, 18.0)),
                StatisticsTextStyle {
                    size: 10.0,
                    bold: false,
                    secondary: true,
                },
            );

            statistics_label(
                mtm,
                &content,
                &format!("{}", run.started_at),
                NSRect::new(NSPoint::new(720.0, y), NSSize::new(125.0, 18.0)),
                StatisticsTextStyle {
                    size: 9.5,
                    bold: false,
                    secondary: true,
                },
            );

            y -= 30.0;
        }

        statistics_label(
            mtm,
            &content,
            "ПО КАТЕГОРИЯМ",
            NSRect::new(NSPoint::new(28.0, 176.0), NSSize::new(300.0, 20.0)),
            StatisticsTextStyle {
                size: 12.0,
                bold: true,
                secondary: false,
            },
        );

        let max_category = snapshot
            .categories
            .iter()
            .map(|category| category.reclaimed_bytes)
            .max()
            .unwrap_or(1);

        let mut category_y = 145.0;

        for category in snapshot.categories.iter().take(5) {
            statistics_label(
                mtm,
                &content,
                &category.category,
                NSRect::new(NSPoint::new(28.0, category_y), NSSize::new(190.0, 18.0)),
                StatisticsTextStyle {
                    size: 10.5,
                    bold: false,
                    secondary: false,
                },
            );

            statistics_progress(
                &content,
                218.0,
                category_y + 2.0,
                370.0,
                12.0,
                category.reclaimed_bytes as f64,
                max_category as f64,
            );

            statistics_label(
                mtm,
                &content,
                &theme::human_bytes(category.reclaimed_bytes),
                NSRect::new(NSPoint::new(602.0, category_y), NSSize::new(125.0, 18.0)),
                StatisticsTextStyle {
                    size: 10.5,
                    bold: true,
                    secondary: false,
                },
            );

            statistics_label(
                mtm,
                &content,
                &format!("{} объектов", category.objects),
                NSRect::new(NSPoint::new(733.0, category_y), NSSize::new(115.0, 18.0)),
                StatisticsTextStyle {
                    size: 9.5,
                    bold: false,
                    secondary: true,
                },
            );

            category_y -= 28.0;
        }

        statistics_label(
            mtm,
            &content,
            "Данные хранятся локально · SQLite · YETI³ Cleaner",
            NSRect::new(NSPoint::new(28.0, 18.0), NSSize::new(520.0, 18.0)),
            StatisticsTextStyle {
                size: 9.5,
                bold: false,
                secondary: true,
            },
        );

        window.makeKeyAndOrderFront(None);

        let app = NSApplication::sharedApplication(mtm);

        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);

        *slot.borrow_mut() = Some(window);
    });
}

fn resource_path(name: &str) -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let macos = executable.parent()?;
    let contents = macos.parent()?;

    Some(contents.join("Resources").join(name))
}

fn load_resource_image(name: &str, template: bool) -> Option<Retained<NSImage>> {
    let path = resource_path(name)?;

    if !path.is_file() {
        return None;
    }

    let path_string = path.to_string_lossy();
    let path_ns = ns(&path_string);

    let image = NSImage::initWithContentsOfFile(NSImage::alloc(), &path_ns)?;

    image.setTemplate(template);

    Some(image)
}

fn show_launch_window(mtm: MainThreadMarker) -> Option<Retained<NSWindow>> {
    let image = load_resource_image("frame-00.png", false)?;

    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(420.0, 420.0));

    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            NSWindowStyleMask::Borderless,
            NSBackingStoreType::Buffered,
            false,
        )
    };

    unsafe {
        window.setReleasedWhenClosed(false);
    }

    window.setOpaque(false);
    window.setBackgroundColor(Some(&NSColor::clearColor()));
    window.setHasShadow(false);

    let image_view: Retained<NSImageView> = unsafe {
        msg_send![
            NSImageView::alloc(mtm),
            initWithFrame: frame
        ]
    };

    image_view.setImage(Some(&image));

    if let Some(content) = window.contentView() {
        content.addSubview(&image_view);
    }

    window.center();
    window.setAlphaValue(1.0);
    window.orderFrontRegardless();

    LAUNCH_IMAGE_VIEW.with(|slot| {
        *slot.borrow_mut() = Some(image_view);
    });

    Some(window)
}

fn cleaner_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join("yeti3-cleaner-engine");

            if bundled.is_file() {
                return bundled;
            }
        }
    }

    PathBuf::from("yeti3-cleaner")
}

fn make_label(
    mtm: MainThreadMarker,
    parent: &NSView,
    text: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) {
    let label = NSTextField::labelWithString(&ns(text), mtm);

    label.setFrame(NSRect::new(NSPoint::new(x, y), NSSize::new(width, height)));

    parent.addSubview(&label);
}

#[allow(clippy::too_many_arguments)]
fn make_checkbox(
    mtm: MainThreadMarker,
    parent: &NSView,
    controller: &Controller,
    title: &str,
    tag: isize,
    enabled: bool,
    x: f64,
    y: f64,
    width: f64,
) {
    let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(width, 22.0));

    let button: Retained<NSButton> = unsafe {
        msg_send![
            NSButton::alloc(mtm),
            initWithFrame: frame
        ]
    };

    unsafe {
        let _: () = msg_send![&*button, setButtonType: 3isize];
        let _: () = msg_send![&*button, setTitle: &*ns(title)];
        let _: () = msg_send![&*button, setTag: tag];
        let _: () = msg_send![
            &*button,
            setAction: Some(objc2::sel!(toggleSetting:))
        ];
        let _: () = msg_send![&*button, setTarget: controller as &AnyObject];
    }

    button.setState(if enabled {
        NSControlStateValueOn
    } else {
        NSControlStateValueOff
    });

    parent.addSubview(&button);
}

fn make_locked_checkbox(
    mtm: MainThreadMarker,
    parent: &NSView,
    title: &str,
    x: f64,
    y: f64,
    width: f64,
) {
    let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(width, 22.0));

    let button: Retained<NSButton> = unsafe {
        msg_send![
            NSButton::alloc(mtm),
            initWithFrame: frame
        ]
    };

    unsafe {
        let _: () = msg_send![&*button, setButtonType: 3isize];
        let _: () = msg_send![&*button, setTitle: &*ns(title)];
    }

    button.setState(NSControlStateValueOn);
    button.setEnabled(false);

    parent.addSubview(&button);
}

fn make_age_field(
    mtm: MainThreadMarker,
    parent: &NSView,
    controller: &Controller,
    value: u64,
    tag: isize,
    x: f64,
    y: f64,
) {
    make_label(mtm, parent, "старше", x, y + 2.0, 50.0, 20.0);

    let frame = NSRect::new(NSPoint::new(x + 52.0, y), NSSize::new(55.0, 24.0));

    let field: Retained<NSTextField> = unsafe {
        msg_send![
            NSTextField::alloc(mtm),
            initWithFrame: frame
        ]
    };

    field.setStringValue(&ns(&value.to_string()));

    unsafe {
        let _: () = msg_send![&*field, setTag: tag];
        let _: () = msg_send![
            &*field,
            setAction: Some(objc2::sel!(ageChanged:))
        ];
        let _: () = msg_send![&*field, setTarget: controller as &AnyObject];
    }

    parent.addSubview(&field);

    make_label(mtm, parent, "дней", x + 114.0, y + 2.0, 40.0, 20.0);
}

#[allow(clippy::too_many_arguments)]
fn make_action_button(
    mtm: MainThreadMarker,
    parent: &NSView,
    controller: &Controller,
    title: &str,
    selector: Sel,
    x: f64,
    y: f64,
    width: f64,
) {
    let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(width, 32.0));

    let button: Retained<NSButton> = unsafe {
        msg_send![
            NSButton::alloc(mtm),
            initWithFrame: frame
        ]
    };

    unsafe {
        let _: () = msg_send![&*button, setTitle: &*ns(title)];
        let _: () = msg_send![&*button, setBezelStyle: 1isize];
        let _: () = msg_send![&*button, setAction: Some(selector)];
        let _: () = msg_send![&*button, setTarget: controller as &AnyObject];
    }

    parent.addSubview(&button);
}

fn set_bool_setting(settings: &mut Settings, tag: isize, value: bool) {
    match tag {
        1 => settings.macos.trash = value,
        2 => settings.macos.application_caches = value,
        3 => settings.macos.temporary_files = value,
        4 => settings.macos.logs = value,
        5 => settings.macos.crash_reports = value,

        10 => settings.browsers.chrome = value,
        11 => settings.browsers.opera = value,
        12 => settings.browsers.firefox = value,
        13 => settings.browsers.chromium = value,
        14 => settings.browsers.brave = value,
        15 => settings.browsers.arc = value,

        20 => settings.development.xcode_derived_data = value,
        21 => settings.development.xcode_source_packages = value,
        22 => settings.development.unavailable_simulators = value,
        23 => settings.development.cargo = value,
        24 => settings.development.npm = value,
        25 => settings.development.npx = value,
        26 => settings.development.pnpm = value,
        27 => settings.development.yarn = value,
        28 => settings.development.pip = value,
        29 => settings.development.uv = value,
        30 => settings.development.gradle = value,
        31 => settings.development.cocoapods = value,

        40 => settings.ai_ml.huggingface = value,
        41 => settings.ai_ml.torch = value,
        42 => settings.ai_ml.whisper = value,
        43 => settings.ai_ml.clip = value,
        44 => settings.ai_ml.coreml = value,
        45 => settings.ai_ml.codex_runtimes = value,
        46 => settings.ai_ml.selenium = value,
        47 => settings.ai_ml.openai_python = value,

        50 => settings.homebrew.enabled = value,
        51 => settings.homebrew.autoremove = value,
        52 => settings.homebrew.old_versions = value,
        53 => settings.homebrew.cache = value,
        54 => settings.homebrew.temporary_builds = value,

        60 => settings.docker.enabled = value,
        61 => settings.docker.build_cache = value,
        62 => settings.docker.unused_images = value,
        63 => settings.docker.stopped_containers = value,
        64 => settings.docker.unused_networks = value,

        70 => settings.podman.enabled = value,
        71 => settings.podman.cache = value,
        72 => settings.podman.stale_machines = value,

        80 => settings.mobile.delete_all_local_backups = value,

        90 => settings.behavior.rescan_after_cleanup = value,
        91 => settings.behavior.show_reclaimed_space = value,
        92 => settings.behavior.write_log = value,

        _ => {}
    }
}

fn show_settings_window(controller: &Controller) {
    let mtm = controller.mtm();

    SETTINGS_WINDOW.with(|slot| {
        if let Some(window) = slot.borrow().as_ref() {
            window.makeKeyAndOrderFront(None);

            let app = NSApplication::sharedApplication(mtm);

            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);

            return;
        }

        let settings = config::load().unwrap_or_else(|_| Settings::default());

        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(720.0, 760.0)),
                NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Miniaturizable,
                NSBackingStoreType::Buffered,
                false,
            )
        };

        unsafe {
            window.setReleasedWhenClosed(false);
        }

        window.setTitle(&ns("Настройки Yeti3-Cleaner"));
        window.center();

        let content = window.contentView().expect("settings window content view");

        let scroll_frame = NSRect::new(NSPoint::new(0.0, 48.0), NSSize::new(720.0, 712.0));

        let scroll: Retained<NSScrollView> = unsafe {
            msg_send![
                NSScrollView::alloc(mtm),
                initWithFrame: scroll_frame
            ]
        };

        unsafe {
            let _: () = msg_send![&*scroll, setHasVerticalScroller: true];
            let _: () = msg_send![&*scroll, setAutohidesScrollers: true];
        }

        let document_frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(700.0, 1700.0));

        let document: Retained<NSView> = unsafe {
            msg_send![
                NSView::alloc(mtm),
                initWithFrame: document_frame
            ]
        };

        let mut y = 1650.0;

        make_label(mtm, &document, "macOS", 24.0, y, 250.0, 26.0);

        y -= 32.0;

        make_checkbox(
            mtm,
            &document,
            controller,
            "Корзина",
            1,
            settings.macos.trash,
            30.0,
            y,
            260.0,
        );

        y -= 28.0;

        make_checkbox(
            mtm,
            &document,
            controller,
            "Кэши приложений",
            2,
            settings.macos.application_caches,
            30.0,
            y,
            260.0,
        );

        y -= 28.0;

        make_checkbox(
            mtm,
            &document,
            controller,
            "Временные файлы",
            3,
            settings.macos.temporary_files,
            30.0,
            y,
            260.0,
        );

        make_age_field(
            mtm,
            &document,
            controller,
            settings.macos.temporary_min_age_days,
            1001,
            330.0,
            y,
        );

        y -= 28.0;

        make_checkbox(
            mtm,
            &document,
            controller,
            "Логи",
            4,
            settings.macos.logs,
            30.0,
            y,
            260.0,
        );

        make_age_field(
            mtm,
            &document,
            controller,
            settings.macos.logs_min_age_days,
            1002,
            330.0,
            y,
        );

        y -= 28.0;

        make_checkbox(
            mtm,
            &document,
            controller,
            "Crash Reports",
            5,
            settings.macos.crash_reports,
            30.0,
            y,
            260.0,
        );

        y -= 42.0;

        make_label(mtm, &document, "Браузеры", 24.0, y, 250.0, 26.0);

        y -= 32.0;

        let browser_rows = [
            ("Chrome", 10, settings.browsers.chrome),
            ("Opera", 11, settings.browsers.opera),
            ("Firefox", 12, settings.browsers.firefox),
            ("Chromium", 13, settings.browsers.chromium),
            ("Brave", 14, settings.browsers.brave),
            ("Arc", 15, settings.browsers.arc),
        ];

        for (i, (title, tag, value)) in browser_rows.iter().enumerate() {
            let col = i % 3;
            let row = i / 3;

            make_checkbox(
                mtm,
                &document,
                controller,
                title,
                *tag,
                *value,
                30.0 + col as f64 * 210.0,
                y - row as f64 * 28.0,
                190.0,
            );
        }

        y -= 86.0;

        make_label(mtm, &document, "Разработка", 24.0, y, 250.0, 26.0);

        y -= 32.0;

        make_checkbox(
            mtm,
            &document,
            controller,
            "Xcode DerivedData",
            20,
            settings.development.xcode_derived_data,
            30.0,
            y,
            280.0,
        );

        y -= 28.0;

        make_checkbox(
            mtm,
            &document,
            controller,
            "Xcode SourcePackages",
            21,
            settings.development.xcode_source_packages,
            30.0,
            y,
            280.0,
        );

        make_age_field(
            mtm,
            &document,
            controller,
            settings.development.xcode_source_packages_min_age_days,
            1003,
            330.0,
            y,
        );

        y -= 28.0;

        make_checkbox(
            mtm,
            &document,
            controller,
            "Недоступные Simulator",
            22,
            settings.development.unavailable_simulators,
            30.0,
            y,
            300.0,
        );

        y -= 32.0;

        let dev_rows = [
            ("Cargo", 23, settings.development.cargo),
            ("npm", 24, settings.development.npm),
            ("npx", 25, settings.development.npx),
            ("pnpm", 26, settings.development.pnpm),
            ("Yarn", 27, settings.development.yarn),
            ("pip", 28, settings.development.pip),
            ("uv", 29, settings.development.uv),
            ("Gradle", 30, settings.development.gradle),
            ("CocoaPods", 31, settings.development.cocoapods),
        ];

        for (i, (title, tag, value)) in dev_rows.iter().enumerate() {
            let col = i % 3;
            let row = i / 3;

            make_checkbox(
                mtm,
                &document,
                controller,
                title,
                *tag,
                *value,
                30.0 + col as f64 * 210.0,
                y - row as f64 * 28.0,
                185.0,
            );
        }

        make_age_field(
            mtm,
            &document,
            controller,
            settings.development.gradle_min_age_days,
            1004,
            520.0,
            y - 2.0 * 28.0,
        );

        y -= 120.0;

        make_label(mtm, &document, "AI / ML", 24.0, y, 250.0, 26.0);

        y -= 32.0;

        let ai_rows = [
            ("HuggingFace", 40, settings.ai_ml.huggingface),
            ("Torch", 41, settings.ai_ml.torch),
            ("Whisper", 42, settings.ai_ml.whisper),
            ("CLIP", 43, settings.ai_ml.clip),
            ("CoreML", 44, settings.ai_ml.coreml),
            ("Codex runtimes", 45, settings.ai_ml.codex_runtimes),
            ("Selenium", 46, settings.ai_ml.selenium),
            ("OpenAI cache", 47, settings.ai_ml.openai_python),
        ];

        for (i, (title, tag, value)) in ai_rows.iter().enumerate() {
            let col = i % 3;
            let row = i / 3;

            make_checkbox(
                mtm,
                &document,
                controller,
                title,
                *tag,
                *value,
                30.0 + col as f64 * 210.0,
                y - row as f64 * 28.0,
                190.0,
            );
        }

        y -= 112.0;

        make_label(mtm, &document, "Homebrew", 24.0, y, 250.0, 26.0);

        y -= 32.0;

        let brew_rows = [
            ("Включить Homebrew", 50, settings.homebrew.enabled),
            (
                "Неиспользуемые зависимости",
                51,
                settings.homebrew.autoremove,
            ),
            ("Старые версии", 52, settings.homebrew.old_versions),
            ("Cache / Downloads", 53, settings.homebrew.cache),
            ("Временные сборки", 54, settings.homebrew.temporary_builds),
        ];

        for (title, tag, value) in brew_rows {
            make_checkbox(
                mtm, &document, controller, title, tag, value, 30.0, y, 360.0,
            );

            y -= 28.0;
        }

        y -= 18.0;

        make_label(mtm, &document, "Docker", 24.0, y, 250.0, 26.0);

        y -= 32.0;

        let docker_rows = [
            ("Включить Docker", 60, settings.docker.enabled),
            ("Build cache", 61, settings.docker.build_cache),
            ("Неиспользуемые images", 62, settings.docker.unused_images),
            (
                "Остановленные containers",
                63,
                settings.docker.stopped_containers,
            ),
            (
                "Неиспользуемые networks",
                64,
                settings.docker.unused_networks,
            ),
        ];

        for (title, tag, value) in docker_rows {
            make_checkbox(
                mtm, &document, controller, title, tag, value, 30.0, y, 360.0,
            );

            y -= 28.0;
        }

        make_locked_checkbox(mtm, &document, "Volumes — защищены всегда", 30.0, y, 360.0);

        y -= 48.0;

        make_label(mtm, &document, "Podman", 24.0, y, 250.0, 26.0);

        y -= 32.0;

        make_checkbox(
            mtm,
            &document,
            controller,
            "Включить Podman",
            70,
            settings.podman.enabled,
            30.0,
            y,
            300.0,
        );

        y -= 28.0;

        make_checkbox(
            mtm,
            &document,
            controller,
            "Cache",
            71,
            settings.podman.cache,
            30.0,
            y,
            300.0,
        );

        y -= 28.0;

        make_checkbox(
            mtm,
            &document,
            controller,
            "Неиспользуемые VM",
            72,
            settings.podman.stale_machines,
            30.0,
            y,
            300.0,
        );

        make_age_field(
            mtm,
            &document,
            controller,
            settings.podman.stale_machine_days,
            1005,
            330.0,
            y,
        );

        y -= 48.0;

        make_label(mtm, &document, "iPhone / iPad", 24.0, y, 250.0, 26.0);

        y -= 32.0;

        make_checkbox(
            mtm,
            &document,
            controller,
            "Удалять все локальные резервные копии",
            80,
            settings.mobile.delete_all_local_backups,
            30.0,
            y,
            430.0,
        );

        y -= 48.0;

        make_label(mtm, &document, "После очистки", 24.0, y, 250.0, 26.0);

        y -= 32.0;

        make_checkbox(
            mtm,
            &document,
            controller,
            "Повторно проверить диск",
            90,
            settings.behavior.rescan_after_cleanup,
            30.0,
            y,
            350.0,
        );

        y -= 28.0;

        make_checkbox(
            mtm,
            &document,
            controller,
            "Показывать освобождённое место",
            91,
            settings.behavior.show_reclaimed_space,
            30.0,
            y,
            390.0,
        );

        y -= 28.0;

        make_checkbox(
            mtm,
            &document,
            controller,
            "Вести журнал",
            92,
            settings.behavior.write_log,
            30.0,
            y,
            350.0,
        );

        y -= 52.0;

        make_label(mtm, &document, "Всегда защищено", 24.0, y, 250.0, 26.0);

        y -= 30.0;

        make_label(
            mtm,
            &document,
            "Downloads · Documents · GIT · Desktop · Pictures · Movies · Music",
            30.0,
            y,
            640.0,
            22.0,
        );

        y -= 24.0;

        make_label(
            mtm,
            &document,
            "iCloud · Mail · Messages · Telegram · WhatsApp · Docker volumes",
            30.0,
            y,
            640.0,
            22.0,
        );

        unsafe {
            let _: () = msg_send![&*scroll, setDocumentView: &*document];
            content.addSubview(&scroll);
        }

        make_action_button(
            mtm,
            &content,
            controller,
            "По умолчанию",
            objc2::sel!(resetDefaults:),
            18.0,
            8.0,
            130.0,
        );

        make_action_button(
            mtm,
            &content,
            controller,
            "Закрыть",
            objc2::sel!(closeSettings:),
            570.0,
            8.0,
            130.0,
        );

        window.makeKeyAndOrderFront(None);

        let app = NSApplication::sharedApplication(mtm);

        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);

        *slot.borrow_mut() = Some(window);
    });
}

fn main() {
    let _design_system = (
        theme::METRICS.signature(),
        theme::GLASS.components(),
        theme::GLASS_CARD.components(),
        theme::ICE.components(),
        theme::ICE_SOFT.components(),
        theme::TEXT_PRIMARY.components(),
        theme::TEXT_SECONDARY.components(),
        theme::SUCCESS.components(),
        theme::WARNING.components(),
        theme::ERROR.components(),
        about::PRODUCT,
        about::SUBTITLE,
        about::MOTTO,
        about::STACK,
        about::version(),
        launcher::FRAME_COUNT,
        launcher::FRAME_INTERVAL_SECONDS,
        launcher::frame_name(0),
        TrayState::Idle.resource(),
        TrayState::Cleaning.resource(),
        TrayState::Paused.resource(),
        TrayState::Complete.resource(),
        TrayState::Error.resource(),
        settings_ui::Section::General.title(),
        settings_ui::Section::Cleanup.title(),
        settings_ui::Section::Development.title(),
        settings_ui::Section::AiMl.title(),
        settings_ui::Section::Containers.title(),
        settings_ui::Section::Protection.title(),
        statistics::Period::SevenDays.title(),
        statistics::Period::ThirtyDays.title(),
        statistics::Period::NinetyDays.title(),
        statistics::Period::AllTime.title(),
        result::TITLE_CLEAN,
        result::TITLE_WITH_NOTES,
        theme::human_bytes(0),
        theme::human_duration(0),
    );

    let mtm = MainThreadMarker::new().expect("Yeti3-Cleaner must run on main thread");

    let app = NSApplication::sharedApplication(mtm);

    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let controller = Controller::new(mtm);

    if let Some(window) = show_launch_window(mtm) {
        LAUNCH_WINDOW.with(|slot| {
            *slot.borrow_mut() = Some(window);
        });
    }

    let status_bar = NSStatusBar::systemStatusBar();

    let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);

    if let Some(button) = status_item.button(mtm) {
        button.setTitle(&ns(""));
        button.setToolTip(Some(&ns(&format!("{PRODUCT_NAME} · {PRODUCT_TAGLINE}"))));

        *controller.ivars().status_button.borrow_mut() = Some(button.clone());

        controller.set_status_image("idle.png");
    }

    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &ns("Yeti3-Cleaner"));

    let start = menu_item(mtm, "Старт", objc2::sel!(startPause:), &controller);

    menu.addItem(&start);

    let stop = menu_item(mtm, "Стоп", objc2::sel!(stop:), &controller);

    stop.setEnabled(false);
    menu.addItem(&stop);

    *controller.ivars().start_item.borrow_mut() = Some(start.clone());

    *controller.ivars().stop_item.borrow_mut() = Some(stop.clone());

    menu.addItem(&NSMenuItem::separatorItem(mtm));

    let statistics_item = menu_item(
        mtm,
        "Статистика…",
        objc2::sel!(openStatistics:),
        &controller,
    );

    menu.addItem(&statistics_item);

    let settings = menu_item(mtm, "Настройки…", objc2::sel!(openSettings:), &controller);

    menu.addItem(&settings);

    menu.addItem(&NSMenuItem::separatorItem(mtm));

    let about = menu_item(mtm, "О программе", objc2::sel!(about:), &controller);

    menu.addItem(&about);

    let quit = menu_item(mtm, "Выход", objc2::sel!(quit:), &controller);

    menu.addItem(&quit);

    status_item.setMenu(Some(&menu));

    let _: () = unsafe {
        msg_send![
            class!(NSTimer),
            scheduledTimerWithTimeInterval: 0.32f64,
            target: &*controller as &AnyObject,
            selector: objc2::sel!(animationTick:),
            userInfo: std::ptr::null::<AnyObject>(),
            repeats: true
        ]
    };

    let _: () = unsafe {
        msg_send![
            class!(NSTimer),
            scheduledTimerWithTimeInterval: 0.055f64,
            target: &*controller as &AnyObject,
            selector: objc2::sel!(launchTick:),
            userInfo: std::ptr::null::<AnyObject>(),
            repeats: true
        ]
    };

    let _keep_alive = (controller, status_item, menu);

    app.run();
}
