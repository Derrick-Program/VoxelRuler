# 隱藏 Bug 審查報告（2026-06-12）

> 範圍：`src/` 全部 14 個檔案人工審查（沙盒無 Rust toolchain，`cargo check`/`clippy` 請於本機補跑）
> 審查人：Claude（derrick 指派）

---

## 🔴 高嚴重度

### 1. 啟動實例的 race condition：重複點擊會雙重啟動
`src/view.rs` `on_launch_instance`（約 L366–423）

`running_procs.contains_key()` 的檢查在 callback 當下執行，但 `child` 要等 `do_launch()`
**整個下載安裝流程跑完**（可能數分鐘）才 insert 進 `running_procs`。
期間再點一次啟動 → 兩條 `do_launch` 並行：重複下載互踩、最後開兩個遊戲。

**建議**：點擊當下先放一個「launching」哨兵（如 `HashSet<String>` 或 insert 佔位），
失敗時移除；或啟動期間把按鈕 disable。

### 2. 取消登入沒有真的取消，且 port 8114 永久卡死
`src/view.rs` L1010–1015、`src/mc_token.rs` `await_oauth_redirect`

- `on_cancel_login` 只設 `AtomicBool`；背景 task 仍卡在 `listener.accept().await`，永遠不會結束。
- 再按一次登入 → 第二個 task `TcpListener::bind("127.0.0.1:8114")` 得到 **Address already in use**
  → 之後每次登入都失敗，直到重開 app。
- 另外 cancel flag 是在 `set_token_in_native_store` **完成後**才檢查——session 已寫入
  keyring、`GLOBAL_CACHE` 已塞 token，「取消」只是不更新 UI，實際上已登入。

**建議**：把登入 task 的 `JoinHandle` 存起來，cancel 時 `abort()`（TcpListener drop 即釋放 port）；
或用 `tokio::select!` 搭配 cancel channel。

### 3. 移除帳號（登出）沒清 `GLOBAL_CACHE`
`src/view.rs` `on_remove_account`（L1096–1130）

`delete_session()` 只刪 keyring/磁碟，`GLOBAL_CACHE` 的 `mc_ac_key` 還在 →
登出後啟動遊戲仍帶舊 token，UI 顯示未登入但實際是登入狀態。
另外：移除**任何**一列帳號（包含 Offline 帳號）都會刪掉 Microsoft session。

**建議**：登出時 `GLOBAL_CACHE.remove("mc_ac_key")`；只有移除的是 Microsoft 帳號才 `delete_session()`。
（對應 TODO.md M1 未勾的「登出功能」項）

### 4. OAuth redirect 只 accept 一條連線 → 會被瀏覽器 preconnect 卡死
`src/mc_token.rs` `await_oauth_redirect`

Chrome/Edge 會對 `127.0.0.1:8114` 做 speculative preconnect（先開 TCP 不送資料）。
`accept()` 一次後就 `read_line` → 讀到的是那條空連線，登入流程永久卡住或解析失敗。

**建議**：改成 loop accept，直到收到帶 `code` 參數的合法 HTTP request 為止（其他連線回 404 後關閉）。

---

## 🟠 中嚴重度

### 5. `McPaths::version_jar()` 會偷偷建立空的 jar 檔
`src/mc_paths.rs` L33–40

```rust
if !d.exists() { std::fs::File::create(&d).ok(); }
```
path getter 不該有建檔副作用。一旦有人呼叫它，空 jar 會讓
`missing_classpath_files()` 的 `exists()` 檢查通過 → 啟動時 JVM 直接炸。
目前無 caller（純地雷），請改成只回傳路徑。

### 6. `download_and_verify` 只比大小就跳過，不驗 SHA1
`src/mc_install.rs` L36–38

檔案存在且 size 相同 → 直接視為有效。損毀但同大小的檔案（斷電、磁碟錯誤）永遠不會被修復。
**建議**：至少對 jar/client 在 size 相同時仍抽驗 sha1，或提供「強制驗證」修復路徑。

### 7. 每個下載都 `reqwest::get()` → 每檔案新建一個 Client
`src/mc_install.rs` `download_and_verify`

`reqwest::get` 內部每次建新 Client（獨立連線池）。assets 併發 128 → 數千次 TLS 握手，
慢且容易被 Mojang CDN 限流。**建議**：建一個 `static` shared `Client` 重複使用。

### 8. watcher task 用 std mpsc 阻塞 recv 卡住 tokio worker thread
`src/view.rs` L150–151

```rust
tokio::spawn(async move { while rx.recv().is_ok() { ... } });
```
`std::sync::mpsc::Receiver::recv()` 是阻塞呼叫，整條 tokio worker thread 被永久佔住。
**建議**：改 `tokio::task::spawn_blocking`，或 debouncer callback 改送 `tokio::sync::mpsc`。

### 9. 實例 icon 用 `env!("CARGO_MANIFEST_DIR")` 載入
`src/view.rs` L108

打包發布後（M6）這個路徑是**編譯機**的絕對路徑，使用者機器上不存在 → icon 全空白。
另外每次重建列表都從磁碟重新載圖（搜尋每個按鍵觸發一次）。
**建議**：用 slint 內建資源（`@image-url` / `Assets`）或 build 時嵌入，並 cache `slint::Image`。

### 10. 搜尋會把 running 狀態洗回 ready
`src/view.rs` `on_search_changed`（L343–357）

watcher 重建列表有保留 `running_ids`，但搜尋的重建沒有 → 遊戲執行中打個搜尋字，
卡片狀態變回 ready（雖然 `running_procs` 擋住重複啟動，但 UI 錯了，停止按鈕也消失）。

### 11. asset index 重新序列化會丟欄位；legacy 資產未處理
`src/mc_install.rs` `install_assets` + `src/mc_types.rs` `McAssetObjects`

寫回磁碟的 index 是 parse 後再 serialize 的 `McAssetObjects`，只剩 `objects`，
`virtual` / `map_to_resources` 欄位被丟掉；而且 ≤1.7.2（index `legacy` / `pre-1.6`）
需要把物件複製成 `assets/virtual/legacy/<原始檔名>` 的佈局，目前完全沒做 →
老版本進遊戲沒音效/語言檔。
**建議**：index 原始 bytes 直接落盤；之後再補 virtual assets 重建。

### 12. kill 後不 `wait()` → Unix 殭屍程序；刪除實例時檔案還被鎖
`src/view.rs` `on_kill_instance`（L427–434）、`on_confirm_delete`（L652–671）

- `child.kill()` 後直接 drop，沒有 `wait()` 回收 → zombie 掛到 launcher 退出。
- 刪除流程 kill 完**立刻** `remove_dir_all`，程序還沒退出（Windows 檔案鎖）→ 刪除容易失敗，
  且 `delete_one` 失敗只寫 log，UI 無提示。

**建議**：kill 後在背景 `wait()`，確認退出再刪資料夾；刪除失敗回報 UI。

---

## 🟡 低嚴重度 / 品質

13. **logging 在 deep link 處理之後才初始化**（`src/main.rs`）：deep link 分支裡所有 `debug!`
    在 subscriber 還沒 init 時發出，全部丟失。把 logging init 移到 main 最前面。
14. **profile 抓取失敗被 `unwrap_or_default()` 吞掉**（`src/mc_token.rs` `mint_and_save_mc_session`）：
    暫時性網路錯誤會把 `mc_username` 存成空字串 → 啟動時 `!username.is_empty()` 判定不顯示帳號，
    登入成功卻看似未登入，且之後不會自我修復。
15. **`GLOBAL_CACHE` token 執行期間不刷新**：只在 app 啟動時 refresh；掛機超過 token 效期後
    啟動遊戲會帶過期 token（多人伺服器驗證失敗）。`do_launch` 前應檢查 `mc_token_expires_at`。
16. **實例列表無排序**（`src/mc_instance.rs` `load`）：`read_dir` 順序不定，
    每次刷新卡片可能洗牌。load 後按 name 或 last_played 排序。
17. **xmx/xms 無格式驗證**：使用者輸入 `abc` → `-Xmxabc` → JVM 起不來，錯誤訊息難懂。
    存檔前用 regex `^\d+[KMGkmg]?$` 驗證。
18. **CSRF 驗證失敗前瀏覽器已顯示「驗證成功」**（`mc_token.rs`）：成功頁面在 state 比對前就回給瀏覽器。
19. **mods 搜尋的 `raw_mods` 是啟動時快照**（`view.rs` L944）：之後 mod_list 變動，搜尋資料來源是舊的（WIP 頁面，先記著）。
20. **`on_launch_instance` 找不到 config 時 fallback 用 id 當 version**：會去 Mojang 找名為 UUID 的版本，
    產生難懂錯誤；直接 return + 顯示錯誤較好。
21. **`select_bundled_translation("en_US")` 寫死**，zh_TW 翻譯永遠用不到（已知，code 有註解）。
22. **遊戲結束後 `last_played` / `play_time_secs` 從未更新**：欄位存在但無人寫入（M4 範圍）。

---

## 建議處理順序

| 優先 | 項目 | 理由 |
|------|------|------|
| P0 | #2 #4（登入流程） | 使用者一取消/遇到 preconnect 就永久卡死，M1 驗收直接掛 |
| P0 | #1（啟動 race） | 核心功能，雙擊即重現 |
| P1 | #3（登出清 cache）、#10（搜尋洗狀態） | M1/M4 驗收項 |
| P1 | #7 #8（效能/執行緒）、#9（M6 打包前必修） | |
| P2 | 其餘 | 隨相關里程碑處理 |
