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
| `main` | GitHub fork 时带过来的上游提交（`eb26336f`），**原样不动、不作基线**：组织规则集管所有仓库的 `main`，禁止强推、只能走 PR 合入，钉不到发布标签 |
| `kusa` | **发布基线**：上游发布标签（现在 `v1.34.4` = `ed827014`）加下表的补丁、`libkrun` 子模块指向 `Kazama-Inc/libkrun` 的 `kusa`、我们的 CI。每次合上游后重做 |
| `feat/*`、`fix/*` | 每个补丁一个分支，从基线标签拉（不是从 `main`），能单独提给上游 |

每两周看一次上游的发布说明，有安全修复当周合。升级步骤：各 `feat/`、`fix/` 分支变基到新标签 → 从新标签重做 `kusa`（`kusa` 不受规则集限制，可以重写） → libkrun fork 同样处理（见它的 `KUSA.md`）→ CI 全绿、Windows e2e 和越权集过了再给主仓库换版本。

## 补丁

目标是这张表只减不增：每条都以 PR 提给上游，合进去一条删一条。

| 分支 | 内容 | 上游状态 |
|---|---|---|
| `feat/windows-boot-helper` | Windows 上启动辅助程序的父进程看门狗（SDK 嵌入时宿主被杀，虚拟机随之退出，先请 guest 干净关机；原来 Windows 上没有，虚拟机会成孤儿）；`main()` 一开始 `SetDefaultDllDirectories`，不从当前目录和 PATH 加载 DLL | 暂不提上游 |
| `feat/windows-state-dir` | Windows 上 `SMOLVM_DATA_DIR` 生效（`smolvm_pack::host_dirs`，各 crate 的 `dirs::` 调用都经它）；启动虚拟机前对状态目录做 AF_UNIX 自检（含 108 字节路径上限），失败直接报原因，不再等 40 秒超时 | 暂不提上游 |
| （子模块） | `libkrun` 指向 `Kazama-Inc/libkrun` 的 `kusa`：Windows virtiofs 的 junction 穿越和「文件名带 `\`、`:` 写出共享目录」两处修复 | 见 libkrun 的 `KUSA.md` |

## Windows 加固（设计 10.5 的 F2）进度

| 项 | 状态 |
|---|---|
| junction 穿越（否决级） | 已修（libkrun `fix/windows-virtiofs-escapes`）。同时修了文件名带 `\`、`:` 写出共享目录（同为否决级，原型没发现）。按上游 `SECURITY.md` 走 GitHub 私密漏洞报告，报告待发 |
| 从源码编 `krun.dll` | 已通（MSVC + musl 目标 + `rust-lld`）。定用 MSVC（11-14 用户确认）。已知 MSVC 版偶发「流式命令 1 秒内没起来」，上游 GNU 版没有，原因未查清 |
| 状态目录可配置 + AF_UNIX 自检 | 已做（`feat/windows-state-dir`） |
| 父进程联动 | 已做（`feat/windows-boot-helper` 的看门狗 + SDK 的作业对象），有 e2e |
| rootfs 解包不依赖符号链接权限 | 未做 |
| 开网络时取消要 5 秒 | 未做。线索：取消靠对克隆的 AF_UNIX 句柄 `shutdown(Both)`（`src/embedded/exec.rs`），Windows 上未必及时叫醒阻塞的读；11-14 不开网络时也碰到一次取消卡死 |
| 内存按上限提交、高段不归还 | 未做 |
| 非 root 写 `/workspace` | 未做。原因：agent 只在共享根属于非 root 时做 id 映射（`crates/smolvm-agent/src/mount_owner.rs`），而 Windows 服务端把所有文件报成 uid 0；要给每个共享传一个缺省属主（工作区共享报主机用户，guest rootfs 共享仍报 root），涉及 smolvm → libkrun 的接口。先在镜像里用 root 跑 |
| 非管理员（无开发人员模式）、家庭版、Windows 10 | 需要别的机器 |

补丁暂不提上游（11-14 用户决定）。上游三个仓库的 CI 都有 `no-ai-attribution` 检查，拒绝带 AI 署名的提交；本 fork 的提交带 `Co-Authored-By` 署名，将来要提时另行处理。

## 构建（Windows）

一律 MSVC 工具链：`rust-toolchain.toml` 写的是 `stable`，在缺省主机为 gnu 的机器上会选到 gnu，编 `getrandom` 时缺 `dlltool`。

```bash
RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc cargo build --release --bin smolvm   # -> smolvm.exe，打包时改名 smol-vmm.exe
```

运行时包的组成和 CI 见 `.github/workflows/kusa-windows.yml`。

## 测试环境的坑

- 从 MSIX 应用（例如 Claude 桌面端）里起的进程继承 AppData 重定向，`%LOCALAPPDATA%\smolvm` 下 AF_UNIX 连不上。设 `SMOLVM_DATA_DIR` 到 AppData 以外的短目录（例如 `%TEMP%` 下）；不设时现在会在启动前直接报错说明。
