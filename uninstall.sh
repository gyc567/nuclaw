#!/bin/bash

set -euo pipefail

FORCE=false
PURGE=false
DRY_RUN=false

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
NC='\033[0m'

for arg in "$@"; do
    case $arg in
        --force) FORCE=true ;;
        --purge) PURGE=true ;;
        --dry-run|--dryrun) DRY_RUN=true ;;
        -h|--help) 
            echo "用法: $0 [选项]"
            echo ""
            echo "选项:"
            echo "  --force      跳过确认提示"
            echo "  --purge      删除所有数据 (包括 ~/.nuclaw/)"
            echo "  --dry-run    预览模式，不执行实际删除"
            echo "  -h, --help   显示此帮助"
            exit 0
            ;;
    esac
done

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${BLUE}[STEP]${NC} $1"; }
log_success() { echo -e "${MAGENTA}[OK]${NC} $1"; }
log_dry() { echo -e "${CYAN}[DRY]${NC} $1"; }

confirm() {
    if [[ "$FORCE" == true ]]; then return 0; fi
    local prompt="$1 (y/N): "
    read -p "$(echo -e "${YELLOW}[PROMPT]${NC} $prompt")" choice
    case "$choice" in
        y|Y|yes|Yes ) return 0 ;;
        * ) return 1 ;;
    esac
}

NUCLAW_HOME="${NUCLAW_HOME:-${HOME}/.nuclaw}"

detect_binaries() {
    local bins=()
    
    [[ -f /usr/local/bin/nuclaw ]] && bins+=("/usr/local/bin/nuclaw")
    [[ -f /usr/bin/nuclaw ]] && bins+=("/usr/bin/nuclaw")
    [[ -f "$HOME/.local/bin/nuclaw" ]] && bins+=("$HOME/.local/bin/nuclaw")
    [[ -f "$NUCLAW_HOME/nuclaw" ]] && bins+=("$NUCLAW_HOME/nuclaw")
    
    echo "${bins[@]}"
}

detect_path_entries() {
    local entries=()
    local shell_rc_files=("$HOME/.zshrc" "$HOME/.bashrc" "$HOME/.profile" "$HOME/.bash_profile")
    
    for rc in "${shell_rc_files[@]}"; do
        [[ -f "$rc" ]] || continue
        
        while IFS= read -r line; do
            [[ -z "$line" ]] && continue
            [[ "$line" =~ ^[[:space:]]*# ]] && continue
            
            if [[ "$line" == *"nuclaw"* ]] && [[ "$line" == *"export PATH"* ]]; then
                entries+=("$rc:$line")
            fi
        done < "$rc"
    done
    
    printf '%s\n' "${entries[@]}"
}

stop_services() {
    local pids
    pids=$(pgrep -f "nuclaw" 2>/dev/null || true)
    
    if [[ -n "$pids" ]]; then
        log_warn "发现运行中的 NuClaw 进程: $pids"
        
        if confirm "是否停止这些进程?"; then
            for pid in $pids; do
                if [[ "$DRY_RUN" == true ]]; then
                    log_dry "将终止进程: $pid"
                else
                    kill "$pid" 2>/dev/null || true
                    log_step "已终止进程: $pid"
                fi
            done
        fi
    else
        log_info "无运行中的 NuClaw 进程"
    fi
}

remove_binaries() {
    local binaries=("$@")
    local removed=0
    
    for bin in "${binaries[@]}"; do
        [[ -z "$bin" ]] && continue
        
        if [[ -f "$bin" ]]; then
            if [[ "$DRY_RUN" == true ]]; then
                log_dry "将删除: $bin"
            else
                if [[ "$bin" == /usr/local/bin/* ]] || [[ "$bin" == /usr/bin/* ]]; then
                    if sudo rm -f "$bin" 2>/dev/null; then
                        log_success "已删除: $bin"
                    else
                        log_error "删除失败 (权限不足): $bin"
                    fi
                else
                    rm -f "$bin"
                    log_success "已删除: $bin"
                fi
            fi
            ((removed++))
        fi
    done
    
    [[ $removed -eq 0 ]] && log_info "未发现需要删除的二进制文件"
}

cleanup_path_entries() {
    local entries=("$@")
    local cleaned=0
    
    for entry in "${entries[@]}"; do
        [[ -z "$entry" ]] && continue
        
        local rc="${entry%%:*}"
        local line="${entry#*:}"
        
        [[ -z "$rc" ]] || [[ -z "$line" ]] && continue
        
        if [[ "$DRY_RUN" == true ]]; then
            log_dry "将从 $rc 移除: $line"
        else
            local tmp
            tmp=$(mktemp)
            
            grep -vF -- "$line" "$rc" > "$tmp" || true
            mv "$tmp" "$rc"
            
            log_success "已从 $rc 移除 PATH 条目"
        fi
        ((cleaned++))
    done
    
    [[ $cleaned -eq 0 ]] && log_info "无 PATH 条目需要清理"
}

remove_nuclaw_home() {
    if [[ -d "$NUCLAW_HOME" ]]; then
        if [[ "$PURGE" == true ]]; then
            if [[ "$DRY_RUN" == true ]]; then
                log_dry "将删除配置目录: $NUCLAW_HOME"
            else
                rm -rf "$NUCLAW_HOME"
                log_success "已删除配置目录: $NUCLAW_HOME"
            fi
        else
            log_warn "配置目录存在: $NUCLAW_HOME"
            log_info "内容包含: $(ls -A "$NUCLAW_HOME" 2>/dev/null | tr '\n' ' ')"
            if confirm "是否删除配置目录 (包含数据库/配置)?"; then
                if [[ "$DRY_RUN" == true ]]; then
                    log_dry "将删除配置目录: $NUCLAW_HOME"
                else
                    rm -rf "$NUCLAW_HOME"
                    log_success "已删除配置目录: $NUCLAW_HOME"
                fi
            else
                log_info "保留配置目录: $NUCLAW_HOME"
            fi
        fi
    else
        log_info "配置目录不存在: $NUCLAW_HOME"
    fi
}

main() {
    echo -e "${CYAN}╔═══════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║                   NuClaw 卸载                         ║${NC}"
    echo -e "${CYAN}╚═══════════════════════════════════════════════════════════╝${NC}"
    echo ""
    
    if [[ "$DRY_RUN" == true ]]; then
        log_warn "预览模式 - 不会执行实际删除"
        echo ""
    fi
    
    local binaries
    binaries=$(detect_binaries)
    local binary_array=($binaries)
    
    if [[ ${#binary_array[@]} -eq 0 ]] && [[ ! -d "$NUCLAW_HOME" ]]; then
        log_error "未检测到 NuClaw 安装"
        exit 1
    fi
    
    echo "==============================================================================="
    log_info "检测到的安装内容:"
    echo ""
    
    if [[ ${#binary_array[@]} -gt 0 ]]; then
        for bin in "${binary_array[@]}"; do
            echo "  二进制: $bin"
        done
        echo ""
    fi
    
    local path_entries
    mapfile -t path_entries < <(detect_path_entries)
    if [[ ${#path_entries[@]} -gt 0 ]]; then
        for entry in "${path_entries[@]}"; do
            local rc="${entry%%:*}"
            echo "  PATH: $rc"
        done
        echo ""
    fi
    
    [[ -d "$NUCLAW_HOME" ]] && echo "  配置: $NUCLAW_HOME/"
    echo "==============================================================================="
    echo ""
    
    if ! confirm "确认卸载 NuClaw?"; then
        log_info "取消卸载"
        exit 0
    fi
    
    stop_services
    
    if [[ ${#binary_array[@]} -gt 0 ]]; then
        log_step "删除二进制文件..."
        remove_binaries "${binary_array[@]}"
    fi
    
    if [[ ${#path_entries[@]} -gt 0 ]]; then
        log_step "清理 PATH 环境变量..."
        cleanup_path_entries "${path_entries[@]}"
    fi
    
    remove_nuclaw_home
    
    echo ""
    echo "==============================================================================="
    log_success "卸载完成!"
    echo "==============================================================================="
    
    if [[ "$DRY_RUN" == true ]]; then
        echo ""
        log_info "以上是预览结果，如需执行请运行: $0"
    fi
}

main "$@"
