//! Objective-C bridge of Madoka Launcher for macOS. The crate is empty on other systems.
//!
//! macOS delivers a URL of a registered scheme as the Apple Event `kAEGetURL`, never as an
//! argument. [`install_url_handler`] receives that event and passes the URL on.
#![cfg(target_os = "macos")]

use std::sync::OnceLock;

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager, NSObject};

/// Four-character code `GURL`: the event class `kInternetEventClass` and the event ID `kAEGetURL`.
const GET_URL: u32 = u32::from_be_bytes(*b"GURL");
/// Four-character code `----`: the parameter `keyDirectObject`, which holds the URL.
const DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");

/// Receiver of the URLs. Set once, before the handler is registered.
static CALLBACK: OnceLock<fn(&str)> = OnceLock::new();

define_class!(
    /// Target of the Apple Event handler. It has no instance variables.
    ///
    /// # Safety
    ///
    /// `NSObject` has no subclassing requirements. The class adds no instance variables
    /// and no `Drop`, and it is main-thread-only, as `NSAppleEventManager` requires.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "MadokaUrlHandler"]
    struct UrlHandler;

    impl UrlHandler {
        /// # Safety
        ///
        /// The selector `handleURL:withReplyEvent:` takes two object arguments and returns
        /// nothing, which is the signature of this method. `NSAppleEventManager` passes two
        /// valid `NSAppleEventDescriptor` references that outlive the call.
        #[unsafe(method(handleURL:withReplyEvent:))]
        fn handle_url(&self, event: &NSAppleEventDescriptor, _reply: &NSAppleEventDescriptor) {
            let url = event
                .paramDescriptorForKeyword(DIRECT_OBJECT)
                .and_then(|descriptor| descriptor.stringValue());
            if let (Some(url), Some(callback)) = (url, CALLBACK.get()) {
                callback(&url.to_string());
            }
        }
    }
);

impl UrlHandler {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(());
        // SAFETY: `this` is a fresh allocation whose instance variables are set (the class has
        // none), and `init` of the superclass `NSObject` takes no arguments and returns `self`.
        unsafe { msg_send![super(this), init] }
    }
}

/// Passes every URL that macOS opens in this process to `callback`, on the main thread.
///
/// Call it once on the main thread, before the event loop starts.
/// Returns `false` when called off the main thread or a second time.
#[must_use]
pub fn install_url_handler(callback: fn(&str)) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    if CALLBACK.set(callback).is_err() {
        return false;
    }
    let handler = UrlHandler::new(mtm);
    // SAFETY: the selector exists on `UrlHandler` with the signature that the manager calls
    // (see `handle_url`). The manager does not retain `handler`: the `forget` below keeps one
    // strong reference for the lifetime of the process, so the target never dangles.
    unsafe {
        NSAppleEventManager::sharedAppleEventManager()
            .setEventHandler_andSelector_forEventClass_andEventID(
                &handler,
                sel!(handleURL:withReplyEvent:),
                GET_URL,
                GET_URL,
            );
    }
    std::mem::forget(handler);
    true
}
