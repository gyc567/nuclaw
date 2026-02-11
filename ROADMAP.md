# NuClaw Roadmap

This document outlines the planned features and improvements for NuClaw.

## Current Version: 1.0.0

## Short Term (Next 3 Months)

### Messaging Enhancements
- [ ] **Slack Integration** - Add Slack bot support alongside WhatsApp and Telegram
- [ ] **Discord Integration** - Support Discord bot API
- [ ] **Message Threading** - Support threaded conversations in Telegram
- [ ] **Rich Media Support** - Handle images, documents, and voice messages

### Security & Privacy
- [ ] **End-to-End Encryption** - Encrypt sensitive data at rest
- [ ] **Rate Limiting** - Implement per-user and per-group rate limits
- [ ] **Audit Logging** - Comprehensive audit trail for all actions
- [ ] **API Key Management** - Secure storage for API keys

### Developer Experience
- [ ] **Plugin System** - Allow custom plugins for message processing
- [ ] **Hot Reload** - Reload configuration without restart
- [ ] **Better Error Messages** - Improve error reporting and debugging
- [ ] **Metrics Endpoint** - Prometheus-compatible metrics export

## Medium Term (3-6 Months)

### AI Features
- [ ] **Multi-Model Support** - Support for GPT, Gemini, and other LLMs
- [ ] **Context Window Management** - Intelligent context summarization
- [ ] **Function Calling** - Support for tool/function calling in agents
- [ ] **RAG Integration** - Built-in retrieval-augmented generation support

### Infrastructure
- [ ] **Horizontal Scaling** - Support for multiple bot instances
- [ ] **Redis Backend** - Optional Redis for state management
- [ ] **Kubernetes Support** - Helm charts and K8s manifests
- [ ] **Cloud Deployment Guides** - AWS, GCP, Azure deployment tutorials

### User Experience
- [ ] **Web Dashboard** - Web UI for configuration and monitoring
- [ ] **Conversation History** - Web interface to view chat history
- [ ] **User Management** - Manage users and permissions via web UI
- [ ] **Analytics** - Usage statistics and insights

## Long Term (6-12 Months)

### Advanced Features
- [ ] **Voice Interface** - Voice-to-text and text-to-voice support
- [ ] **Multi-Language Support** - i18n for all user-facing text
- [ ] **Custom Workflows** - Visual workflow builder
- [ ] **Agent Marketplace** - Share and discover agent configurations

### Enterprise Features
- [ ] **SSO Integration** - SAML and OIDC support
- [ ] **RBAC** - Role-based access control
- [ ] **Compliance** - SOC2, GDPR compliance features
- [ ] **Enterprise Support** - SLA and priority support

### Platform Expansion
- [ ] **iOS App** - Native iOS companion app
- [ ] **Android App** - Native Android companion app
- [ ] **Desktop App** - Electron-based desktop application
- [ ] **Browser Extension** - Chrome/Firefox extension

## Completed Features ✅

### Version 1.0.0
- [x] WhatsApp integration via MCP
- [x] Telegram Bot API integration
- [x] Container-based agent execution
- [x] Task scheduler with cron support
- [x] SQLite database for persistence
- [x] Group isolation and context management
- [x] Webhook and polling support
- [x] Mount allowlist security
- [x] Comprehensive test suite (21 tests)
- [x] One-click deployment script

## How to Contribute

We welcome contributions to any item on this roadmap! Please:

1. Check existing [issues](https://github.com/gyc567/nuclaw/issues) to avoid duplication
2. Open a new issue to discuss major features before starting
3. Comment on an issue to indicate you're working on it
4. Follow our [Contributing Guidelines](CONTRIBUTING.md)

## Requesting Features

To request a new feature:

1. Search existing issues first
2. If not found, open a new issue with:
   - Clear description of the feature
   - Use case and motivation
   - Possible implementation approach (optional)

## Priority Labels

- **P0** - Critical, blocks releases
- **P1** - High priority, planned for next release
- **P2** - Medium priority, on roadmap
- **P3** - Nice to have, community contributions welcome

---

*This roadmap is a living document and will be updated as priorities change. Last updated: February 2026*
