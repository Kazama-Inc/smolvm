# Kazama-Inc/smolvm：相对上游的差异

这是 `smol-machines/smolvm`（引擎）的 fork，给 Kusa 桌面端的本机执行环境（`box` 档）用。设计依据：`Kazama-Inc/kusa-desktop` 的 `docs/DESIGN.md` 第 10.5 节。一共三个 fork：

| 仓库 | 上游 | 用途 |
|---|---|---|
| `Kazama-Inc/smolvm`（本仓库） | `smol-machines/smolvm` | 引擎、`smol-vmm` 启动辅助程序、CI |
| `Kazama-Inc/smol` | `smol-machines/smol` | 只用 `sdk/go`；检出在本仓库的 `smol/` 目录下（Go SDK 的 Rust 部分按相对路径依赖引擎） |
| `Kazama-Inc/libkrun` | `smol-machines/libkrun` | 本仓库的 `libkrun` 子模块；Windows 的 `krun.dll` 从这里源码编 |

libkrunfw 不 fork：`libkrunfw.dll` 用本仓库 `lib/windows-x86_64/` 里上游提交的二进制，按 `libkrun.provenance` 记录的来源核对。

## 分支

| 分支 | 内容 |
|---|---|
| `main` | 跟上游的**发布标签**，不追每个提交。现在 = `v1.34.4`（`ed827014`） |
| `kusa` | `main` 加下表的补丁、`libkrun` 子模块指向 `Kazama-Inc/libkrun` 的 `kusa`、我们的 CI。每次合上游后重做 |
| `feat/*`、`fix/*` | 每个补丁一个分支，从 `main` 拉，能单独提给上游 |

每两周看一次上游的发布说明，有安全修复当周合。升级步骤：`main` 快进到新标签 → 各 `feat/`、`fix/` 分支变基 → 重做 `kusa` → libkrun fork 同样处理（见它的 `KUSA.md`）→ CI 全绿、Windows e2e 和越权集过了再给主仓库换版本。

## 补丁

目标是这张表只减不增：每条都以 PR 提给上游，合进去一条删一条。

| 分支 | 内容 | 上游状态 |
|---|---|---|
| `feat/windows-boot-helper` | Windows 上启动辅助程序的父进程看门狗（SDK 嵌入时宿主被杀，虚拟机随之退出，先请 guest 干净关机；原来 Windows 上没有，虚拟机会成孤儿）；`main()` 一开始 `SetDefaultDllDirectories`，不从当前目录和 PATH 加载 DLL | 草稿，未提 |
| （子模块） | `libkrun` 指向 `Kazama-Inc/libkrun` 的 `kusa`：Windows virtiofs 的 junction 穿越和「文件名带 `\`、`:` 写出共享目录」两处修复 | 见 libkrun 的 `KUSA.md` |

## 构建（Windows）

一律 MSVC 工具链：`rust-toolchain.toml` 写的是 `stable`，在缺省主机为 gnu 的机器上会选到 gnu，编 `getrandom` 时缺 `dlltool`。

```bash
RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc cargo build --release --bin smolvm   # -> smolvm.exe，打包时改名 smol-vmm.exe
```

运行时包的组成和 CI 见 `.github/workflows/kusa-windows.yml`。

## 测试环境的坑

- 从 MSIX 应用（例如 Claude 桌面端）里起的进程继承 AppData 重定向，`%LOCALAPPDATA%\smolvm` 下 AF_UNIX 连不上，表现是「agent did not become ready within 40 seconds」。在普通终端里跑。
