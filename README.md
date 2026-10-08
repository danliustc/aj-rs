# aj-rs

[autojump](https://github.com/wting/autojump) 的 Rust 移植版：一个会"学习"的 `cd`。

```sh
brew install danliustc/tap/aj-rs
echo 'eval "$(autojump --init zsh)"' >> ~/.zshrc    # bash / fish 见下文
```

重开终端后：

```sh
j proj        # 跳到你最常待的、名字含 proj 的目录
j code proj   # 多个关键词：按路径层级依次匹配
jc src        # 只在当前目录的子孙里找
jo docs       # 用文件管理器打开
jco docs      # 同 jc，但用文件管理器打开
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
安装分三步：**装二进制 → 在 shell 配置里加一行 → 重开终端**。只做第一步的话，
只有 `autojump` 命令，没有 `j`，也不会记录你去过的目录。

### 第 1 步：安装 `autojump` 二进制

任选一种。

**Homebrew**（macOS / Linux，推荐）：

```sh
brew install danliustc/tap/aj-rs
```

装的是预编译二进制，不需要 Rust。命令名仍是 `autojump`，所以和 Homebrew 官方的 `autojump` 互斥，
装过的话先 `brew uninstall autojump`。想跟 main 分支走：`brew install --HEAD danliustc/tap/aj-rs`（需要 Rust）。

**Cargo**，需要 Rust ≥ 1.89：

```sh
cargo install --git https://github.com/danliustc/aj-rs    # main 分支最新代码
cargo install aj-rs                                       # crates.io 发布版（尚未发布，暂时用上一行）
```

crate 叫 `aj-rs`，装出来的命令叫 `autojump`，在 `~/.cargo/bin` 下，确保它在 `PATH` 里。

> Ubuntu 用户注意：`apt install cargo` 装的 Rust 太旧（24.04 是 1.75），会编译失败。
> 请用 [rustup](https://rustup.rs) 安装 Rust，或用其他方式。

**下载预编译二进制**。从
[Releases](https://github.com/danliustc/aj-rs/releases) 下载对应平台的包，解压后把 `autojump` 放进 `PATH`：

| 平台 | 文件 |
| --- | --- |
| macOS Apple Silicon | `autojump-<版本>-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `autojump-<版本>-x86_64-apple-darwin.tar.gz` |
| Linux x86_64 | `autojump-<版本>-x86_64-unknown-linux-musl.tar.gz` |
| Linux aarch64 | `autojump-<版本>-aarch64-unknown-linux-musl.tar.gz` |

```sh
tar xzf autojump-v0.1.0-aarch64-apple-darwin.tar.gz
mkdir -p ~/.local/bin && mv autojump-v0.1.0-aarch64-apple-darwin/autojump ~/.local/bin/
```

Linux 包是静态链接（musl），不挑 glibc 版本，任何 Ubuntu 都能直接跑。
macOS 下首次运行如果被 Gatekeeper 拦截：`xattr -d com.apple.quarantine ~/.local/bin/autojump`。

装好后确认一下：

```sh
autojump --version    # autojump v0.1.0 (Rust port)
```

### 第 2 步：在 shell 配置里加一行

`j` / `jc` / `jo` / `jco` 这几个命令和"每次切换目录自动记录"都是这一行提供的，**必须加**。
不确定自己用哪个 shell，执行 `echo $SHELL`（macOS 默认是 zsh）。

**zsh**（`~/.zshrc`）：

```sh
eval "$(autojump --init zsh)"
```

要放在 `compinit` **之后**，否则 Tab 补全不生效。用 oh-my-zsh 的话，放在 `source $ZSH/oh-my-zsh.sh`
那行之后（oh-my-zsh 会替你执行 `compinit`），并且把 `plugins=(...)` 里的 `autojump` 删掉，
那个插件加载的是原版。没用任何框架、`.zshrc` 里也没有 `compinit` 的话，把下面两行一起加到文件末尾：

```sh
autoload -Uz compinit && compinit
eval "$(autojump --init zsh)"
```

**bash**（`~/.bashrc`，macOS 上如果是登录 shell 则是 `~/.bash_profile`）：

```sh
eval "$(autojump --init bash)"
```

macOS 自带的 `/bin/bash` 3.2 也支持。

**fish**（`~/.config/fish/config.fish`）：

```fish
autojump --init fish | source
```

也可以直接用命令追加，比如 zsh：

```sh
echo 'eval "$(autojump --init zsh)"' >> ~/.zshrc
```

### 第 3 步：重开终端，验证

重开一个终端窗口（或 `exec zsh` / `exec bash` / `exec fish`），然后：

```sh
type j                # 应显示 j 是一个 shell 函数
cd ~/Code/some-project && cd ~
j some                # 跳回 ~/Code/some-project
autojump -s           # 查看已记录的目录和权重
```

`j` 只能跳到**去过的**目录：刚装好时数据库是空的，正常使用一阵子，常去的目录就都记下了。
从原版 autojump 迁移的话不用等，见下文。

### 常见问题

- **`j: command not found`**：第 2 步那行没加，或者加了但没重开终端。也可能加错了文件，
  比如 zsh 用户加到了 `~/.bashrc`。
- **`j` 能用但 Tab 没有补全**（zsh）：`eval` 那行放到了 `compinit` 前面，挪到后面去。
- **`autojump: directory 'xxx' not found`**：数据库里还没有匹配的目录，先 `cd` 进去一次。
- **和原版冲突**：删掉原版的 `source .../autojump.sh`（或 `[ -f .../autojump.sh ] && . ...` 之类）那行、
  oh-my-zsh 的 `autojump` 插件，以及 `brew uninstall autojump`。两套同时加载时，后加载的那个生效。

### 从原版迁移

**零成本。** 数据文件路径和格式（`权重\t路径`）与原版完全一致，
装好后原来积累的 `autojump.txt` 直接可用，`j` 立刻就能跳到以前常去的目录。
只要按上面的"和原版冲突"把原版的加载方式删掉就行。

### 升级和卸载

| 安装方式 | 升级 | 卸载 |
| --- | --- | --- |
| Homebrew | `brew upgrade aj-rs` | `brew uninstall aj-rs` |
| Cargo | 重新执行安装命令，加 `--force` | `cargo uninstall aj-rs` |
| 预编译二进制 | 下载新版本覆盖 | 删掉 `autojump` 文件 |

卸载后记得删掉 shell 配置里第 2 步加的那行。数据文件（见"数据位置"）不会被删，要清理请手动删除。

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

## 发布新版本（维护者）

1. 改 `Cargo.toml` 里的 `version`，提交。
2. 打 tag 并推送：`git tag v0.2.0 && git push origin v0.2.0`。tag 必须和 `version` 一致，否则 Release 工作流直接失败。
3. Release 工作流会构建四个平台的二进制、挂到 GitHub Release，并生成 Homebrew formula `aj-rs.rb` 一起挂上。
   配置了 `HOMEBREW_TAP_TOKEN` secret（对 `danliustc/homebrew-tap` 有写权限的 token）时，会自动提交到 tap；
   没配就手动把 `aj-rs.rb` 放进 tap 仓库的 `Formula/` 目录。
4. 发布到 crates.io：`cargo publish`（先 `cargo login`）。

## 许可

GPL-3.0-or-later，与原版 autojump 相同。
