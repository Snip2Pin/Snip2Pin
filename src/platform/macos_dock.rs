//! A nib connects the Dock menu without replacing winit's application delegate.
#![allow(unsafe_code)]

use std::cell::RefCell;

use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, Sel},
    sel,
};
use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem, NSNib};
use objc2_foundation::{
    NSAppleEventDescriptor, NSAppleEventManager, NSAppleEventManagerSuspensionID, NSArray,
    NSBundle, NSObject, NSObjectProtocol, NSString,
};
use snip2pin::i18n::{Locale, Message, Text};

use crate::tray::Action;

const NIB_NAME: &str = "DockMenu";
// Stable resource identities; titles are translated only after loading.
const HOME_TAG: isize = 1;
const CAPTURE_TAG: isize = 2;
const CLOSE_PINS_TAG: isize = 3;
// Public AppleEvents.h / AEDataModel.h wire identifiers.
const CORE_EVENT_CLASS: u32 = u32::from_be_bytes(*b"aevt");
const QUIT_APPLICATION: u32 = u32::from_be_bytes(*b"quit");
const REOPEN_APPLICATION: u32 = u32::from_be_bytes(*b"rapp");
const ERROR_NUMBER: u32 = u32::from_be_bytes(*b"errn");
const NULL_DESCRIPTOR: u32 = u32::from_be_bytes(*b"null");
const USER_CANCELED: i32 = -128;
const EVENT_FAILED: i32 = -10000;

type Callback = Box<dyn Fn(Action) + Send + Sync>;

struct TargetState {
    callback: Callback,
    // These are outstanding system requests, not a second confirmation state.
    pending_quits: RefCell<Vec<NSAppleEventManagerSuspensionID>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements. This owns only its
    // callback; AppKit/window backend classes and delegates are untouched.
    #[unsafe(super(NSObject))]
    #[name = "Snip2PinDockMenuTarget"]
    #[thread_kind = MainThreadOnly]
    #[ivars = TargetState]
    struct DockTarget;

    // SAFETY: NSObjectProtocol imposes no additional requirements.
    unsafe impl NSObjectProtocol for DockTarget {}

    impl DockTarget {
        // Dock actions can arrive with a nil sender, so routing must not depend
        // on inspecting an NSMenuItem or its translated title in the callback.
        #[unsafe(method(snip2pinOpenHome:))]
        fn open_home(&self, _sender: Option<&AnyObject>) {
            (self.ivars().callback)(Action::Home);
        }

        #[unsafe(method(snip2pinCapture:))]
        fn capture(&self, _sender: Option<&AnyObject>) {
            (self.ivars().callback)(Action::Capture);
        }

        #[unsafe(method(snip2pinClosePins:))]
        fn close_pins(&self, _sender: Option<&AnyObject>) {
            (self.ivars().callback)(Action::ClosePins);
        }

        #[unsafe(method(snip2pinQuit:))]
        fn quit(&self, _sender: Option<&AnyObject>) {
            (self.ivars().callback)(Action::Quit);
        }

        #[unsafe(method(snip2pinReopenEvent:withReplyEvent:))]
        fn reopen_event(&self, _event: &NSAppleEventDescriptor, _reply: &NSAppleEventDescriptor) {
            (self.ivars().callback)(Action::Home);
        }

        // NSAppleEventManager's documented two-descriptor callback ABI.
        #[unsafe(method(snip2pinQuitEvent:withReplyEvent:))]
        fn quit_event(&self, _event: &NSAppleEventDescriptor, reply: &NSAppleEventDescriptor) {
            let manager = NSAppleEventManager::sharedAppleEventManager();
            let suspension = manager.suspendCurrentAppleEvent();
            if suspension.is_null() {
                // A live AppleEvent handler normally always has a current event.
                // If suspension fails, report failure without authorizing a quit
                // or claiming a successful asynchronous request to the sender.
                set_reply_error(reply, EVENT_FAILED);
                return;
            }
            self.ivars().pending_quits.borrow_mut().push(suspension);
            (self.ivars().callback)(Action::Quit);
        }
    }
);

impl DockTarget {
    fn new(main: MainThreadMarker, callback: Callback) -> Retained<Self> {
        let target = Self::alloc(main).set_ivars(TargetState {
            callback,
            pending_quits: RefCell::new(Vec::new()),
        });
        // SAFETY: NSObject initialization of our own allocated class and ivars.
        unsafe { msg_send![super(target), init] }
    }
}

struct QuitItem {
    item: Retained<NSMenuItem>,
    original_target: Option<Retained<AnyObject>>,
    original_action: Option<Sel>,
}

pub struct Dock {
    target: Retained<DockTarget>,
    event_manager: Retained<NSAppleEventManager>,
    quit_items: Vec<QuitItem>,
    menu: Option<DockMenu>,
}

struct DockMenu {
    // Retain the nib's top-level menu independently of AppKit's outlet.
    _menu: Retained<NSMenu>,
    home: Retained<NSMenuItem>,
    capture: Retained<NSMenuItem>,
    close_pins: Retained<NSMenuItem>,
    state: Option<(Locale, bool, bool, bool)>,
}

impl Dock {
    /// Install once on the main thread after eframe initializes AppKit and its
    /// main menu. Keep alive until the application event loop ends; hot removal
    /// or replacement of this adapter or the application menu is unsupported.
    /// Skip this in the sandbox. The callback must enqueue an application command
    /// and wake its UI, not terminate immediately.
    /// Every Quit command must eventually call resolve_quit before cancellation
    /// or application shutdown, even if no confirmation dialog was needed.
    pub fn new(callback: impl Fn(Action) + Send + Sync + 'static) -> Result<Self, Message> {
        let main = MainThreadMarker::new().ok_or(Text::UiThread)?;
        let application = NSApplication::sharedApplication(main);
        let main_menu = application.mainMenu().ok_or(Text::DockQuitUnavailable)?;
        let mut quit_items = Vec::new();
        collect_quit_items(&main_menu, &mut quit_items);
        if quit_items.is_empty() {
            return Err(Text::DockQuitUnavailable.into());
        }
        let target = DockTarget::new(main, Box::new(callback));
        for quit in &quit_items {
            // Preserve the backend's existing title, key equivalent and modifiers.
            bind(&quit.item, &target, sel!(snip2pinQuit:));
        }
        let event_manager = NSAppleEventManager::sharedAppleEventManager();
        // SAFETY: AppKit is initialized on the main thread. This target implements
        // the documented event/reply descriptor ABI and lives until removal.
        // Only the process's Quit and Reopen AppleEvents are claimed; delegates,
        // window classes and unrelated event handlers remain unchanged.
        unsafe {
            event_manager.setEventHandler_andSelector_forEventClass_andEventID(
                &target,
                sel!(snip2pinQuitEvent:withReplyEvent:),
                CORE_EVENT_CLASS,
                QUIT_APPLICATION,
            );
            event_manager.setEventHandler_andSelector_forEventClass_andEventID(
                &target,
                sel!(snip2pinReopenEvent:withReplyEvent:),
                CORE_EVENT_CLASS,
                REOPEN_APPLICATION,
            );
        }
        Ok(Self {
            target,
            event_manager,
            quit_items,
            menu: None,
        })
    }

    /// Load the optional custom Dock menu from the signed application bundle.
    /// Failure leaves Quit and Reopen connected. Items remain disabled until the
    /// caller's next update supplies current labels and enabled state.
    pub fn load_menu(&mut self) -> Result<(), Message> {
        if self.menu.is_some() {
            return Ok(());
        }
        let main = MainThreadMarker::new().ok_or(Text::UiThread)?;
        let bundle = NSBundle::mainBundle();
        let name = NSString::from_str(NIB_NAME);
        if bundle
            .pathForResource_ofType(Some(&name), Some(&NSString::from_str("nib")))
            .is_none()
        {
            return Err(Text::DockMenuMissing.into());
        }
        let nib = NSNib::initWithNibNamed_bundle(NSNib::alloc(), &name, Some(&bundle))
            .ok_or(Text::DockMenuLoadFailed)?;
        let objects = instantiate(&nib, &NSApplication::sharedApplication(main))?;
        let menu = objects
            .to_vec()
            .into_iter()
            .find_map(|object| object.downcast::<NSMenu>().ok())
            .ok_or(Text::DockMenuLoadFailed)?;
        let home = menu.itemWithTag(HOME_TAG).ok_or(Text::DockMenuLoadFailed)?;
        let capture = menu
            .itemWithTag(CAPTURE_TAG)
            .ok_or(Text::DockMenuLoadFailed)?;
        let close_pins = menu
            .itemWithTag(CLOSE_PINS_TAG)
            .ok_or(Text::DockMenuLoadFailed)?;
        menu.setAutoenablesItems(false);
        bind(&home, &self.target, sel!(snip2pinOpenHome:));
        bind(&capture, &self.target, sel!(snip2pinCapture:));
        bind(&close_pins, &self.target, sel!(snip2pinClosePins:));
        self.menu = Some(DockMenu {
            _menu: menu,
            home,
            capture,
            close_pins,
            state: None,
        });
        Ok(())
    }

    /// Complete all native Quit requests after the application's one decision.
    /// On confirmation, the caller must then perform normal app cleanup and exit.
    /// On cancellation, pending senders receive userCanceledErr, including a
    /// logout/shutdown requester. Nothing is reported successful before this call.
    pub fn resolve_quit(&self, confirmed: bool) {
        // Resume may deliver further native messages: release the RefCell borrow
        // before entering AppKit, and consume each suspension ID exactly once.
        let pending = std::mem::take(&mut *self.target.ivars().pending_quits.borrow_mut());
        for suspension in pending {
            if !confirmed {
                // SAFETY: This token came from suspendCurrentAppleEvent and has
                // not been resumed. NSAppleEventManager forbids retaining its
                // reply descriptor, so borrow the raw result only until resume.
                let reply: *mut NSAppleEventDescriptor = unsafe {
                    msg_send![&*self.event_manager, replyAppleEventForSuspensionID: suspension]
                };
                // SAFETY: For a live suspension ID the API guarantees a non-null
                // descriptor owned by the suspended event until resume.
                if let Some(reply) = unsafe { reply.as_ref() } {
                    set_reply_error(reply, USER_CANCELED);
                }
            }
            // SAFETY: The ID is live and uniquely consumed by this loop.
            unsafe { self.event_manager.resumeWithSuspensionID(suspension) };
        }
    }

    /// Called with the same event-driven application state as the tray menu.
    pub fn update(
        &mut self,
        locale: Locale,
        capture_enabled: bool,
        has_unlocked_pins: bool,
        capturing: bool,
    ) {
        let Some(menu) = &mut self.menu else {
            return;
        };
        let state = (locale, capture_enabled, has_unlocked_pins, capturing);
        if menu.state == Some(state) {
            return;
        }
        menu.home
            .setTitle(&NSString::from_str(locale.text(Text::Home)));
        menu.home.setEnabled(true);
        menu.capture
            .setTitle(&NSString::from_str(locale.text(if capturing {
                Text::CancelCapture
            } else {
                Text::Capture
            })));
        menu.capture.setEnabled(capture_enabled);
        menu.close_pins
            .setTitle(&NSString::from_str(locale.text(Text::ClosePins)));
        menu.close_pins.setEnabled(has_unlocked_pins);
        menu.state = Some(state);
    }
}

fn bind(item: &NSMenuItem, target: &DockTarget, action: Sel) {
    // SAFETY: All menu selectors are declared above with AppKit's action ABI.
    // Dock retains target and clears these links before releasing it.
    unsafe {
        item.setTarget(Some(target));
        item.setAction(Some(action));
    }
}

fn collect_quit_items(menu: &NSMenu, found: &mut Vec<QuitItem>) {
    for item in menu.itemArray().to_vec() {
        if item.action() == Some(sel!(terminate:)) {
            found.push(QuitItem {
                original_target: item.target(),
                original_action: item.action(),
                item: item.clone(),
            });
        }
        if let Some(submenu) = item.submenu() {
            collect_quit_items(&submenu, found);
        }
    }
}

fn set_reply_error(reply: &NSAppleEventDescriptor, error: i32) {
    // A null reply means the sender requested no response; do not modify it.
    if reply.descriptorType() != NULL_DESCRIPTOR {
        reply.setParamDescriptor_forKeyword(
            &NSAppleEventDescriptor::descriptorWithInt32(error),
            ERROR_NUMBER,
        );
    }
}

fn instantiate(nib: &NSNib, owner: &NSApplication) -> Result<Retained<NSArray>, Message> {
    let mut objects: *mut NSArray = std::ptr::null_mut();
    // SAFETY: This is the public macOS 10.8+ API, omitted by objc2-app-kit
    // because of its ownership annotation. NSNib.h specifies autoreleased
    // top-level objects. Pass a writable raw out-pointer and retain its result
    // before the enclosing autorelease pool can drain. The trusted bundled nib
    // connects only its menu to NSApplication's documented dockMenu outlet.
    let loaded: bool =
        unsafe { msg_send![nib, instantiateWithOwner: owner, topLevelObjects: &mut objects] };
    if !loaded {
        return Err(Text::DockMenuLoadFailed.into());
    }
    // SAFETY: On success, the API writes either nil or a live NSArray pointer.
    unsafe { Retained::retain(objects) }.ok_or_else(|| Text::DockMenuLoadFailed.into())
}

impl Drop for Dock {
    fn drop(&mut self) {
        // This owner lasts until event-loop teardown. Do not leave pending system
        // requests unanswered or allow a new one to target a released callback.
        self.event_manager
            .removeEventHandlerForEventClass_andEventID(CORE_EVENT_CLASS, QUIT_APPLICATION);
        self.event_manager
            .removeEventHandlerForEventClass_andEventID(CORE_EVENT_CLASS, REOPEN_APPLICATION);
        self.resolve_quit(false);
        for quit in &self.quit_items {
            // SAFETY: Restore the exact valid target/action pair captured before
            // installation; original_target remains retained through this call.
            unsafe {
                quit.item.setAction(quit.original_action);
                quit.item.setTarget(quit.original_target.as_deref());
            }
        }
        // NSApplication may retain the connected menu beyond this owner. Leave
        // no callbacks pointing at a released target during application teardown.
        if let Some(menu) = &self.menu {
            for item in [&menu.home, &menu.capture, &menu.close_pins] {
                item.setEnabled(false);
                // SAFETY: Removing target/action links is valid on the main thread;
                // the MainThreadOnly target prevents Dock from moving off-thread.
                unsafe {
                    item.setAction(None);
                    item.setTarget(None);
                }
            }
        }
    }
}
