#!/bin/bash

#==============================================================================
# NuClaw Cross-Platform Installer
# 
# Supports: Linux, macOS, Windows (WSL)
#
# Usage:
#   curl -sSL https://raw.githubusercontent.com/gyc567/nuclaw/main/install.sh | bash
#   ./install.sh
#==============================================================================

set -euo pipefail

#------------------------------------------------------------------------------
# Configuration
#------------------------------------------------------------------------------
NUCLAW_REPO="gyc567/nuclaw"
GITHUB_API="https://api.github.com/repos/${NUCLAW_REPO}/releases/latest"
INSTALL_VERSION="${INSTALL_VERSION:-latest}"

# Colors (ANSI)
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
NC='\033[0m'

#------------------------------------------------------------------------------
# Utility Functions
#------------------------------------------------------------------------------
log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${BLUE}[STEP]${NC} $1"; }
log_success() { echo -e "${MAGENTA}[OK]${NC} $1"; }

# Detect if running in CI/automated environment
is_ci() {
    [[ -n "${CI:-}" || -n "${GITHUB_ACTIONS:-}" ]]
}

#------------------------------------------------------------------------------
# Platform Detection
#------------------------------------------------------------------------------
detect_os() {
    local os
    case "$(uname -s)" in
        Linux*)     echo "linux" ;;
        Darwin*)    echo "macos" ;;
        CYGWIN*|MINGW*|MSYS*) echo "windows" ;;
        *)          echo "unknown" ;;
    esac
}

detect_arch() {
    local arch
    arch=$(uname -m)
    case "$arch" in
        x86_64)     echo "x86_64" ;;
        aarch64|arm64) echo "arm64" ;;
        i386|i686)   echo "x86" ;;
        *)          echo "$arch" ;;
    esac
}

# Get NuClaw home directory
get_nuclaw_home() {
    if [[ -n "${NUCLAW_HOME:-}" ]]; then
        echo "$NUCLAW_HOME"
    elif [[ "${OS:-unknown}" == "windows" ]]; then
        echo "${USERPROFILE:-${HOME}}/.nuclaw"
    else
        echo "${HOME}/.nuclaw"
    fi
}

# Get temp directory
get_temp_dir() {
    if [[ "${OS:-unknown}" == "windows" ]]; then
        echo "${TEMP:-/tmp}"
    else
        echo "/tmp"
    fi
}

#------------------------------------------------------------------------------
# GitHub API
#------------------------------------------------------------------------------
get_latest_version() {
    local version
    version=$(curl -sSL "${GITHUB_API}" 2>/dev/null | grep -o '"tag_name": "[^"]*"' | head -1 | cut -d'"' -f4 || echo "")
    echo "${version#v}"
}

#------------------------------------------------------------------------------
# Download Functions
#------------------------------------------------------------------------------
get_download_url() {
    local version=$1
    local os=$2
    local arch=$3
    
    local filename=""
    case "$os" in
        linux)
            filename="nuclaw-${version}-${arch}-unknown-linux-gnu.tar.gz"
            ;;
        macos)
            filename="nuclaw-${version}-${arch}-apple-darwin.tar.gz"
            ;;
        windows)
            filename="nuclaw-${version}-${arch}-pc-windows-msvc.tar.gz"
            ;;
    esac
    
    echo "https://github.com/${NUCLAW_REPO}/releases/download/v${version}/${filename}"
}

download_binary() {
    local version=$1
    local os=$2
    local arch=$3
    local temp_dir
    temp_dir=$(get_temp_dir)
    
    local url
    url=$(get_download_url "$version" "$os" "$arch")
    
    local filename
    filename=$(basename "$url")
    local output="${temp_dir}/${filename}"
    
    log_step "Downloading NuClaw v${version} for ${os}-${arch}..."
    log_info "URL: ${url}"
    
    if curl -sSL --fail --location -o "$output" "$url" 2>/dev/null; then
        log_success "Downloaded: ${output}"
        echo "$output"
        return 0
    else
        rm -f "$output"
        return 1
    fi
}

#------------------------------------------------------------------------------
# Installation Functions
#------------------------------------------------------------------------------
install_from_tarball() {
    local tarball=$1
    local nuclaw_home=$2
    
    log_step "Installing to ${nuclaw_home}..."
    
    mkdir -p "$nuclaw_home"
    
    # Extract tarball
    tar -xzf "$tarball" -C "$nuclaw_home" --strip-components=1 2>/dev/null || {
        # Try without strip if structure differs
        tar -xzf "$tarball" -C "$nuclaw_home"
    }
    
    chmod +x "${nuclaw_home}/nuclaw" 2>/dev/null || true
    
    # Cleanup
    rm -f "$tarball"
    
    log_success "Installed: ${nuclaw_home}/nuclaw"
}

setup_directories() {
    local nuclaw_home=$1
    local dirs=("store" "data" "groups" "logs" "skills")
    
    for dir in "${dirs[@]}"; do
        mkdir -p "${nuclaw_home}/${dir}"
    done
    
    log_success "Directory structure created"
}

create_initial_config() {
    local nuclaw_home=$1
    local config_file="${nuclaw_home}/config.json"
    
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
        log_success "Config created: ${config_file}"
    fi
}

create_env_template() {
    local nuclaw_home=$1
    local env_file="${nuclaw_home}/.env.example"
    
    if [[ ! -f "${nuclaw_home}/.env" ]]; then
        cat > "$env_file" << 'EOF'
# NuClaw Configuration Template
# Copy this file to .env and fill in your values

# LLM Provider Configuration
# Choose one of: anthropic, openai, openrouter, custom
ANTHROPIC_API_KEY=your-api-key-here
ANTHROPIC_BASE_URL=https://api.anthropic.com

# Telegram Bot (optional)
TELEGRAM_BOT_TOKEN=your-bot-token-here
EOF
        log_success "Env template created: ${env_file}"
        log_info "Copy to .env and configure your API keys"
    fi
}

#------------------------------------------------------------------------------
# Service Setup (Linux systemd)
#------------------------------------------------------------------------------
setup_systemd() {
    local nuclaw_home=$1
    local user=$2
    
    log_step "Setting up systemd service..."
    
    local service_dir="/etc/systemd/system"
    local service_file="${service_dir}/nuclaw.service"
    
    if [[ -w "$service_dir" ]] || [[ "$EUID" -eq 0 ]]; then
        cat > "$service_file" << EOF
[Unit]
Description=NuClaw AI Assistant
After=network.target

[Service]
Type=simple
User=${user}
WorkingDirectory=${nuclaw_home}
Environment="NUCLAW_HOME=${nuclaw_home}"
ExecStart=${nuclaw_home}/nuclaw
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF
        
        systemctl daemon-reload 2>/dev/null || true
        log_success "systemd service created: ${service_file}"
    else
        log_warn "Need root to install systemd service"
        log_info "Manual setup required: sudo cp nuclaw.service /etc/systemd/system/"
    fi
}

#------------------------------------------------------------------------------
# Service Setup (macOS launchd)
#------------------------------------------------------------------------------
setup_launchd() {
    local nuclaw_home=$1
    
    log_step "Setting up macOS launchd..."
    
    local plist_dir="${HOME}/Library/LaunchAgents"
    local plist_file="${plist_dir}/com.nuclaw.agent.plist"
    
    mkdir -p "$plist_dir"
    
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
        <string>${nuclaw_home}</string>
    </dict>
    <key>ProgramArguments</key>
    <array>
        <string>${nuclaw_home}/nuclaw</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
EOF
    
    launchctl load "$plist_file" 2>/dev/null || true
    log_success "launchd service created: ${plist_file}"
}

#------------------------------------------------------------------------------
# Source Build (Fallback)
#------------------------------------------------------------------------------
build_from_source() {
    local nuclaw_home=$1
    local os=$2
    
    log_step "Building from source..."
    
    # Check Rust
    if ! command -v cargo &>/dev/null; then
        log_info "Installing Rust..."
        
        if [[ "$os" == "macos" ]]; then
            brew install rust
        elif [[ "$os" == "linux" ]]; then
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        fi
    fi
    
    # Clone or update
    local temp_dir
    temp_dir=$(get_temp_dir)
    local clone_dir="${temp_dir}/nuclaw-build"
    
    if [[ -d "$clone_dir" ]]; then
        rm -rf "$clone_dir"
    fi
    
    git clone --depth 1 "https://github.com/${NUCLAW_REPO}.git" "$clone_dir"
    
    cd "$clone_dir"
    cargo build --release
    
    mkdir -p "$nuclaw_home"
    cp "target/release/nuclaw" "${nuclaw_home}/"
    
    log_success "Built and installed from source"
}

#------------------------------------------------------------------------------
# Main Installation Flow
#------------------------------------------------------------------------------
main() {
    local os arch nuclaw_home version tarball
    
    # Banner
    echo ""
    echo -e "${CYAN}╔═══════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║     NuClaw Installer - Cross-Platform                   ║${NC}"
    echo -e "${CYAN}║     Rust-powered AI Assistant                          ║${NC}"
    echo -e "${CYAN}╚═══════════════════════════════════════════════════════════╝${NC}"
    echo ""
    
    # Detect platform
    os=$(detect_os)
    arch=$(detect_arch)
    nuclaw_home=$(get_nuclaw_home)
    
    if [[ "$os" == "unknown" ]]; then
        log_error "Unsupported operating system"
        exit 1
    fi
    
    log_info "Platform: ${os}-${arch}"
    log_info "Install directory: ${nuclaw_home}"
    echo ""
    
    # Get version
    if [[ "$INSTALL_VERSION" == "latest" ]]; then
        version=$(get_latest_version)
        if [[ -z "$version" ]]; then
            log_warn "Could not fetch latest version, using source build"
            build_from_source "$nuclaw_home" "$os"
            setup_directories "$nuclaw_home"
            create_initial_config "$nuclaw_home"
            create_env_template "$nuclaw_home"
            return 0
        fi
    else
        version="$INSTALL_VERSION"
    fi
    
    log_info "Target version: v${version}"
    echo ""
    
    # Try download
    if tarball=$(download_binary "$version" "$os" "$arch" 2>/dev/null); then
        install_from_tarball "$tarball" "$nuclaw_home"
    else
        log_warn "Pre-built binary not available for ${os}-${arch}"
        log_info "Falling back to source build..."
        build_from_source "$nuclaw_home" "$os"
    fi
    
    # Setup directories and config
    setup_directories "$nuclaw_home"
    create_initial_config "$nuclaw_home"
    create_env_template "$nuclaw_home"
    
    # Setup service
    if [[ "$os" == "linux" ]]; then
        setup_systemd "$nuclaw_home" "$(whoami)"
    elif [[ "$os" == "macos" ]]; then
        setup_launchd "$nuclaw_home"
    fi
    
    # Usage instructions
    echo ""
    echo -e "${GREEN}═══════════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}  Installation Complete!${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════════════════════${NC}"
    echo ""
    echo "Location: ${nuclaw_home}/nuclaw"
    echo ""
    echo "Quick start:"
    echo "  1. Copy env template: cp ${nuclaw_home}/.env.example ${nuclaw_home}/.env"
    echo "  2. Edit .env with your API keys"
    echo "  3. Run: ${nuclaw_home}/nuclaw --onboard"
    echo ""
    echo "Options:"
    echo "  ${nuclaw_home}/nuclaw --help        # Show help"
    echo "  ${nuclaw_home}/nuclaw --onboard    # Configure LLM/Telegram"
    echo "  ${nuclaw_home}/nuclaw --whatsapp   # Start WhatsApp bot"
    echo "  ${nuclaw_home}/nuclaw --telegram  # Start Telegram bot"
    echo ""
}

#------------------------------------------------------------------------------
# Entry Point
#------------------------------------------------------------------------------
main "$@"
