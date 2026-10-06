# aj-rs

[autojump](https://github.com/wting/autojump) 的 Rust 移植版：一个会"学习"的 `cd`。

```sh
j proj        # 跳到你最常待的、名字含 proj 的目录
j code proj   # 多个关键词：按路径层级依次匹配
jc src        # 只在当前目录的子孙里找
jo docs       # 用文件管理器打开
```

## 为什么要移植

原版 autojump 是 Python 写的，**每次显示提示符**都会起一个 Python 进程去记录当前目录。
Rust 版是单个静态二进制，启动即退出：

| 操作（2000 条记录） | Python autojump | aj-rs |
| --- | --- | --- |
| 查询（无命中，三种策略全跑一遍） | ~72 ms | ~4.6 ms |
| `--add`（每次提示符都会执行） | ~52 ms | ~5 ms（大头是 fsync） |

## 安装

支持 **macOS**（Apple Silicon / Intel）和 **Linux**（Ubuntu 等，x86_64 / aarch64）。

**方式一：下载预编译二进制**（推荐，不需要 Rust）。从
[Releases](https://github.com/danliustc/aj-rs/releases) 下载对应平台的包，把 `autojump` 放进 `PATH`：

| 平台 | 文件 |
| --- | --- |
| macOS Apple Silicon | `autojump-<版本>-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `autojump-<版本>-x86_64-apple-darwin.tar.gz` |
| Linux x86_64 | `autojump-<版本>-x86_64-unknown-linux-musl.tar.gz` |
| Linux aarch64 | `autojump-<版本>-aarch64-unknown-linux-musl.tar.gz` |

Linux 包是静态链接（musl），不挑 glibc 版本，任何 Ubuntu 都能直接跑。
macOS 下首次运行如果被 Gatekeeper 拦截：`xattr -d com.apple.quarantine ./autojump`。

**方式二：从源码编译**，需要 Rust ≥ 1.89：

```sh
cargo install --git https://github.com/danliustc/aj-rs
```

> Ubuntu 用户注意：`apt install cargo` 装的 Rust 太旧（24.04 是 1.75），会编译失败。
> 请用 [rustup](https://rustup.rs) 安装 Rust，或直接用方式一。

然后在 shell 配置里加一行：

| Shell | 配置 |
| --- | --- |
| bash (`~/.bashrc`) | `eval "$(autojump --init bash)"` |
| zsh (`~/.zshrc`，放在 `compinit` 之后；macOS 默认 shell) | `eval "$(autojump --init zsh)"` |
| fish (`~/.config/fish/config.fish`) | `autojump --init fish \| source` |

macOS 自带的 `/bin/bash` 是 3.2，也支持。

**从原版迁移：零成本。** 数据文件路径和格式（`权重\t路径`）与原版完全一致，
装好后原来积累的 `autojump.txt` 直接可用。记得把原版的 `source .../autojump.sh` 删掉。

## 用法

```text
autojump [DIRECTORY ...]       打印最佳匹配（无匹配时打印 "."）
  -a, --add DIR                记录一次访问
  -i, --increase [W]           当前目录权重 +W（默认 10）
  -d, --decrease [W]           当前目录权重 -W（默认 15）
  --purge                      删掉已不存在的目录
  -s, --stat                   查看数据库
  --complete                   Tab 补全用
  --init SHELL                 输出 shell 集成脚本（bash / zsh / fish）
```

Tab 补全会给出 `proj__1__/path/to/proj` 形式的菜单，选中后 `j` 直接跳过去；
也可以手打 `j proj__2` 跳到第 2 个候选。

## 工作原理

**权重。** 每显示一次提示符（zsh/fish 是每次切换目录），当前目录权重按
`w = sqrt(w² + 10²)` 增长——常去的目录涨得快，但增长是次线性的，不会被单个目录垄断。
家目录永远不记录。

**匹配。** 候选按权重降序排列，依次用三种策略匹配，结果按顺序拼接、去重，
再过滤掉当前目录和已不存在的目录，取第一个：

1. **连续匹配**：关键词依次落在相邻的路径分量里，最后一个关键词必须在最后一个分量。
   `j foo bar` 命中 `/x/foo/bar`、`/x/myfoo/rebar`，不命中 `/x/foo/y/bar`。
2. **模糊匹配**：最后一个关键词和路径最后一段的相似度（difflib `SequenceMatcher.ratio()`）≥ 0.6，
   所以 `j dcuments` 也能到 `~/documents`。
3. **任意位置**：关键词按顺序出现在路径任意位置即可。

大小写是 smartcase：关键词全小写时不区分大小写，含大写字母时区分。

## 与原版的差异

匹配排序在随机生成的数据库和查询上与原版逐字节做过差分测试（数千个用例）。以下是有意为之的不同：

- **显式路径直通**：`j ..`、`j /tmp`、`j ./src` 直接跳过去，不查数据库（裸词 `j src` 仍查数据库）。
- **补全菜单去重**：原版同一路径可能被多个策略命中而在菜单里重复出现，这里去重。
- **`j foo__3` 无命中时**原版会崩溃（Python 异常），这里返回 `.`。
- **只跳目录**：原版用 `exists` 检查，会把已变成文件的旧路径当候选；这里用 `is_dir`。
- **并发安全**：多个 shell 同时写数据库时用文件锁串行化，原版会互相覆盖丢更新。
- **找不到时报错**：原版找不到时会 `cd .` 并打印一个红色的 `.`；这里提示 not found 并返回非零。
- **不需要 `AUTOJUMP_SOURCED`**：原版不 source 脚本就拒绝运行，这里不检查（脚本仍会设置它以兼容）。
- 额外支持 `AUTOJUMP_DATA_DIR` 环境变量覆盖数据目录。

暂不支持：tcsh、Windows 的 clink/cmd 脚本；非 UTF-8 路径会被有损转换。

## 数据位置

| 平台 | 路径 |
| --- | --- |
| Linux / BSD | `$XDG_DATA_HOME/autojump/autojump.txt`（默认 `~/.local/share/autojump/`） |
| macOS | `~/Library/autojump/autojump.txt` |
| Windows | `%APPDATA%\autojump\autojump.txt` |

旁边的 `autojump.txt.bak` 每 24 小时备份一次；主文件丢失时会自动从备份读。

## 许可

GPL-3.0-or-later，与原版 autojump 相同。
