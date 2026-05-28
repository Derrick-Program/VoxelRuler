/// macOS URL Scheme Handler — objc2 0.6.4 + objc2-foundation 0.3.0
///
/// # 為什麼要這樣做
/// macOS custom URL scheme (`voxelruler://`) 不走 argv，
/// 而是透過 Apple Events（kInternetEventClass / kAEGetURL）傳遞。
/// 必須向 NSAppleEventManager 註冊一個 ObjC handler 物件才能收到。
///
/// # 使用方式
/// 在 `open_view()` 之前（main thread）呼叫 `register()`，
/// 取得 receiver 後在 tokio task 裡監聽：
/// ```rust
/// let mut rx = url_handler::register();
/// tokio::spawn(async move {
///     while let Some(url) = rx.recv().await { /* ... */ }
/// });
/// ```
// ── 版本資訊 ──────────────────────────────────────────────────────────────
// objc2            = "0.6.4"
// objc2-foundation = "0.3.0"  (依賴 objc2 ^0.6.0)
//
// ── Binding 缺口（需要 msg_send! 補）────────────────────────────────────
// NSAppleEventManager::setEventHandler:andSelector:forEventClass:andEventID:
//   → 不在 0.3.0 的 bindings 裡，用 msg_send!
// NSAppleEventDescriptor::paramDescriptorForKeyword:
//   → 不在 0.3.0 的 bindings 裡，用 msg_send!
// NSAppleEventDescriptor::stringValue
//   → ✅ 有 binding，回傳 Option<Retained<NSString>>
use std::sync::OnceLock;

use objc2::define_class;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AllocAnyThread, DefinedClass, msg_send, sel};
use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager, NSObject};
use tokio::sync::mpsc;
use tracing::debug;

// ── Apple Event FourCharCode 常數 ──────────────────────────────────────────
// big-endian 4-byte code，用 b"XXXX" 轉換
// kInternetEventClass = 'GURL' = 0x4755524C
// kAEGetURL           = 'GURL' = 0x4755524C （class 和 event ID 相同，Apple 規格如此）
// keyDirectObject     = '----' = 0x2D2D2D2D （event 主要參數的 key）
const INTERNET_EVENT_CLASS: u32 = u32::from_be_bytes(*b"GURL");
const AE_GET_URL: u32 = u32::from_be_bytes(*b"GURL");
const KEY_DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");

/// 全域 channel sender，由 ObjC handler 方法（主執行緒）送出 URL
static URL_SENDER: OnceLock<mpsc::UnboundedSender<String>> = OnceLock::new();

// ── Objective-C Handler 類別定義 ───────────────────────────────────────────
//
// define_class! 在 compile time 向 ObjC runtime 註冊新 class `VRURLHandler`。
// 繼承自 NSObject，新增 selector `handleGetURLEvent:withReplyEvent:`。
//
// NSAppleEventManager 持有 handler 的 weak reference，
// 因此 handler 物件必須在 app 生命週期內保持 alive（見 HANDLER static）。
//
// 自訂 class 繼承自 NSObject 且無 ivars，
// 根據 objc2 文件其 auto traits 為 Send + Sync ✓
define_class!(
    // SAFETY:
    // - NSObject 沒有額外的 subclass 限制。
    // - VRURLHandler 不實作 Drop，不需要特別處理 dealloc。
    #[unsafe(super(NSObject))]
    // 不設 #[thread_kind = MainThreadOnly]，讓它可以安全地存在 OnceLock 靜態中
    #[name = "VRURLHandler"]
    pub struct VRURLHandler;

    impl VRURLHandler {
        /// NSAppleEventManager 在收到 URL event 時呼叫此 selector。
        ///
        /// - `event`：Apple Event descriptor，URL 字串在 keyDirectObject 參數裡
        /// - `_reply_event`：同步呼叫的回覆用（URL scheme 屬非同步，無需處理）
        ///
        /// SAFETY: 型別簽名必須精確對應 ObjC selector 的實際型別，否則 UB。
        /// `NSAppleEventDescriptor *` 在兩個參數位置上正確對應。
        #[unsafe(method(handleGetURLEvent:withReplyEvent:))]
        fn handle_url_event(
            &self,
            event: &NSAppleEventDescriptor,
            _reply_event: &NSAppleEventDescriptor,
        ) {
            // SAFETY:
            // 1. paramDescriptorForKeyword: 不在 bindings，用 msg_send! 呼叫。
            //    回傳型別是 nullable `NSAppleEventDescriptor *`，
            //    用 raw pointer + NonNull::new 安全處理 null。
            // 2. Retained::retain() 對 non-null ObjC 物件是安全的，
            //    讓 Rust 管理 retain count（paramDescriptorForKeyword 回傳 autoreleased）。
            let url_string: Option<String> = unsafe {
                // Step 1：取出 event 的主要參數（URL 字串所在位置）
                let raw_desc: *mut NSAppleEventDescriptor =
                    msg_send![event, paramDescriptorForKeyword: KEY_DIRECT_OBJECT];

                // Step 2：null check → 轉成 Retained
                // 實際簽名：Retained::retain(ptr: *mut T) -> Option<Retained<T>>
                // ptr 為 null 時回傳 None，直接傳 raw_desc 即可。
                let retained_desc: Option<Retained<NSAppleEventDescriptor>> =
                    Retained::retain(raw_desc);

                // Step 3：取出字串（stringValue 有 binding，直接呼叫）
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

/// 靜態持有 handler 物件，防止被釋放。
/// NSAppleEventManager 只持有 weak reference，
/// 若此靜態不持有，handler 會被 drop，之後的 URL event 會無聲失敗。
///
/// VRURLHandler: Send + Sync（NSObject super + 無 ivars）
/// → 可以安全地放入 OnceLock<_>
static HANDLER: OnceLock<Retained<VRURLHandler>> = OnceLock::new();

// ── 公開 API ───────────────────────────────────────────────────────────────

/// 向 NSAppleEventManager 註冊 URL scheme handler，
/// 並回傳 receiver 端供 async 任務監聽。
///
/// **必須在 main thread 呼叫**（NSAppleEventManager 是 !Send + !Sync）。
/// **必須在 Slint event loop 啟動前呼叫**，否則早期 event 可能遺失。
/// 重複呼叫為 no-op（OnceLock 保護），但 receiver 會是孤立的。
pub fn register() -> mpsc::UnboundedReceiver<String> {
    let (tx, rx) = mpsc::unbounded_channel::<String>();

    if URL_SENDER.set(tx).is_err() {
        debug!("url_handler::register() 被重複呼叫，略過重複初始化");
        return rx;
    }

    // 建立 ObjC handler 物件並存入靜態（keep-alive）
    //
    // define_class! 不自動繼承 NSObject::new()，
    // 必須用標準的 alloc + set_ivars + super init 模式：
    //   1. alloc()       → Allocated<VRURLHandler>（需 AllocAnyThread trait）
    //   2. set_ivars(()) → 初始化 ivars（預設為 ()，需 DefinedClass trait）
    //   3. msg_send![super(...), init] → 呼叫 NSObject 的 init
    //
    // SAFETY: NSObject 的 init 對無自訂初始化邏輯的子類別始終安全。
    let handler: Retained<VRURLHandler> = unsafe {
        let alloc = VRURLHandler::alloc().set_ivars(());
        msg_send![super(alloc), init]
    };
    HANDLER.set(handler).ok();

    // 向 NSAppleEventManager 註冊 handler
    //
    // Obj-C 簽名：
    //   - (void)setEventHandler:(id)handler
    //              andSelector:(SEL)selector
    //            forEventClass:(AEEventClass)eventClass   // u32
    //              andEventID:(AEEventID)eventID;         // u32
    //
    // SAFETY:
    // - NSAppleEventManager::sharedAppleEventManager() 回傳 app-wide singleton，
    //   在 NSApplication 初始化後始終有效。
    // - setEventHandler:andSelector:... 的型別對應正確（id, SEL, u32, u32）。
    // - HANDLER.get().unwrap() 在此行之前已設定，不會 panic。
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
        event_class = INTERNET_EVENT_CLASS, // 0x4755524C = 'GURL'
        event_id = AE_GET_URL,              // 0x4755524C = 'GURL'
        "URL scheme handler 已向 NSAppleEventManager 註冊"
    );

    rx
}
