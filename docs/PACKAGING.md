# 串口调试助手 — 打包指南

## 工具安装

```bash
cargo install cargo-packager      # 处理 nsis / deb / appimage / pacman
cargo install cargo-generate-rpm  # 处理 rpm
```

## 图标文件

打包前确认项目根目录下存在以下文件（已生成，无需重复操作）：

```
icons/
├── icon.ico       # Windows 安装包图标（含 16/32/48/256px）
├── 32x32.png
├── 128x128.png
└── 256x256.png
```

---

## 平台 × 架构支持矩阵

### Linux

| 格式     | x86_64   | aarch64      | armv7        | x86 (i686)  |
|----------|:--------:|:------------:|:------------:|:-----------:|
| deb      | ✅ 原生  | ✅ 交叉编译  | ✅ 交叉编译  | ✅ 交叉编译 |
| pacman   | ✅ 原生  | ✅ 交叉编译  | ✅ 交叉编译  | ✅ 交叉编译 |
| rpm      | ✅ 原生  | ✅ 交叉编译  | ✅ 交叉编译  | ✅ 交叉编译 |
| AppImage | ✅ 原生  | ⚠️ 建议原生  | ⚠️ 建议原生  | ✅ 交叉编译 |

### Windows

| 格式 | x86_64  | aarch64     | arm32       | x86 (i686)  |
|------|:-------:|:-----------:|:-----------:|:-----------:|
| nsis | ✅ 原生 | ✅ 交叉编译 | ✅ 交叉编译 | ✅ 交叉编译 |

> **AppImage 说明**：aarch64 / armv7 的 AppImage 交叉构建工具链支持不完整，推荐在原生机器或 QEMU 容器中构建。

---

## Linux x86_64（原生）

```bash
# deb + AppImage + pacman（三合一）
cargo packager --release --formats deb,appimage,pacman

# 单独格式
cargo packager --release --formats deb
cargo packager --release --formats appimage
cargo packager --release --formats pacman

# RPM（cargo-packager 不支持，单独处理）
cargo build --release --bin serial-debugger
cargo generate-rpm
```

---

## 交叉编译前提条件

所有非原生目标均需满足以下三点。

### 1. Cargo 链接器配置

项目根目录已包含 `.cargo/config.toml`，其中预置了各目标链接器。安装对应 C 工具链后无需手动修改。

### 2. 两步构建流程

`Cargo.toml` 中的 `beforePackagingCommand` 不会自动透传 `--target`，必须分两步：

```bash
# 步骤 1：手动编译二进制
cargo build --release --bin serial-debugger --target <triple>

# 步骤 2：打包（cargo-packager 读取已编译的二进制）
cargo packager --release --formats <fmt> --target <triple>
```

### 3. pkg-config 环境变量（仅 Linux 目标）

`serialport` crate 通过 `pkg-config` 查找 `libudev`。交叉编译时 pkg-config 默认只搜索宿主路径，需要手动覆盖。两种方案任选其一：

**方案 A：手动设置环境变量（通用）**

```bash
PKG_CONFIG_ALLOW_CROSS=1 \
PKG_CONFIG_SYSROOT_DIR=<sysroot> \
PKG_CONFIG_PATH=<sysroot>/lib/pkgconfig \
cargo build --release --target <triple>
```

具体路径见各目标章节。

**方案 B：cargo-cross（推荐，Docker 容器自动处理依赖）**

```bash
cargo install cross --git https://github.com/cross-rs/cross
cross build --release --bin serial-debugger --target <triple>
```

`cross` 内部使用预配置好目标 sysroot 的 Docker 镜像，无需手动设置 pkg-config，也无需安装 C 交叉工具链。

---

## Linux aarch64（在 x86_64 上交叉编译）

目标三元组：`aarch64-unknown-linux-gnu`

### 1. 安装工具链

```bash
rustup target add aarch64-unknown-linux-gnu

# Arch Linux
sudo pacman -S aarch64-linux-gnu-gcc

# Debian / Ubuntu（同时安装交叉 libudev 开发包）
sudo apt install gcc-aarch64-linux-gnu libudev-dev:arm64

# 验证
aarch64-linux-gnu-gcc --version
```

### 2. 编译二进制

```bash
# Arch Linux（需手动指定 sysroot，/usr/aarch64-linux-gnu 为工具链安装路径）
PKG_CONFIG_ALLOW_CROSS=1 \
PKG_CONFIG_SYSROOT_DIR=/usr/aarch64-linux-gnu \
PKG_CONFIG_PATH=/usr/aarch64-linux-gnu/lib/pkgconfig \
cargo build --release --bin serial-debugger --target aarch64-unknown-linux-gnu

# Debian / Ubuntu（multiarch 方案，dpkg 已配置 arm64 搜索路径）
PKG_CONFIG_ALLOW_CROSS=1 \
PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig \
cargo build --release --bin serial-debugger --target aarch64-unknown-linux-gnu

# 或使用 cargo-cross（自动处理，推荐）
cross build --release --bin serial-debugger --target aarch64-unknown-linux-gnu
```

### 3. 打包

```bash
cargo packager --release --formats deb,pacman \
    --target aarch64-unknown-linux-gnu

# AppImage（建议原生 aarch64 环境）
cargo packager --release --formats appimage \
    --target aarch64-unknown-linux-gnu

# RPM
cargo generate-rpm --target aarch64-unknown-linux-gnu
```

---

## Linux armv7（在 x86_64 上交叉编译）

目标三元组：`armv7-unknown-linux-gnueabihf`（硬浮点，适用于树莓派 2/3/4 等）

### 1. 安装工具链

```bash
rustup target add armv7-unknown-linux-gnueabihf

# Arch Linux（AUR）
yay -S arm-linux-gnueabihf-gcc

# Debian / Ubuntu
sudo apt install gcc-arm-linux-gnueabihf libudev-dev:armhf

# 验证
arm-linux-gnueabihf-gcc --version
```

### 2. 编译二进制

```bash
# Arch Linux
PKG_CONFIG_ALLOW_CROSS=1 \
PKG_CONFIG_SYSROOT_DIR=/usr/arm-linux-gnueabihf \
PKG_CONFIG_PATH=/usr/arm-linux-gnueabihf/lib/pkgconfig \
cargo build --release --bin serial-debugger --target armv7-unknown-linux-gnueabihf

# Debian / Ubuntu
PKG_CONFIG_ALLOW_CROSS=1 \
PKG_CONFIG_PATH=/usr/lib/arm-linux-gnueabihf/pkgconfig \
cargo build --release --bin serial-debugger --target armv7-unknown-linux-gnueabihf

# 或使用 cargo-cross
cross build --release --bin serial-debugger --target armv7-unknown-linux-gnueabihf
```

### 3. 打包

```bash
cargo packager --release --formats deb,pacman \
    --target armv7-unknown-linux-gnueabihf

# AppImage（建议原生 armv7 环境）
cargo packager --release --formats appimage \
    --target armv7-unknown-linux-gnueabihf

# RPM
cargo generate-rpm --target armv7-unknown-linux-gnueabihf
```

---

## Linux x86 32 位（在 x86_64 上交叉编译）

目标三元组：`i686-unknown-linux-gnu`

### 1. 安装工具链

```bash
rustup target add i686-unknown-linux-gnu

# Arch Linux（启用 multilib 仓库后）
sudo pacman -S multilib-devel lib32-systemd

# Debian / Ubuntu
sudo apt install gcc-multilib libsystemd-dev:i386
```

### 2. 编译二进制

```bash
# Arch / Debian（x86_64 gcc 开启 multilib 后可直接编译）
PKG_CONFIG_ALLOW_CROSS=1 \
PKG_CONFIG_PATH=/usr/lib/i386-linux-gnu/pkgconfig \
cargo build --release --bin serial-debugger --target i686-unknown-linux-gnu
```

### 3. 打包

```bash
cargo packager --release --formats deb,appimage,pacman \
    --target i686-unknown-linux-gnu

# RPM
cargo generate-rpm --target i686-unknown-linux-gnu
```

---

## Windows x86_64（需在 Windows 上运行）

**需要：** [NSIS](https://nsis.sourceforge.io/Download) 已安装

```powershell
cargo packager --release --formats nsis
```

安装程序特性：
- 安装到系统目录（需管理员权限）
- 简体中文 / English 界面切换
- LZMA 压缩（体积最小）
- 自带卸载程序

---

## Windows aarch64（在 x86_64 Windows 上交叉编译）

目标三元组：`aarch64-pc-windows-msvc`

**需要：** Visual Studio Build Tools + **ARM64 组件**

### 1. 安装工具链

```powershell
rustup target add aarch64-pc-windows-msvc
```

### 2. 编译

```powershell
cargo build --release --bin serial-debugger --target aarch64-pc-windows-msvc
```

### 3. 打包

```powershell
cargo packager --release --formats nsis --target aarch64-pc-windows-msvc
```

---

## Windows arm32（在 x86_64 Windows 上交叉编译）

目标三元组：`thumbv7a-pc-windows-msvc`

**需要：** Visual Studio Build Tools + **ARM 组件**

### 1. 安装工具链

```powershell
rustup target add thumbv7a-pc-windows-msvc
```

### 2. 编译

```powershell
cargo build --release --bin serial-debugger --target thumbv7a-pc-windows-msvc
```

### 3. 打包

```powershell
cargo packager --release --formats nsis --target thumbv7a-pc-windows-msvc
```

---

## Windows x86 32 位（在 x86_64 Windows 上交叉编译）

目标三元组：`i686-pc-windows-msvc`

**需要：** Visual Studio Build Tools + **x86 组件**（默认已包含）

### 1. 安装工具链

```powershell
rustup target add i686-pc-windows-msvc
```

### 2. 编译

```powershell
cargo build --release --bin serial-debugger --target i686-pc-windows-msvc
```

### 3. 打包

```powershell
cargo packager --release --formats nsis --target i686-pc-windows-msvc
```

---

## 安装与卸载

### .deb（Debian / Ubuntu）

```bash
sudo dpkg -i dist/*.deb
sudo dpkg -r serial-debugger
```

运行时依赖：`libc6 (>= 2.17)`、`libudev1`

### .pkg.tar.zst（Arch Linux）

```bash
sudo pacman -U dist/*.pkg.tar.zst
sudo pacman -R serial-debugger
```

运行时依赖：`gcc-libs`、`systemd`（提供 libudev）

### .rpm（Fedora / RHEL / openSUSE）

```bash
# Fedora / RHEL
sudo dnf install target/generate-rpm/*.rpm
sudo dnf remove serial-debugger

# openSUSE
sudo zypper install target/generate-rpm/*.rpm
sudo zypper remove serial-debugger
```

运行时依赖：`glibc`、`systemd-libs`（提供 libudev.so）

### AppImage（通用，免安装）

```bash
chmod +x dist/*.AppImage
./dist/*.AppImage
```

---

## 产物路径汇总

| 格式 | 产物路径 |
|------|----------|
| NSIS `.exe` | `dist/` |
| `.deb` | `dist/` |
| `.pkg.tar.zst` (pacman) | `dist/` |
| AppImage | `dist/` |
| `.rpm` | `target/generate-rpm/` |

---

## 升级版本号

修改以下两处保持一致：

1. `Cargo.toml` → `[package]` → `version`
2. `Cargo.toml` → `[package.metadata.packager]` → `version`

> `[package.metadata.generate-rpm]` 自动继承 `[package].version`，无需单独修改。
