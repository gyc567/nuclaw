#!/bin/bash

set -euo pipefail

NUCLAW_REPO="gyc567/nuclaw"
GITHUB_API="https://api.github.com/repos/${NUCLAW_REPO}/releases/latest"
INSTALL_VERSION="${INSTALL_VERSION:-latest}"
QUIET_MODE=false

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
NC='\033[0m'

for arg in "$@"; do
    case $arg in
        -y|--yes|--quiet) QUIET_MODE=true ;;
        -h|--help)
            echo "用法: curl -sSL https://raw.githubusercontent.com/gyc567/nuclaw/main/install.sh | bash"
            echo ""
            echo "选项:"
            echo "  -y, --yes, --quiet  自动确认所有提示"
            echo "  -h, --help         显示此帮助"
            exit 0
            ;;
    esac
done

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${BLUE}[STEP]${NC} $1"; }
log_success() { echo -e "${MAGENTA}[OK]${NC} $1"; }

confirm() {
    if [ "$QUIET_MODE" = true ]; then return 0; fi
    local prompt="$1 (y/N): "
    read -p "$(echo -e "${YELLOW}[PROMPT]${NC} $prompt")" choice
    case "$choice" in
        y|Y|yes|Yes ) return 0 ;;
        * ) return 1 ;;
    esac
}

detect_os() {
    case "$(uname -s)" in
        Linux*)     echo "linux" ;;
        Darwin*)    echo "macos" ;;
        *)          echo "unknown" ;;
    esac
}

detect_arch() {
    local arch=$(uname -m)
    case "$arch" in
        x86_64)     echo "x86_64" ;;
        aarch64|arm64) echo "arm64" ;;
        *)          echo "$arch" ;;
    esac
}

get_nuclaw_home() { echo "${NUCLAW_HOME:-${HOME}/.nuclaw}"; }

get_latest_version() {
    curl -sSL "${GITHUB_API}" 2>/dev/null | grep -o '"tag_name": "[^"]*"' | head -1 | cut -d'"' -f4 | sed 's/^v//' || echo ""
}

download_binary() {
    local version=$1 os=$2 arch=$3
    local filename=""
    local target=""

    case "$os" in
        linux)
            case "$arch" in
                x86_64)  target="x86_64-unknown-linux-gnu" ;;
                arm64)   target="aarch64-unknown-linux-gnu" ;;
                *)       return 1 ;;
            esac
            ;;
        macos)
            target="${arch}-apple-darwin"
            ;;
        *)  return 1 ;;
    esac

    filename="nuclaw-${target}.tar.gz"
    local url="https://github.com/${NUCLAW_REPO}/releases/download/v${version}/${filename}"
    local output="/tmp/${filename}"

    log_step "下载预编译二进制 (重试 3 次)..."

    local max_retries=3
    local retry=0
    while [[ $retry -lt $max_retries ]]; do
        if curl -fSL --retry 3 --retry-delay 2 -o "$output" "$url" 2>/dev/null; then
            if tar -tzf "$output" &>/dev/null; then
                echo "$output"
                return 0
            fi
        fi
        ((retry++))
        log_warn "下载失败，重试 $retry/$max_retries..."
        sleep 2
    done

    return 1
}

build_from_source() {
    local nuclaw_home=$1
    log_warn "预编译版本不可用"
    if ! confirm "是否从源码构建? (需要 5-10 分钟)"; then
        log_error "安装取消"
        exit 1
    fi

    log_step "从源码构建..."
    if ! command -v cargo &>/dev/null; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi

    local build_dir="/tmp/nuclaw-build"
    rm -rf "$build_dir"
    git clone --depth 1 "https://github.com/${NUCLAW_REPO}.git" "$build_dir"
    cd "$build_dir"
    cargo build --release
    mkdir -p "$nuclaw_home"
    cp target/release/nuclaw "${nuclaw_home}/"
    log_success "源码构建完成"
}

setup_path() {
    local nuclaw_home=$1
    if [[ ":$PATH:" == *":${nuclaw_home}:"* ]]; then return 0; fi

    if confirm "是否添加 NuClaw 到 PATH?"; then
        local shell_rc=""
        case "$SHELL" in
            */zsh)  shell_rc="$HOME/.zshrc" ;;
            */bash) shell_rc="$HOME/.bashrc" ;;
            *)      shell_rc="$HOME/.profile" ;;
        esac

        if [[ -n "$shell_rc" ]]; then
            echo -e "\n# NuClaw PATH\nexport PATH=\"\$PATH:${nuclaw_home}\"" >> "$shell_rc"
            log_success "已添加到 ${shell_rc}，请运行: source ${shell_rc}"
        fi
    fi
}

main() {
    echo -e "${CYAN}╔═══════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║           NuClaw Setup Wizard (v1.2)                    ║${NC}"
    echo -e "${CYAN}╚═══════════════════════════════════════════════════════════╝${NC}"

    local os=$(detect_os) arch=$(detect_arch) nuclaw_home=$(get_nuclaw_home)
    if [[ "$os" == "unknown" ]]; then log_error "不支持的操作系统"; exit 1; fi

    log_info "目标: ${os}-${arch} | 目录: ${nuclaw_home}"

    local version=$(get_latest_version)
    if [[ -z "$version" ]]; then
        build_from_source "$nuclaw_home"
    else
        log_info "最新版本: v${version}"
        local tarball=$(download_binary "$version" "$os" "$arch" 2>/dev/null || echo "")
        if [[ -n "$tarball" ]]; then
            mkdir -p "$nuclaw_home"
            tar -xzf "$tarball" -C "$nuclaw_home" --strip-components=1
            chmod +x "${nuclaw_home}/nuclaw"
            rm -f "$tarball"
            log_success "二进制安装完成"
        else
            build_from_source "$nuclaw_home"
        fi
    fi

    mkdir -p "${nuclaw_home}/"{store,data,groups,logs,skills}

    if confirm "是否安装为系统服务?"; then
        if [[ "$os" == "macos" ]]; then
            log_info "macOS LaunchAgent 设置..."
        fi
    fi

    setup_path "$nuclaw_home"

    echo -e "\n${GREEN}安装完成!${NC}"

    if confirm "是否启动配置向导?"; then
        "${nuclaw_home}/nuclaw" --onboard
    else
        log_info "后续配置，运行: ${nuclaw_home}/nuclaw --onboard"
    fi
}

main "$@"
