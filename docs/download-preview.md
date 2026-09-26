### 🖥️ 安装说明

| 平台 | 安装方式 | 首次打开 |
| :-- | :-- | :-- |
| **macOS**（Apple Silicon） | 打开 `.dmg`，把 **PixivFlow Desktop** 拖进「应用程序」 | 未签名：在「访达」里**右键 → 打开**，或到「系统设置 → 隐私与安全性」点「仍要打开」 |
| **Windows** 10/11 | 运行 `-setup.exe` 安装 | 未签名：SmartScreen 里点「更多信息 → 仍要运行」 |
| **Linux**（x64） | `sudo apt install ./PixivFlow-Desktop-v*-linux-amd64.deb` | 直接启动，数据在 `~/.local/share/` 下的应用目录 |

> 安装包目前**未做代码签名**，所以系统会拦一次；这是已知限制，不是安装包损坏。
> 校验完整性请下载同页的 `SHA256SUMS` 并执行 `sha256sum -c SHA256SUMS`。
>
> 装完不用再装别的：桌面版自带 PixivFlow 运行时（含独立 `node`）与 WebUI，双击即可用。
> 首次启动会问一次 Pixiv 登录，登录页直接嵌在应用窗口里。
