# macOS 开机自启动配置

## 自动安装

安装脚本会自动配置 launchd。

## 手动安装

```bash
# 1. 编辑 plist 文件，替换 __USER__ 为你的用户名
sed -i '' 's|__USER__|'"$USER"'|g' com.nuclaw.agent.plist

# 2. 复制到 LaunchAgents 目录
mkdir -p ~/Library/LaunchAgents
cp com.nuclaw.agent.plist ~/Library/LaunchAgents/

# 3. 加载配置
launchctl load ~/Library/LaunchAgents/com.nuclaw.agent.plist

# 4. 立即启动
launchctl start com.nuclaw.agent
```

## 管理命令

```bash
# 停止服务
launchctl stop com.nuclaw.agent

# 重新加载配置
launchctl unload ~/Library/LaunchAgents/com.nuclaw.agent.plist
launchctl load ~/Library/LaunchAgents/com.nuclaw.agent.plist

# 查看日志
tail -f ~/.nuclaw/logs/launchd.log
tail -f ~/.nuclaw/logs/launchd.err

# 取消开机自启动
launchctl unload ~/Library/LaunchAgents/com.nuclaw.agent.plist
rm ~/Library/LaunchAgents/com.nuclaw.agent.plist
```
