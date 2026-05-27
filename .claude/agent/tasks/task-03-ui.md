# Task 03 — UI 層 (Slint 檔案)

> **依賴**：Task 01 完成後即可執行（可與 Task 02 平行）  
> **預計時間**：10–15 分鐘  
> **分支**：feat/mc/new_instance

---

## 目標

1. 在 `ui/global.slint` 新增 `InstanceCreateLogic` global
2. 建立 `ui/components/pages/create-instance-dialog.slint`（3-tab 對話框）
3. 在 `ui/components/index.slint` 加入 export
4. 在 `ui/main.slint` 整合 dialog

---

## 步驟

### Step 1：在 global.slint 新增 InstanceCreateLogic

在 `ui/global.slint` 檔案末尾（`PageAccountLogic` global 結束後），追加以下內容：

```slint
export global InstanceCreateLogic {
    // Dialog control
    in-out property <bool>     show-dialog: false;
    in-out property <int>      active-tab: 0;

    // Basic Tab
    in-out property <string>   name: "";
    in-out property <[string]> version-list: [];
    in-out property <string>   selected-version: "";
    in-out property <string>   mod-loader: "None";

    // Advanced Tab
    in-out property <string>   xmx: "2G";
    in-out property <string>   xms: "512M";
    in-out property <bool>     logs-enabled: true;

    // Resources Tab
    in-out property <string>   world-path: "";
    in-out property <string>   resource-pack: "";
    in-out property <string>   shader-pack: "";

    // State
    in-out property <string>   error-msg: "";
    in-out property <bool>     is-loading: false;

    callback confirm-create();
    callback cancel-create();
}
```

### Step 2：建立 create-instance-dialog.slint

建立新檔案 `ui/components/pages/create-instance-dialog.slint`，完整內容如下：

```slint
import { AppTheme } from "@theme";
import { InstanceCreateLogic } from "@global";
import { ComboBox } from "std-widgets.slint";

export component CreateInstanceDialog inherits Rectangle {
    background: #000000.with-alpha(0.6);

    Rectangle {
        width: 460px;
        height: 520px;
        x: (parent.width - self.width) / 2;
        y: (parent.height - self.height) / 2;
        background: #1e2028;
        border-radius: 12px;
        border-width: 1px;
        border-color: #ffffff.with-alpha(0.1);

        VerticalLayout {
            // ── Header ──────────────────────────────────────────────
            Rectangle {
                height: 52px;
                HorizontalLayout {
                    padding-left: 20px;
                    padding-right: 12px;
                    alignment: space-between;
                    Text {
                        text: @tr("New Minecraft Instance");
                        font-size: 15px;
                        font-weight: 600;
                        color: #e0e0e0;
                        vertical-alignment: center;
                    }
                    Rectangle {
                        width: 28px;
                        height: 28px;
                        border-radius: 6px;
                        background: x-ta.has-hover ? #ffffff.with-alpha(0.1) : transparent;
                        x-ta := TouchArea { clicked => { InstanceCreateLogic.cancel-create(); } }
                        Text {
                            text: "✕";
                            color: #888;
                            font-size: 14px;
                            horizontal-alignment: center;
                            vertical-alignment: center;
                        }
                    }
                }
            }

            Rectangle { height: 1px; background: #ffffff.with-alpha(0.08); }

            // ── Tab bar ─────────────────────────────────────────────
            Rectangle {
                height: 44px;
                HorizontalLayout {
                    padding-left: 12px;
                    spacing: 4px;
                    alignment: start;

                    Rectangle {
                        width: 80px; height: 44px;
                        tab0-ta := TouchArea { clicked => { InstanceCreateLogic.active-tab = 0; } }
                        Text {
                            text: @tr("Basic");
                            horizontal-alignment: center; vertical-alignment: center;
                            font-size: 13px;
                            color: InstanceCreateLogic.active-tab == 0 ? #76bc51 : #888;
                            font-weight: InstanceCreateLogic.active-tab == 0 ? 700 : 400;
                        }
                        Rectangle {
                            y: parent.height - 2px; x: 8px;
                            width: parent.width - 16px; height: 2px;
                            background: InstanceCreateLogic.active-tab == 0 ? #76bc51 : transparent;
                        }
                    }
                    Rectangle {
                        width: 80px; height: 44px;
                        tab1-ta := TouchArea { clicked => { InstanceCreateLogic.active-tab = 1; } }
                        Text {
                            text: @tr("Advanced");
                            horizontal-alignment: center; vertical-alignment: center;
                            font-size: 13px;
                            color: InstanceCreateLogic.active-tab == 1 ? #76bc51 : #888;
                            font-weight: InstanceCreateLogic.active-tab == 1 ? 700 : 400;
                        }
                        Rectangle {
                            y: parent.height - 2px; x: 8px;
                            width: parent.width - 16px; height: 2px;
                            background: InstanceCreateLogic.active-tab == 1 ? #76bc51 : transparent;
                        }
                    }
                    Rectangle {
                        width: 80px; height: 44px;
                        tab2-ta := TouchArea { clicked => { InstanceCreateLogic.active-tab = 2; } }
                        Text {
                            text: @tr("Resources");
                            horizontal-alignment: center; vertical-alignment: center;
                            font-size: 13px;
                            color: InstanceCreateLogic.active-tab == 2 ? #76bc51 : #888;
                            font-weight: InstanceCreateLogic.active-tab == 2 ? 700 : 400;
                        }
                        Rectangle {
                            y: parent.height - 2px; x: 8px;
                            width: parent.width - 16px; height: 2px;
                            background: InstanceCreateLogic.active-tab == 2 ? #76bc51 : transparent;
                        }
                    }
                }
            }

            Rectangle { height: 1px; background: #ffffff.with-alpha(0.08); }

            // ── Tab content ─────────────────────────────────────────
            Rectangle {
                vertical-stretch: 1;

                // Basic
                if InstanceCreateLogic.active-tab == 0: VerticalLayout {
                    padding: 20px; spacing: 16px;

                    VerticalLayout {
                        spacing: 6px;
                        Text { text: @tr("Instance Name"); font-size: 12px; color: #9e9e9e; }
                        Rectangle {
                            height: 36px; border-radius: 6px;
                            border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                            background: #242424;
                            TextInput {
                                x: 12px; width: parent.width - 24px; height: parent.height;
                                text <=> InstanceCreateLogic.name;
                                placeholder-text: @tr("e.g. Survival 1.20");
                                color: #e0e0e0; font-size: 13px; vertical-alignment: center;
                            }
                        }
                    }

                    VerticalLayout {
                        spacing: 6px;
                        Text { text: @tr("Minecraft Version"); font-size: 12px; color: #9e9e9e; }
                        if InstanceCreateLogic.is-loading: Rectangle {
                            height: 36px; border-radius: 6px;
                            border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                            background: #242424;
                            Text {
                                x: 12px; height: parent.height;
                                text: @tr("Loading versions...");
                                color: #888; font-size: 13px; vertical-alignment: center;
                            }
                        }
                        if !InstanceCreateLogic.is-loading: ComboBox {
                            height: 36px;
                            model: InstanceCreateLogic.version-list;
                            current-value <=> InstanceCreateLogic.selected-version;
                        }
                    }

                    VerticalLayout {
                        spacing: 8px;
                        Text { text: @tr("Mod Loader"); font-size: 12px; color: #9e9e9e; }
                        HorizontalLayout {
                            spacing: 20px; alignment: start;

                            HorizontalLayout {
                                spacing: 6px;
                                Rectangle {
                                    width: 16px; height: 16px; border-radius: 8px;
                                    border-width: 2px;
                                    border-color: InstanceCreateLogic.mod-loader == "None" ? #76bc51 : #666;
                                    background: transparent;
                                    if InstanceCreateLogic.mod-loader == "None": Rectangle {
                                        width: 8px; height: 8px; x: 2px; y: 2px;
                                        border-radius: 4px; background: #76bc51;
                                    }
                                    r0-ta := TouchArea { clicked => { InstanceCreateLogic.mod-loader = "None"; } }
                                }
                                Text { text: "None"; color: #e0e0e0; font-size: 13px; vertical-alignment: center; }
                            }

                            HorizontalLayout {
                                spacing: 6px;
                                Rectangle {
                                    width: 16px; height: 16px; border-radius: 8px;
                                    border-width: 2px;
                                    border-color: InstanceCreateLogic.mod-loader == "Fabric" ? #76bc51 : #666;
                                    background: transparent;
                                    if InstanceCreateLogic.mod-loader == "Fabric": Rectangle {
                                        width: 8px; height: 8px; x: 2px; y: 2px;
                                        border-radius: 4px; background: #76bc51;
                                    }
                                    r1-ta := TouchArea { clicked => { InstanceCreateLogic.mod-loader = "Fabric"; } }
                                }
                                Text { text: "Fabric"; color: #e0e0e0; font-size: 13px; vertical-alignment: center; }
                            }

                            HorizontalLayout {
                                spacing: 6px;
                                Rectangle {
                                    width: 16px; height: 16px; border-radius: 8px;
                                    border-width: 2px;
                                    border-color: InstanceCreateLogic.mod-loader == "Forge" ? #76bc51 : #666;
                                    background: transparent;
                                    if InstanceCreateLogic.mod-loader == "Forge": Rectangle {
                                        width: 8px; height: 8px; x: 2px; y: 2px;
                                        border-radius: 4px; background: #76bc51;
                                    }
                                    r2-ta := TouchArea { clicked => { InstanceCreateLogic.mod-loader = "Forge"; } }
                                }
                                Text { text: "Forge"; color: #e0e0e0; font-size: 13px; vertical-alignment: center; }
                            }
                        }
                    }
                }

                // Advanced
                if InstanceCreateLogic.active-tab == 1: VerticalLayout {
                    padding: 20px; spacing: 16px;

                    HorizontalLayout {
                        spacing: 12px;
                        Text {
                            text: @tr("Max Memory (Xmx)"); width: 150px;
                            font-size: 13px; color: #e0e0e0; vertical-alignment: center;
                        }
                        Rectangle {
                            horizontal-stretch: 1; height: 36px; border-radius: 6px;
                            border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                            background: #242424;
                            TextInput {
                                x: 12px; width: parent.width - 24px; height: parent.height;
                                text <=> InstanceCreateLogic.xmx;
                                placeholder-text: "2G";
                                color: #e0e0e0; font-size: 13px; vertical-alignment: center;
                            }
                        }
                    }

                    HorizontalLayout {
                        spacing: 12px;
                        Text {
                            text: @tr("Init Memory (Xms)"); width: 150px;
                            font-size: 13px; color: #e0e0e0; vertical-alignment: center;
                        }
                        Rectangle {
                            horizontal-stretch: 1; height: 36px; border-radius: 6px;
                            border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                            background: #242424;
                            TextInput {
                                x: 12px; width: parent.width - 24px; height: parent.height;
                                text <=> InstanceCreateLogic.xms;
                                placeholder-text: "512M";
                                color: #e0e0e0; font-size: 13px; vertical-alignment: center;
                            }
                        }
                    }

                    HorizontalLayout {
                        spacing: 12px;
                        Text {
                            text: @tr("Enable Game Logs"); width: 150px;
                            font-size: 13px; color: #e0e0e0; vertical-alignment: center;
                        }
                        Rectangle {
                            width: 42px; height: 24px; border-radius: 12px;
                            background: InstanceCreateLogic.logs-enabled ? #76bc51 : #555;
                            tog-ta := TouchArea {
                                clicked => { InstanceCreateLogic.logs-enabled = !InstanceCreateLogic.logs-enabled; }
                            }
                            Rectangle {
                                width: 20px; height: 20px; border-radius: 10px; background: white;
                                x: InstanceCreateLogic.logs-enabled ? parent.width - self.width - 2px : 2px;
                                y: 2px;
                                animate x { duration: 150ms; easing: ease-in-out; }
                            }
                        }
                    }
                }

                // Resources
                if InstanceCreateLogic.active-tab == 2: VerticalLayout {
                    padding: 20px; spacing: 16px;

                    VerticalLayout {
                        spacing: 6px;
                        Text { text: @tr("World/Save Path"); font-size: 12px; color: #9e9e9e; }
                        Rectangle {
                            height: 36px; border-radius: 6px;
                            border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                            background: #242424;
                            TextInput {
                                x: 12px; width: parent.width - 24px; height: parent.height;
                                text <=> InstanceCreateLogic.world-path;
                                placeholder-text: @tr("Full path to world (optional)");
                                color: #e0e0e0; font-size: 13px; vertical-alignment: center;
                            }
                        }
                    }

                    VerticalLayout {
                        spacing: 6px;
                        Text { text: @tr("Resource Pack Path"); font-size: 12px; color: #9e9e9e; }
                        Rectangle {
                            height: 36px; border-radius: 6px;
                            border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                            background: #242424;
                            TextInput {
                                x: 12px; width: parent.width - 24px; height: parent.height;
                                text <=> InstanceCreateLogic.resource-pack;
                                placeholder-text: @tr("Full path to resource pack (optional)");
                                color: #e0e0e0; font-size: 13px; vertical-alignment: center;
                            }
                        }
                    }

                    VerticalLayout {
                        spacing: 6px;
                        Text { text: @tr("Shader Pack Path"); font-size: 12px; color: #9e9e9e; }
                        Rectangle {
                            height: 36px; border-radius: 6px;
                            border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                            background: #242424;
                            TextInput {
                                x: 12px; width: parent.width - 24px; height: parent.height;
                                text <=> InstanceCreateLogic.shader-pack;
                                placeholder-text: @tr("Full path to shader pack (optional)");
                                color: #e0e0e0; font-size: 13px; vertical-alignment: center;
                            }
                        }
                    }
                }
            }

            Rectangle { height: 1px; background: #ffffff.with-alpha(0.08); }

            // ── Footer ──────────────────────────────────────────────
            Rectangle {
                height: 64px;
                VerticalLayout {
                    padding-top: 4px;
                    padding-left: 20px;
                    padding-right: 20px;
                    spacing: 4px;
                    alignment: center;

                    if InstanceCreateLogic.error-msg != "": Text {
                        text: InstanceCreateLogic.error-msg;
                        color: #e74c3c; font-size: 12px;
                        horizontal-alignment: right; wrap: word-wrap;
                    }

                    HorizontalLayout {
                        alignment: end; spacing: 8px;

                        Rectangle {
                            width: 80px; height: 34px; border-radius: 8px;
                            border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                            background: cancel-ta.has-hover ? #ffffff.with-alpha(0.1) : transparent;
                            cancel-ta := TouchArea { clicked => { InstanceCreateLogic.cancel-create(); } }
                            Text {
                                text: @tr("Cancel"); color: #888; font-size: 13px;
                                horizontal-alignment: center; vertical-alignment: center;
                            }
                        }

                        Rectangle {
                            width: 110px; height: 34px; border-radius: 8px;
                            background: confirm-ta.has-hover ? #67ab44 : #5a9a3a;
                            confirm-ta := TouchArea { clicked => { InstanceCreateLogic.confirm-create(); } }
                            Text {
                                text: @tr("Create Instance"); color: white;
                                font-size: 13px; font-weight: 600;
                                horizontal-alignment: center; vertical-alignment: center;
                            }
                        }
                    }
                }
            }
        }
    }
}
```

### Step 3：在 index.slint 新增 export

在 `ui/components/index.slint` 末尾新增：
```slint
export { CreateInstanceDialog } from "./pages/create-instance-dialog.slint";
```

### Step 4：更新 main.slint

修改 `ui/main.slint`：

**4a. 更新 components import（加入 CreateInstanceDialog）：**
```slint
// 改成：
import { SideBar, Footer, Pages, DownloadDialog, LogDialog, CreateInstanceDialog } from "components/index.slint";
```

**4b. 更新 global import（加入 InstanceCreateLogic）：**
```slint
// 改成：
import { AppData, InstanceLogic, AppLogic, PageAccountLogic, ModLogic, ModData, InstanceCreateLogic } from "@global";
```

**4c. 更新 export 行：**
```slint
// 改成：
export { AppData, InstanceLogic, AppLogic, PageAccountLogic, ModLogic, ModData, InstanceCreateLogic }
```

**4d. 在 MainApp 最後一個 `if` block 之後新增 dialog：**
```slint
if InstanceCreateLogic.show-dialog: CreateInstanceDialog {
    x: 0;
    y: 0;
    width: root.width;
    height: root.height;
}
```

### Step 5：驗證編譯

```bash
cd /Users/derrick/Documents/Program/rust/Project/VoxelRuler
cargo check
```
預期：無錯誤（如果 Task 02 還沒完成，Rust 端的 `InstanceCreateLogic` 相關代碼會在 Task 04 才加入，目前 Slint 部分應能通過）

### Step 6：提交

```bash
cd /Users/derrick/Documents/Program/rust/Project/VoxelRuler
git add ui/global.slint ui/components/pages/create-instance-dialog.slint ui/components/index.slint ui/main.slint
git commit -m "feat: add InstanceCreateLogic global and 3-tab CreateInstanceDialog UI"
```

---

## 完成後

更新 `.claude/agent/status.md`，將 Batch 3 改為 `✅ 完成`，並記錄完成時間。
