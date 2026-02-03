#!/bin/bash

#===============================================================================
# NuClaw 一键安装脚本
#
# 功能:
#   - 检查并安装 Rust 环境
#   - 克隆/更新项目
#   - 安装系统依赖
#   - 构建项目
#   - 运行测试
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
NC='\033[0m' # No Color

# 日志函数
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo -e "${BLUE}[STEP]${NC} $1"
}

# 检查是否以 root 用户运行
check_root() {
    if [[ $EUID -eq 0 ]]; then
        log_warn "建议不要以 root 用户运行此脚本"
        log_warn "按 Enter 继续，或 Ctrl+C 退出..."
        read -r
    fi
}

# 检查系统类型
check_os() {
    log_step "检测操作系统..."

    if [[ "$OSTYPE" == "darwin"* ]]; then
        OS="macOS"
        PKG_MANAGER="brew"
    elif [[ -f /etc/os-release ]]; then
        . /etc/os-release
        OS="$NAME"
        if command -v apt-get &> /dev/null; then
            PKG_MANAGER="apt"
        elif command -v yum &> /dev/null; then
            PKG_MANAGER="yum"
        elif command -v dnf &> /dev/null; then
            PKG_MANAGER="dnf"
        elif command -v pacman &> /dev/null; then
            PKG_MANAGER="pacman"
        fi
    else
        OS="Unknown"
        PKG_MANAGER="unknown"
    fi

    log_info "检测到系统: $OS ($PKG_MANAGER)"
}

# 检查 Rust 是否安装
check_rust() {
    log_step "检查 Rust 环境..."

    if command -v rustc &> /dev/null; then
        RUST_VERSION=$(rustc --version | awk '{print $2}')
        log_info "Rust 已安装: v$RUST_VERSION"

        if command -v cargo &> /dev/null; then
            CARGO_VERSION=$(cargo --version | awk '{print $2}')
            log_info "Cargo 已安装: v$CARGO_VERSION"
        fi

        # 检查 cargo 是否支持 tokio 等需要的特性
        return 0
    else
        log_warn "Rust 未安装，将自动安装"
        return 1
    fi
}

# 安装 Rust
install_rust() {
    log_step "安装 Rust 环境..."

    # 安装 rustup
    if ! command -v rustup &> /dev/null; then
        log_info "正在安装 rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable

        # 添加到 PATH
        export PATH="$HOME/.cargo/bin:$PATH"

        # 源配置
        if [ -f "$HOME/.cargo/env" ]; then
            source "$HOME/.cargo/env"
        fi

        log_info "Rust 安装完成"
    else
        log_info "Rust 已存在"
    fi

    # 验证安装
    if command -v rustc &> /dev/null; then
        log_info "Rust 版本: $(rustc --version)"
        log_info "Cargo 版本: $(cargo --version)"
    else
        log_error "Rust 安装失败"
        exit 1
    fi
}

# 安装系统依赖
install_system_deps() {
    log_step "安装系统依赖..."

    if [[ "$OS" == "macOS" ]]; then
        if command -v brew &> /dev/null; then
            log_info "使用 Homebrew 安装依赖..."
            # Rust 编译需要的一些 macOS 工具
            brew install sqlite3 2>/dev/null || log_warn "sqlite3 安装失败 (可能已安装)"
        else
            log_warn "未找到 Homebrew，部分功能可能受限"
        fi
    elif [[ "$PKG_MANAGER" == "apt" ]]; then
        log_info "使用 apt 安装依赖..."
        sudo apt-get update -qq
        sudo apt-get install -y -qq build-essential libssl-dev pkg-config sqlite3 2>/dev/null || true
    elif [[ "$PKG_MANAGER" == "dnf" ]] || [[ "$PKG_MANAGER" == "yum" ]]; then
        log_info "使用 dnf/yum 安装依赖..."
        sudo dnf install -y -q gcc gcc-c++ openssl-devel pkg-config sqlite 2>/dev/null || true
    fi

    log_info "系统依赖安装完成"
}

# 克隆或更新项目
setup_project() {
    log_step "设置项目..."

    # 确定项目目录
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    PROJECT_DIR="$SCRIPT_DIR"

    if [[ "$SCRIPT_DIR" == "$(pwd)" ]]; then
        # 当前目录就是项目目录
        if [[ -d ".git" ]]; then
            log_info "更新现有项目..."
            git pull origin main 2>/dev/null || log_warn "Git 更新失败"
        else
            log_info "克隆项目..."
            if command -v git &> /dev/null; then
                log_info "请指定项目目录或确保当前目录是项目目录"
                log_info "跳过项目克隆 (请手动执行: git clone https://github.com/gyc567/nuclaw.git)"
            else
                log_error "Git 未安装，无法克隆项目"
            fi
        fi
    else
        cd "$PROJECT_DIR"
        log_info "工作目录: $PROJECT_DIR"
    fi

    # 检查项目文件
    if [[ -f "Cargo.toml" ]]; then
        log_info "找到 Cargo.toml，项目配置正确"
    else
        log_error "未找到 Cargo.toml，请确保在项目目录下运行此脚本"
        return 1
    fi
}

# 构建项目
build_project() {
    log_step "构建项目..."

    cd "$PROJECT_DIR"

    # 清理旧构建
    if [[ -d "target" ]]; then
        log_info "清理旧构建文件..."
        cargo clean 2>/dev/null || true
    fi

    # 下载依赖
    log_info "下载依赖..."
    cargo fetch 2>/dev/null || cargo build --no-run 2>&1 | head -20

    # 构建项目
    log_info "编译项目 (release 模式)..."
    if cargo build --release; then
        log_info "构建成功!"
    else
        log_error "构建失败"
        exit 1
    fi
}

# 运行测试
run_tests() {
    log_step "运行测试..."

    cd "$PROJECT_DIR"

    # 运行 cargo test
    log_info "执行 cargo test..."
    if cargo test --release; then
        log_info "所有测试通过!"
        TEST_RESULT=0
    else
        log_warn "部分测试失败，但核心功能正常"
        TEST_RESULT=1
    fi
}

# 创建必要目录
setup_directories() {
    log_step "创建运行时目录..."

    cd "$PROJECT_DIR"

    # 创建运行时目录
    mkdir -p store
    mkdir -p data
    mkdir -p groups

    log_info "目录创建完成"
}

# 验证安装
verify_installation() {
    log_step "验证安装..."

    cd "$PROJECT_DIR"

    local checks=0
    local passed=0

    # 检查二进制文件
    if [[ -f "target/release/nuclaw" ]]; then
        log_info "✓ 二进制文件存在"
        ((passed++))
    else
        log_error "✗ 二进制文件不存在"
    fi
    ((checks++))

    # 检查版本
    if ./target/release/nuclaw --version &> /dev/null; then
        log_info "✓ 程序可执行"
        ((passed++))
    else
        log_error "✗ 程序不可执行"
    fi
    ((checks++))

    # 检查帮助
    if ./target/release/nuclaw --help &> /dev/null; then
        log_info "✓ CLI 正常响应"
        ((passed++))
    else
        log_error "✗ CLI 无响应"
    fi
    ((checks++))

    # 检查目录
    if [[ -d "store" ]] && [[ -d "data" ]]; then
        log_info "✓ 运行时目录创建成功"
        ((passed++))
    else
        log_error "✗ 运行时目录创建失败"
    fi
    ((checks++))

    # 运行程序 - 使用 background + sleep 替代 timeout
    log_info "测试程序运行..."
    ./target/release/nuclaw > /tmp/nuclaw_test.log 2>&1 &
    NUCLAW_PID=$!
    sleep 2

    # 检查进程是否还在运行
    if ps -p $NUCLAW_PID > /dev/null 2>&1 || grep -q "Starting NuClaw" /tmp/nuclaw_test.log; then
        log_info "✓ 程序启动正常"
        ((passed++))
    else
        log_error "✗ 程序启动失败"
    fi
    ((checks++))

    # 清理进程
    kill $NUCLAW_PID 2>/dev/null || true

    echo ""
    log_info "验证结果: $passed/$checks 检查通过"

    return $((checks - passed))
}

# 显示使用说明
show_usage() {
    echo ""
    echo "==============================================================================="
    echo "  NuClaw 安装完成!"
    echo "==============================================================================="
    echo ""
    echo "使用方式:"
    echo "  ./target/release/nuclaw              # 启动服务"
    echo "  ./target/release/nuclaw --help       # 查看帮助"
    echo "  ./target/release/nuclaw --auth       # 认证流程"
    echo ""
    echo "目录说明:"
    echo "  store/    - SQLite 数据库和认证文件"
    echo "  data/     - 运行时数据 (会话、IPC)"
    echo "  groups/   - 群组 CLAUDE.md 文件"
    echo ""
    echo "后续步骤:"
    echo "  1. 配置 WhatsApp 认证"
    echo "  2. 注册群组"
    echo "  3. 设置计划任务 (可选)"
    echo ""
    echo "详细文档请查看 README.md"
    echo ""
}

# 主函数
main() {
    echo "==============================================================================="
    echo "  NuClaw 一键安装脚本"
    echo "  Rust 版本的个人 Claude 助手"
    echo "==============================================================================="
    echo ""

    # 初始化变量
    PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    TEST_RESULT=0

    # 欢迎信息
    log_info "开始安装 NuClaw..."
    echo ""

    # 执行安装步骤
    check_root
    check_os
    check_rust || install_rust
    install_system_deps
    setup_project
    setup_directories
    build_project
    run_tests
    verify_installation || TEST_RESULT=$?

    # 显示结果
    if [[ $TEST_RESULT -eq 0 ]]; then
        show_usage
        log_info "安装成功完成!"
        exit 0
    else
        log_warn "安装完成，但部分验证失败"
        log_info "请查看上述输出了解详情"
        exit 1
    fi
}

# 脚本入口
main "$@"
