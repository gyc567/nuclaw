#!/bin/bash

set -euo pipefail

NUCLAW_REPO="gyc567/nuclaw"
GITHUB_API="https://api.github.com/repos/${NUCLAW_REPO}/releases/latest"
SKIP_TESTS=false
FORCE_BUILD=false
QUIET_MODE=false

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${BLUE}[STEP]${NC} $1"; }
log_success() { echo -e "${MAGENTA}[OK]${NC} $1"; }

for arg in "$@"; do
    case $arg in
        --skip-tests) SKIP_TESTS=true ;;
        --force-build) FORCE_BUILD=true ;;
        -y|--yes|--quiet) QUIET_MODE=true ;;
    esac
done

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

get_latest_version() {
    curl -sSL "${GITHUB_API}" 2>/dev/null | grep -o '"tag_name": "[^"]*"' | head -1 | cut -d'"' -f4 | sed 's/^v//' || echo ""
}

download_prebuilt() {
    local version=$1 os=$2 arch=$3
    local target=""

    case "$os-$arch" in
        linux-x86_64)    target="x86_64-unknown-linux-gnu" ;;
        linux-arm64)     target="aarch64-unknown-linux-gnu" ;;
        macos-x86_64)    target="x86_64-apple-darwin" ;;
        macos-arm64)     target="aarch64-apple-darwin" ;;
        *)               return 1 ;;
    esac

    local filename="nuclaw-${target}.tar.gz"
    local url="https://github.com/${NUCLAW_REPO}/releases/download/v${version}/${filename}"
    local output="/tmp/${filename}"

    log_step "尝试下载预编译二进制: ${os}-${arch}"
    if curl -fSL --retry 3 --retry-delay 2 -o "$output" "$url" 2>/dev/null; then
        echo "$output"
        return 0
    fi
    return 1
}

install_prebuilt() {
    local tarball=$1 install_dir=$2

    mkdir -p "$install_dir"
    tar -xzf "$tarball" -C "$install_dir" --strip-components=1
    chmod +x "${install_dir}/nuclaw"
    rm -f "$tarball"

    log_success "预编译二进制安装完成"
}

check_rust() {
    command -v cargo &>/dev/null
}

install_rust() {
    log_step "安装 Rust 环境..."
    if ! command -v rustup &>/dev/null; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi
    log_info "Rust 版本: $(rustc --version)"
}

install_system_deps() {
    log_step "安装系统依赖..."

    if [[ "$(detect_os)" == "macos" ]]; then
        command -v brew &>/dev/null && brew install sqlite3 2>/dev/null || true
    else
        if command -v apt-get &>/dev/null; then
            sudo apt-get update -qq
            sudo apt-get install -y -qq build-essential libssl-dev pkg-config sqlite3 2>/dev/null || true
        elif command -v dnf &>/dev/null; then
            sudo dnf install -y -q gcc gcc-c++ openssl-devel pkg-config sqlite 2>/dev/null || true
        elif command -v yum &>/dev/null; then
            sudo yum install -y -q gcc gcc-c++ openssl-devel pkg-config sqlite 2>/dev/null || true
        fi
    fi
}

build_from_source() {
    local install_dir=$1
    log_warn "从源码构建 (预计需要 5-10 分钟)..."

    check_rust || install_rust
    install_system_deps

    local build_dir="/tmp/nuclaw-build"
    rm -rf "$build_dir"

    git clone --depth 1 "https://github.com/${NUCLAW_REPO}.git" "$build_dir"
    cd "$build_dir"

    cargo build --release
    mkdir -p "$install_dir"
    cp target/release/nuclaw "${install_dir}/"
    rm -rf "$build_dir"

    log_success "源码构建完成"
}

install_to_path() {
    local binary_path=$1

    log_step "安装到系统 PATH..."

    if sudo cp "$binary_path" "/usr/local/bin/nuclaw" && sudo chmod +x "/usr/local/bin/nuclaw"; then
        log_success "已安装到 /usr/local/bin/nuclaw"
        return 0
    fi

    mkdir -p "$HOME/.local/bin"
    cp "$binary_path" "$HOME/.local/bin/nuclaw"
    chmod +x "$HOME/.local/bin/nuclaw"

    log_success "已安装到 $HOME/.local/bin/nuclaw"
    log_info "请添加以下到 ~/.bashrc 或 ~/.zshrc:"
    echo '  export PATH="$HOME/.local/bin:$PATH"'
}

run_tests() {
    if [[ "$SKIP_TESTS" == true ]]; then
        log_info "跳过测试 (--skip-tests)"
        return 0
    fi

    log_step "运行测试..."
    cargo test --release || log_warn "部分测试失败"
}

verify_installation() {
    local binary_path=$1

    log_step "验证安装..."

    if [[ -x "$binary_path" ]]; then
        log_success "二进制可执行"
        "$binary_path" --version || true
        return 0
    fi

    log_error "验证失败"
    return 1
}

main() {
    echo -e "${CYAN}╔═══════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║           NuClaw 一键部署 (优化版)                      ║${NC}"
    echo -e "${CYAN}╚═══════════════════════════════════════════════════════════╝${NC}"

    local os=$(detect_os) arch=$(detect_arch)
    local install_dir="${NUCLAW_HOME:-${HOME}/.nuclaw}"

    if [[ "$os" == "unknown" ]]; then
        log_error "不支持的操作系统"
        exit 1
    fi

    log_info "目标平台: ${os}-${arch}"
    log_info "安装目录: ${install_dir}"
    echo ""

    mkdir -p "$install_dir"/{store,data,groups,logs}

    local binary_path=""

    if [[ "$FORCE_BUILD" != true ]]; then
        local version=$(get_latest_version)
        if [[ -n "$version" ]]; then
            log_info "最新版本: v${version}"

            local tarball=$(download_prebuilt "$version" "$os" "$arch" 2>/dev/null || echo "")
            if [[ -n "$tarball" ]]; then
                install_prebuilt "$tarball" "$install_dir"
                binary_path="${install_dir}/nuclaw"
            fi
        fi
    fi

    if [[ -z "$binary_path" ]] || [[ ! -x "$binary_path" ]]; then
        if [[ "$FORCE_BUILD" == true ]]; then
            log_warn "强制从源码构建 (--force-build)"
        else
            log_warn "预编译版本不可用，回退到源码构建"
        fi
        build_from_source "$install_dir"
        binary_path="${install_dir}/nuclaw"
    fi

    install_to_path "$binary_path"
    run_tests
    verify_installation "$binary_path"

    echo ""
    echo "==============================================================================="
    log_success "部署完成!"
    echo "==============================================================================="
    echo ""
    echo "使用方式:"
    echo "  nuclaw              # 启动服务"
    echo "  nuclaw --onboard    # 配置向导"
    echo "  nuclaw --help       # 查看帮助"
    echo ""
    echo "配置目录: ${install_dir}"
    echo ""
}

main "$@"
