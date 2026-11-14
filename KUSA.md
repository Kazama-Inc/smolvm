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
| `feat/windows-share-host-owner` | Windows 上用户共享带 `KRUN_VIRTIOFS_FLAG_HOST_OWNER`，主机文件报 uid 1000，agent 的 id 映射（`mount_owner`）把它映射给工作负载用户；libkrun 不认这个 flag 时照旧挂载 | 暂不提上游 |
| `feat/windows-rootfs-lx-symlinks` | Windows 上 agent rootfs 和镜像层的符号链接存成 WSL 式 LX 链接（`smolvm_pack::lx_symlink`），不再需要开发人员模式或管理员；rootfs 改在进程内解包（不用系统 `tar`）；摘要、rootfs 仓库拷贝、checkpoint 解包、pack 打包都能读写 LX 链接 | 暂不提上游 |
| （子模块） | `libkrun` 指向 `Kazama-Inc/libkrun` 的 `kusa`：Windows virtiofs 的 junction 穿越和「文件名带 `\`、`:` 写出共享目录」两处修复 | 见 libkrun 的 `KUSA.md` |

## Windows 加固（设计 10.5 的 F2）进度

| 项 | 状态 |
|---|---|
| junction 穿越（否决级） | 已修（libkrun `fix/windows-virtiofs-escapes`）。同时修了文件名带 `\`、`:` 写出共享目录（同为否决级，原型没发现）。按上游 `SECURITY.md` 走 GitHub 私密漏洞报告，报告待发 |
| 从源码编 `krun.dll` | 已通（MSVC + musl 目标 + `rust-lld`）。定用 MSVC（11-14 用户确认）。原以为 MSVC 版偶发「流式命令 1 秒内没起来」，查明是测试的心跳只精确到秒（smol `fix/e2e-heartbeat-counter`），GNU 版同样会挂，与编法无关 |
| 状态目录可配置 + AF_UNIX 自检 | 已做（`feat/windows-state-dir`） |
| 父进程联动 | 已做（`feat/windows-boot-helper` 的看门狗 + SDK 的作业对象），有 e2e |
| rootfs 解包不依赖符号链接权限 | 已修（`feat/windows-rootfs-lx-symlinks`）。链接存成 LX 重解析点：不要特权，目标原样保存（`/usr/bin/x` 不会变成 `C:\usr\bin\x`），Windows 程序不跟，libkrun 按符号链接呈现给 guest。本机实测：重新解包的 rootfs 383 个链接全是 LX，Windows e2e 全过。本机开着开发人员模式，「没开开发人员模式」的整机验证仍要另一台机器；新代码路径不再调用原生符号链接 API |
| 取消要 5 秒 | 已修（libkrun `fix/vsock-unix-prompt-close`）。原因：Windows 上宿主对自己的 AF_UNIX 句柄 `shutdown` 叫不醒同一 socket 上阻塞的 `recv`，只能等 libkrun 关主机侧 socket，而 unix 代理要等 reaper 的 5 秒宽限期才关。现在 guest 一关连接就 `shutdown` 主机侧 socket。实测（MSVC，开不开网络一样）：取消 2～14 ms 返回，下一条命令约 10 ms；`ExecOptions.Timeout` 2 秒准时返回 124，原型记录的「晚 1～3 秒」也没了 |
| 内存按上限提交、高段不归还 | 已做（libkrun `feat/whp-lazy-guest-ram`，默认开，`SMOLVM_WHP_LAZY_RAM=0` 关）。本机实测 2 GiB 档：开机后私有提交 194 MB（原来 2127 MB，即上限加 80 MB）；guest 填 1.2 GB 后 1210 MB；释放后回落到 257～758 MB（看 balloon 回收进度；原来一直 2133 MB 不降）。首次填 1 GB 用 0.28 秒（原来 0.21 秒；如果不预填、只按 64 KiB 提交，要 4.4 秒）。压力测试（填满、释放、马上做大量 virtio-fs 读和 vsock 输出，4 轮）每轮校验都一致；Windows e2e 全过。设计 10.4 里「Windows 在 F2 修好内存之前封顶 3 GiB」这一条可以重新评估 |
| 建机偶发卡住（另案） | 压力配置（2 vCPU、2 GiB、挂载目录里有 96 MB 文件）下，原版（整块提交）干净跑 6 次卡 1 次：guest agent 已经起来、一直在接受连接，VMM CPU 为 0，主机的建机请求一直等不到回应。lazy 版 6 次没出现。简单配置 16 次都没出现。原因未查 |
| 非 root 写 `/workspace` | 已修（smolvm `feat/windows-share-host-owner` + libkrun `feat/windows-virtiofs-host-owner`）。工作区共享报 uid/gid 1000；uid 1000 的工作负载直接能写，别的非 root 用户靠 agent 建容器时的 id 映射（按 `MachineConfig.User` 或镜像的 `USER`；单条命令的 `ExecOptions.User` 不映射，与上游在 macOS 上的行为相同）。e2e：smol `feat/windows-workspace-owner-test`，修前两种情况都 Permission denied |
| 非管理员（无开发人员模式）、家庭版、Windows 10 | 需要别的机器 |

补丁暂不提上游（11-14 用户决定）。上游三个仓库的 CI 都有 `no-ai-attribution` 检查，拒绝带 AI 署名的提交；本 fork 的提交带 `Co-Authored-By` 署名，将来要提时另行处理。

## 构建（Windows）

一律 MSVC 工具链：`rust-toolchain.toml` 写的是 `stable`，在缺省主机为 gnu 的机器上会选到 gnu，编 `getrandom` 时缺 `dlltool`。

```bash
RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc cargo build --release --bin smolvm   # -> smolvm.exe，打包时改名 smol-vmm.exe
```

运行时包的组成和 CI 见 `.github/workflows/kusa-windows.yml`。GitHub 托管的 `windows-latest` runner 有 WHP（嵌套虚拟化；11-14 run 43544630569 实测 `Available: true`），Windows e2e 和越权集在 CI 上真起虚拟机跑；工作流仍先探测，没有时只出警告。

## 测试环境的坑

- 从 MSIX 应用（例如 Claude 桌面端）里起的进程继承 AppData 重定向，`%LOCALAPPDATA%\smolvm` 下 AF_UNIX 连不上。设 `SMOLVM_DATA_DIR` 到 AppData 以外的短目录（例如 `%TEMP%` 下）；不设时现在会在启动前直接报错说明。
