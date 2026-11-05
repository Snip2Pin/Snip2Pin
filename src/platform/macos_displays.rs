//! Event-driven display invalidation; no window queries or mutations in callbacks.
use std::ptr::NonNull;

use block2::RcBlock;
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::NSApplicationDidChangeScreenParametersNotification;
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObjectProtocol};
use snip2pin::i18n::{Message, Text};

pub struct DisplayChanges {
    center: Retained<NSNotificationCenter>,
    observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

impl DisplayChanges {
    pub fn install(notify: impl Fn() + Send + Sync + 'static) -> Result<Self, Message> {
        Self::observe(NSNotificationCenter::defaultCenter(), notify)
    }

    #[allow(unsafe_code)]
    pub(crate) fn observe(
        center: Retained<NSNotificationCenter>,
        notify: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self, Message> {
        let _thread = objc2::MainThreadMarker::new().ok_or(Text::UiThread)?;
        let callback = RcBlock::new(move |_: NonNull<NSNotification>| notify());
        // SAFETY: The framework notification name is immutable. No object or
        // operation queue is supplied. The copied block owns only Send + Sync
        // state; delivery merely invalidates state and wakes the UI.
        let observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSApplicationDidChangeScreenParametersNotification),
                None,
                None,
                &callback,
            )
        };
        Ok(Self { center, observer })
    }
}

impl Drop for DisplayChanges {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        let observer: &objc2::runtime::AnyObject = self.observer.as_ref();
        // SAFETY: This is the retained token returned by this center. Removing
        // it releases the callback; in-flight copies own their captured state.
        unsafe { self.center.removeObserver(observer) };
    }
}
