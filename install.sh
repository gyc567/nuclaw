#!/bin/bash

#===============================================================================
# NuClaw 一键安装脚本
#
# 功能:
#   - 检测系统架构，下载预编译二进制
#   - 回退到源码构建（如果需要）
#   - 配置 ~/.nuclaw/ 目录结构
#   - 设置 launchd 开机自启动 (macOS)
#
# 使用方法:
#   curl -sSL https://raw.githubusercontent.com/gyc567/nuclaw/main/install.sh | bash
#   或
#   ./install.sh
#
#===============================================================================

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# 常量
NUCLAW_REPO="gyc567/nuclaw"
NUCLAW_HOME="${HOME}/.nuclaw"
GITHUB_API="https://api.github.com/repos/${NUCLAW_REPO}/releases/latest"

# 日志函数
log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${BLUE}[STEP]${NC} $1"; }

# 检测架构
detect_architecture() {
    local arch
    arch=$(uname -m)
    case "$arch" in
        x86_64)
            echo "x86_64-apple-darwin"
            ;;
        arm64|aarch64)
            echo "aarch64-apple-darwin"
            ;;
        *)
            log_error "不支持的架构: $arch"
            exit 1
            ;;
    esac
}

# 检测操作系统
detect_os() {
    if [[ "$OSTYPE" == "darwin"* ]]; then
        echo "macOS"
    elif [[ "$OSTYPE" == "linux"* ]]; then
        echo "Linux"
    else
        log_error "不支持的操作系统: $OSTYPE"
        exit 1
    fi
}

# 获取最新版本号
get_latest_version() {
    curl -sSL "${GITHUB_API}" | grep -o '"tag_name": "[^"]*"' | cut -d'"' -f4
}

# 下载预编译二进制
download_binary() {
    local arch=$1
    local version
    version=$(get_latest_version)

    if [[ -z "$version" ]]; then
        log_warn "无法获取最新版本，回退到源码构建"
        return 1
    fi

    local filename="nuclaw-${arch}.tar.gz"
    local url="https://github.com/${NUCLAW_REPO}/releases/download/${version}/${filename}"

    log_step "下载 NuClaw v${version} (${arch})..."
    log_info "URL: ${url}"

    # 下载
    if curl -sSL -o "/tmp/${filename}" "$url"; then
        log_info "下载完成: /tmp/${filename}"
        echo "/tmp/${filename}"
        return 0
    else
        log_warn "下载失败"
        rm -f "/tmp/${filename}"
        return 1
    fi
}

# 安装二进制到 NUCLAW_HOME
install_binary() {
    local tarball=$1
    local arch=$2

    log_step "安装到 ${NUCLAW_HOME}..."

    # 创建目录
    mkdir -p "${NUCLAW_HOME}"

    # 解压
    tar -xzf "$tarball" -C "${NUCLAW_HOME}"
    chmod +x "${NUCLAW_HOME}/nuclaw"

    log_info "安装完成: ${NUCLAW_HOME}/nuclaw"
    rm -f "$tarball"
}

# 设置 launchd 配置 (macOS)
setup_launchd() {
    log_step "配置 launchd 开机自启动..."

    local plist_dir="${HOME}/Library/LaunchAgents"
    local plist_file="${plist_dir}/com.nuclaw.agent.plist"

    mkdir -p "$plist_dir"

    # 创建 launchd plist
    cat > "$plist_file" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.nuclaw.agent</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>NUCLAW_HOME</key>
        <string>${NUCLAW_HOME}</string>
    </dict>
    <key>ProgramArguments</key>
    <array>
        <string>${NUCLAW_HOME}/nuclaw</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>${NUCLAW_HOME}/logs/launchd.log</string>
    <key>StandardErrorPath</key>
    <string>${NUCLAW_HOME}/logs/launchd.err</string>
</dict>
</plist>
EOF

    # 加载 launchd 配置
    launchctl load "$plist_file" 2>/dev/null || true

    log_info "launchd 配置已创建: ${plist_file}"
}

# 源码构建（回退选项）
build_from_source() {
    log_step "从源码构建..."

    # 检测 Rust
    if ! command -v rustc &> /dev/null; then
        log_step "安装 Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
        export PATH="$HOME/.cargo/bin:$PATH"
    fi

    # 克隆或更新项目
    if [[ -d "nuclaw" ]]; then
        cd nuclaw
        git pull origin main 2>/dev/null || true
    else
        git clone https://github.com/${NUCLAW_REPO}.git
        cd nuclaw
    fi

    # 构建
    cargo build --release

    # 安装到 NUCLAW_HOME
    mkdir -p "${NUCLAW_HOME}"
    cp target/release/nuclaw "${NUCLAW_HOME}/"

    log_info "源码构建完成"
}

# 创建目录结构
setup_directories() {
    log_step "创建运行时目录..."

    mkdir -p "${NUCLAW_HOME}"/{store,data,groups,logs}

    log_info "目录结构已创建"
}

# 创建初始配置
create_initial_config() {
    log_step "创建初始配置..."

    local config_file="${NUCLAW_HOME}/config.json"

    if [[ ! -f "$config_file" ]]; then
        cat > "$config_file" << 'EOF'
{
  "version": "1.0.0",
  "settings": {
    "assistant_name": "Andy",
    "timezone": "UTC",
    "container_timeout_ms": 300000
  }
}
EOF
        log_info "配置文件已创建: ${config_file}"
    fi
}

# 显示使用说明
show_usage() {
    echo ""
    echo "==============================================================================="
    echo "  NuClaw 安装完成!"
    echo "==============================================================================="
    echo ""
    echo "安装位置: ${NUCLAW_HOME}"
    echo ""
    echo "使用方式:"
    echo "  ${NUCLAW_HOME}/nuclaw              # 启动服务"
    echo "  ${NUCLAW_HOME}/nuclaw --help       # 查看帮助"
    echo "  ${NUCLAW_HOME}/nuclaw --auth       # 认证流程"
    echo ""
    echo "目录说明:"
    echo "  ${NUCLAW_HOME}/store/    - SQLite 数据库和认证文件"
    echo "  ${NUCLAW_HOME}/data/     - 运行时数据 (会话、IPC)"
    echo "  ${NUCLAW_HOME}/groups/   - 群组 CLAUDE.md 文件"
    echo "  ${NUCLAW_HOME}/logs/     - 日志文件"
    echo ""
    echo "macOS 开机自启动: 已配置"
    echo ""
    echo "后续步骤:"
    echo "  1. 配置 WhatsApp 认证 (设置 WHATSAPP_MCP_URL)"
    echo "  2. 注册群组"
    echo ""
}

# 主函数
main() {
    echo ""
    echo "==============================================================================="
    echo "  NuClaw 一键安装脚本"
    echo "  Rust 版本的个人 Claude 助手"
    echo "==============================================================================="
    echo ""

    local os
    local arch
    local tarball

    os=$(detect_os)
    arch=$(detect_architecture)

    log_info "检测到系统: ${os} (${arch})"
    echo ""

    # 尝试下载预编译二进制
    if tarball=$(download_binary "$arch"); then
        install_binary "$tarball" "$arch"
    else
        build_from_source
    fi

    setup_directories
    create_initial_config

    # macOS 启动配置
    if [[ "$os" == "macOS" ]]; then
        setup_launchd
    fi

    echo ""
    show_usage

    log_info "安装成功!"
}

main "$@"
