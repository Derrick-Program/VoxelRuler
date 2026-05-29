use std::sync::OnceLock;

use objc2::define_class;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AllocAnyThread, msg_send, sel};
use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager, NSObject};
use tokio::sync::mpsc;
use tracing::debug;

const INTERNET_EVENT_CLASS: u32 = u32::from_be_bytes(*b"GURL");
const AE_GET_URL: u32 = u32::from_be_bytes(*b"GURL");
const KEY_DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");

static URL_SENDER: OnceLock<mpsc::UnboundedSender<String>> = OnceLock::new();

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "VRURLHandler"]
    pub struct VRURLHandler;
    impl VRURLHandler {
        #[unsafe(method(handleGetURLEvent:withReplyEvent:))]
        fn handle_url_event(
            &self,
            event: &NSAppleEventDescriptor,
            _reply_event: &NSAppleEventDescriptor,
        ) {
            let url_string: Option<String> = unsafe {
                let raw_desc: *mut NSAppleEventDescriptor =
                    msg_send![event, paramDescriptorForKeyword: KEY_DIRECT_OBJECT];
                let retained_desc: Option<Retained<NSAppleEventDescriptor>> =
                    Retained::retain(raw_desc);
                retained_desc
                    .as_deref()
                    .and_then(|desc| desc.stringValue())
                    .map(|ns_str| ns_str.to_string())
            };

            match url_string {
                Some(url) => {
                    debug!(url = %url, "Apple Event 收到 URL scheme");
                    if let Some(tx) = URL_SENDER.get() && tx.send(url).is_err() {
                            debug!("URL channel receiver 已關閉，略過此 event");
                        }
                }
                None => {
                    debug!("Apple Event 的 URL descriptor 為空或無法取得字串");
                }
            }
        }
    }
);

static HANDLER: OnceLock<Retained<VRURLHandler>> = OnceLock::new();

pub fn register() -> mpsc::UnboundedReceiver<String> {
    let (tx, rx) = mpsc::unbounded_channel::<String>();
    if URL_SENDER.set(tx).is_err() {
        debug!("url_handler::register() 被重複呼叫，略過重複初始化");
        return rx;
    }
    let handler: Retained<VRURLHandler> = unsafe {
        let alloc = VRURLHandler::alloc().set_ivars(());
        msg_send![super(alloc), init]
    };
    HANDLER.set(handler).ok();
    unsafe {
        let manager: Retained<NSAppleEventManager> = NSAppleEventManager::sharedAppleEventManager();

        let handler_obj: &AnyObject = HANDLER.get().unwrap().as_ref();
        let selector = sel!(handleGetURLEvent:withReplyEvent:);

        let _: () = msg_send![
            &*manager,
            setEventHandler: handler_obj,
            andSelector: selector,
            forEventClass: INTERNET_EVENT_CLASS,
            andEventID: AE_GET_URL
        ];
    }

    debug!(
        event_class = INTERNET_EVENT_CLASS,
        event_id = AE_GET_URL,
        "URL scheme handler 已向 NSAppleEventManager 註冊"
    );
    rx
}
