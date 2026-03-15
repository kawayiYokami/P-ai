# π師傅（Arch Linux 本地安裝）

## 安裝

```bash
cd packaging/arch
chmod +x install-with-yay.sh
./install-with-yay.sh
```

## 更新

```bash
cd packaging/arch
./install-with-yay.sh
```

腳本會先抓 GitHub 最新 release tag，更新 `PKGBUILD` 的 `pkgver`，再用 `yay -Bi` 重新建置與升級。

## 資料位置

應用資料預設會放在：

- `~/.config/easy-call-ai/`

其中常見子目錄：

- `config/`（設定）
- `chat/`（對話資料）
- `memory/`（`memory_store.db`）
- `media/`（圖片/音訊）
- `backups/`（遷移備份）
