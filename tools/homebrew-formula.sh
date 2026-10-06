#!/usr/bin/env bash
# Print a Homebrew formula for a release, using the prebuilt archives
# produced by .github/workflows/release.yml.
#
#   tools/homebrew-formula.sh v0.1.0 dist/ > aj-rs.rb
#
# DIR must contain autojump-<tag>-<target>.tar.gz for all four targets.
set -euo pipefail

tag=${1:?usage: $0 TAG DIR}
dir=${2:?usage: $0 TAG DIR}
version=${tag#v}
repo=https://github.com/danliustc/aj-rs

sha() {
    local f="$dir/autojump-$tag-$1.tar.gz"
    [[ -f $f ]] || { echo "missing $f" >&2; exit 1; }
    if command -v sha256sum >/dev/null; then
        sha256sum "$f" | cut -d' ' -f1
    else
        shasum -a 256 "$f" | cut -d' ' -f1
    fi
}

url() { echo "$repo/releases/download/$tag/autojump-$tag-$1.tar.gz"; }

cat <<EOF
class AjRs < Formula
  desc "Fast Rust port of autojump: a cd command that learns"
  homepage "$repo"
  version "$version"
  license "GPL-3.0-or-later"

  on_macos do
    on_arm do
      url "$(url aarch64-apple-darwin)"
      sha256 "$(sha aarch64-apple-darwin)"
    end
    on_intel do
      url "$(url x86_64-apple-darwin)"
      sha256 "$(sha x86_64-apple-darwin)"
    end
  end

  on_linux do
    on_arm do
      url "$(url aarch64-unknown-linux-musl)"
      sha256 "$(sha aarch64-unknown-linux-musl)"
    end
    on_intel do
      url "$(url x86_64-unknown-linux-musl)"
      sha256 "$(sha x86_64-unknown-linux-musl)"
    end
  end

  head do
    url "$repo.git", branch: "main"
    depends_on "rust" => :build
  end

  conflicts_with "autojump", because: "both install an \`autojump\` binary"

  def install
    if build.head?
      system "cargo", "install", *std_cargo_args
    else
      bin.install "autojump"
    end
  end

  def caveats
    <<~EOS
      Add the shell integration (provides j, jc, jo, jco) to your shell config:

        bash  (~/.bashrc):                 eval "\$(autojump --init bash)"
        zsh   (~/.zshrc, after compinit):  eval "\$(autojump --init zsh)"
        fish  (~/.config/fish/config.fish): autojump --init fish | source

      Data from the original autojump is picked up as is. Remove any old
      \`source .../autojump.sh\` line so the two don't both run.
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/autojump --version")
    ENV["AUTOJUMP_DATA_DIR"] = (testpath/"data").to_s
    (testpath/"projects/foo").mkpath
    system bin/"autojump", "--add", testpath/"projects/foo"
    assert_equal (testpath/"projects/foo").to_s, shell_output("#{bin}/autojump foo").strip
    assert_match "j()", shell_output("#{bin}/autojump --init bash")
  end
end
EOF
