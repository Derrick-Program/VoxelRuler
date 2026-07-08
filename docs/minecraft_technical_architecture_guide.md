# Minecraft 技術架構深度剖析：從啟動器核心、皮膚 API 到 3D 渲染機制

本文件完整梳理了 Minecraft 整合生態系中的核心技術實作，涵蓋第三方啟動器（以 Prism Launcher 為例）的底層架構、官方皮膚（Skin）的生命週期與替換邏輯、離線狀態下的預設降級機制、自動化 CLI 管線實作，以及網頁端 3D 外觀預覽技術。

---

## 一、 Prism Launcher 技術架構與實作核心

Prism Launcher 作為高效能、跨平台的開源第三方麥塊啟動器，其核心設計旨在解決「環境隔離」與「高效能非同步任務處理」兩大痛點。

### 1. 核心技術棧 (Tech Stack)
* **開發語言**：完全基於 **C++**（現代 C++ 特性，如 `std::shared_ptr` 智慧型指標）編寫，確保極低的記憶體佔用與極高的檔案 I/O 處理效能。
* **GUI 框架**：採用 **Qt 框架 (Qt 5 / Qt 6)**。不僅用於繪製原生跨平台介面，更深度依賴 Qt 的 `QEventLoop` 處理非同步事件。

### 2. 非同步任務系統 (Task System)
為了避免在下載大量模組（Mod）時造成軟體畫面凍結（ANR），Prism Launcher 實作了物件導向的任務排程器：
* **`Task` 基底類別**：所有耗時操作（如物件建立、解壓縮、檔案校驗、Java 檢查）皆繼承自 `Task`，具備獨立狀態機（未執行、執行中、成功、失敗）與訊號回報機制。
* **並行下載引擎 (`NetJob`)**：當玩家安裝包含數百個模組的整合包時，系統會建立一個 `NetJob` 容器，內含數個 `NetAction` 子任務。透過多執行緒並行發送 HTTP 請求，並在下載完成後即時進行 **SHA-1 / SHA-256 雜湊校驗**。

### 3. 中繼資料與依賴樹解析 (Meta Parser)
由於 Minecraft 各版本、各載入器（Forge, Fabric, NeoForge）與 Java 版本間的依賴關係極其複雜，Prism 團隊維護了中心化的中繼伺服器（`meta.prismlauncher.org`）：
* 啟動器透過 API 拉取 JSON 格式的依賴表。
* 在記憶體中建構**遞迴依賴樹（Dependency Tree）**，精準計算出當前版本所需的本機函式庫（如 `LWJGL` 的 `.dll` / `.so` / `.dylib` 靜態連結庫）。

### 4. 子行程接管 (Launch Process)
啟動遊戲的最終任務是拼裝出極度冗長的 JVM 啟動參數：
1. 透過微軟 Xbox Live 進行 **OAuth 2.0 驗證**，刷新並獲取正版 Session Token。
2. 串接所有 `.jar` 依賴硬體路徑，構建完整的 **Classpath**。
3. 加入記憶體限制參數（如 `-Xmx4G`）與垃圾回收（GC）最佳化參數。
4. 使用 Qt 的 **`QProcess`** 以子行程方式喚醒本機 Java 虛擬機，並將 `stdout` 與 `stderr` 重新導向至啟動器的日誌主控台。

---

## 二、 Minecraft 皮膚（Skin）生命週期與替換邏輯

Minecraft 的皮膚系統是一個典型的 **RESTful API 查詢與雙層快取失效（Cache Invalidation）** 架構。

### 1. 後端儲存與 UUID 綁定
在微軟資料庫中，玩家的帳號由一組不變的 **UUID** 為 Primary Key。當玩家透過官網上傳皮膚 `.png` 時：
* 後端計算圖片 Hash 值，並將圖片上傳至專屬 CDN 伺服器：`textures.minecraft.net/texture/<Hash>`。
* 資料庫中該 UUID 的指標會被**覆蓋**，重新指向新的 CDN 網址，並記錄模型中繼資料（`classic` 粗手 或 `slim` 細手）。
* **註：官方不提供歷史紀錄備份。一旦覆蓋，舊綁定即在官方系統消失。**

### 2. 用戶端獲取流程 (Runtime Skin Retrieval)
當一個玩家進入另一個玩家的視野時，遊戲本體會觸發以下網路串流：

1. **身分映射**：向 `api.mojang.com/users/profiles/minecraft/<ID>` 查詢取得 UUID。
2. **獲取安全屬性**：向 `sessionserver.mojang.com/session/minecraft/profile/<UUID>` 請求玩家 Profile JSON。
3. **安全解密**：提取 JSON 中 `properties` 陣列內的 Base64 加密字串。解碼該字串後，方可取得真實的 CDN 圖片 URL。
4. **貼圖渲染**：透過圖形 API（OpenGL / Vulkan）將下載的 2D 紋理映射至 3D 網格（Mesh）上。

### 3. 快取機制與延遲問題
為減輕伺服器負載，系統設有兩層快取：
* **伺服器端快取**：Mojang Session API 對同一 UUID 的請求設有約 1 分鐘的 TTL 快取。
* **用戶端快取**：遊戲本體會將皮膚快取在硬碟本機目錄（`.minecraft/assets/skins/`）。若不重登伺服器或跨越維度（觸發實體重組），用戶端不會主動刷新視野內玩家的皮膚。

---

## 三、 未登入與離線模式的預設降級（Fallback）機制

當遊戲處於未登入狀態、完全斷網、或 Mojang API 伺服器逾時（Timeout）時，系統會啟動防呆的**降級機制（Graceful Degradation）**。

### 1. 本地靜態資產
遊戲 `.jar` 核心中內建了兩張經典的預設皮膚：`steve.png`（4-pixel 粗手）與 `alex.png`（3-pixel 細手）。

### 2. 離線 UUID 雜湊演算法
在離線模式下，系統無法調用 Session API，此時會根據玩家輸入的字串 ID，透過以下邏輯決定分派 Steve 還是 Alex：
1. **生成離線 UUID**：將字串 `OfflinePlayer:<玩家ID>` 進行 **MD5 雜湊計算**，生成一組標準的離線型 UUID。
2. **奇偶數判定**：解析該 UUID 特定個位元的數值（通常透過雜湊值的特定 Bit 位進行邏輯與 `&` 運算）。
   * 若計算結果符合群組 A：分派預設 **Steve** 模型。
   * 若計算結果符合群組 B：分派預設 **Alex** 模型。
這確保了在完全沒有網路的情境下，相同 ID 的玩家永遠會獲得一致的預設外觀。

---

## 四、 自動化下載管線：CLI 實作指南

利用 Linux/macOS 終端機中常見的 `curl`、`jq` 與 `base64` 工具，我們可以建構一條純文字的自動化 Pipeline，將指定玩家的當前皮膚下載下來：

```bash
#!/bin/bash
# Minecraft Skin Downloader Pipeline

# 1. 定義目標玩家 ID
PLAYER_NAME="Notch"

echo "[1/4] 正在查詢 ${PLAYER_NAME} 的 UUID..."
UUID=$(curl -s "https://api.mojang.com/users/profiles/minecraft/${PLAYER_NAME}" | jq -r '.id')

if [ -z "$UUID" ] || [ "$UUID" == "null" ]; then
    echo "錯誤：找不到該玩家或 API 請求失敗。"
    exit 1
fi

echo "➔ 取得 UUID: ${UUID}"

# 2. 獲取 Profile 的 Base64 中繼資料
echo "[2/4] 正在拉取 Session 中繼資料..."
BASE64_DATA=$(curl -s "https://sessionserver.mojang.com/session/minecraft/profile/${UUID}" | jq -r '.properties[0].value')

# 3. Base64 解碼並萃取真實 CDN 網址
echo "[3/4] 正在解碼並解析 CDN 網址..."
SKIN_URL=$(echo "$BASE64_DATA" | base64 --decode | jq -r '.textures.SKIN.url')

echo "➔ 取得皮膚 CDN 網址: ${SKIN_URL}"

# 4. 下載二進位圖檔並儲存
echo "[4/4] 正在下載皮膚圖檔..."
curl -s "$SKIN_URL" -o "${PLAYER_NAME}_skin.png"

echo "🎯 執行成功！檔案已儲存為: ${PLAYER_NAME}_skin.png"
```

---

## 五、 網頁端 3D 外觀預覽與渲染機制

官方網站與 NameMC 等平台所使用的網頁 3D 互動預覽，底層技術為 **WebGL**（通常採用 **Three.js** 框架）。

### 1. 3D 幾何體建構 (Geometry Mesh)

角色模型由 6 個正向長方體（BoxGeometry）階層式拼裝而成：

* **Head** (8x8x8)
* **Torso** (8x12x4)
* **Left/Right Arm** (Classic: 4x12x4 | Slim: 3x12x4)
* **Left/Right Leg** (4x12x4)

### 2. UV 映射 (UV Mapping)

2D 皮膚圖片（64x64 像素）的每個區域都有固定的坐標規範。渲染器必須手動定義 36 個面（6 個立方體 × 6 個面）的 UV 頂點坐標。例如，頭部正面的 UV 坐標區間為 `(8/64, 8/64)` 到 `(16/64, 16/64)`，渲染引擎會據此將平面的像素精確包裹到立體方塊上。

### 3. 雙層皮膚（Overlay Layer）

為了呈現外套、帽子、立體頭髮等層次感，渲染器會建立一組**體積放大約 5%** 的外層幾何體。外層材質（Material）的 `transparent` 屬性必須設定為 `true`，以完美渲染 `.png` 圖片中的 Alpha 透明通道。

### 4. 動態物理矩陣（Animation Loop）

* **互動旋轉**：綁定 `mousedown` 與 `mousemove` 事件，藉由計算滑鼠滑動的 Δx 與 Δy，動態修改 3D 相機的軌道坐標（Orbit Controls）或模型的 `rotation.y`。
* **走路動畫**：在 `requestAnimationFrame` 迴圈中，利用正弦波函數（Sine Wave）隨時間變更手腳幾何體的旋轉矩陣：

實現左右手腳交替擺動的逼真行走效果。
