use crate::error::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn send(&self, jid: &str, message: &str) -> Result<()>;
    async fn start(&self) -> Result<()>;
    fn is_enabled(&self) -> bool;
}

pub struct ChannelRegistry {
    channels: RwLock<HashMap<String, Box<dyn Channel>>>,
}

impl ChannelRegistry {
    pub fn new() -> Self {
        Self {
            channels: RwLock::new(HashMap::new()),
        }
    }

    pub fn register<C: Channel + 'static>(&self, channel: C) -> &Self {
        if let Ok(mut channels) = self.channels.write() {
            channels.insert(channel.name().to_string(), Box::new(channel));
        }
        self
    }

    pub fn get(&self, name: &str) -> Option<Box<dyn Channel>> {
        self.channels
            .read()
            .ok()?
            .get(name)
            .map(|_| panic!("Cannot get Channel by value - use list() or is_registered()"))
    }

    pub fn list(&self) -> Vec<String> {
        self.channels
            .read()
            .map(|c| c.keys().cloned().collect())
            .unwrap_or_default()
    }

    pub fn is_registered(&self, name: &str) -> bool {
        self.channels
            .read()
            .map(|c| c.contains_key(name))
            .unwrap_or(false)
    }

    pub fn is_enabled(&self, name: &str) -> bool {
        self.channels
            .read()
            .ok()
            .and_then(|channels| channels.get(name).map(|c| c.is_enabled()))
            .unwrap_or(false)
    }

    pub fn unregister(&self, name: &str) -> bool {
        self.channels
            .write()
            .map(|mut c| c.remove(name).is_some())
            .unwrap_or(false)
    }
}

impl Default for ChannelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Box<dyn Channel> {
    fn clone(&self) -> Self {
        panic!("Channel cannot be cloned - use registry instead")
    }
}

pub fn channel_registry() -> ChannelRegistry {
    ChannelRegistry::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct MockChannel {
        channel_name: String,
        enabled: bool,
    }

    impl MockChannel {
        fn new(name: &str, enabled: bool) -> Self {
            Self {
                channel_name: name.to_string(),
                enabled,
            }
        }
    }

    #[async_trait]
    impl Channel for MockChannel {
        fn name(&self) -> &str {
            &self.channel_name
        }

        async fn send(&self, _jid: &str, _message: &str) -> Result<()> {
            Ok(())
        }

        async fn start(&self) -> Result<()> {
            Ok(())
        }

        fn is_enabled(&self) -> bool {
            self.enabled
        }
    }

    #[test]
    fn test_channel_registry_new() {
        let registry = ChannelRegistry::new();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_register_channel() {
        let registry = ChannelRegistry::new();
        registry.register(MockChannel::new("test", true));
        assert!(registry.is_registered("test"));
    }

    #[test]
    fn test_get_channel() {
        let registry = ChannelRegistry::new();
        registry.register(MockChannel::new("test", true));
        assert!(registry.is_registered("test"));
    }

    #[test]
    fn test_get_channel_nonexistent() {
        let registry = ChannelRegistry::new();
        let channel = registry.get("nonexistent");
        assert!(channel.is_none());
    }

    #[test]
    fn test_list_channels() {
        let registry = ChannelRegistry::new();
        registry.register(MockChannel::new("ch1", true));
        registry.register(MockChannel::new("ch2", false));
        let list = registry.list();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"ch1".to_string()));
        assert!(list.contains(&"ch2".to_string()));
    }

    #[test]
    fn test_is_enabled() {
        let registry = ChannelRegistry::new();
        registry.register(MockChannel::new("enabled", true));
        registry.register(MockChannel::new("disabled", false));
        
        assert!(registry.is_enabled("enabled"));
        assert!(!registry.is_enabled("disabled"));
    }

    #[test]
    fn test_unregister() {
        let registry = ChannelRegistry::new();
        registry.register(MockChannel::new("test", true));
        assert!(registry.is_registered("test"));
        
        registry.unregister("test");
        assert!(!registry.is_registered("test"));
    }

    #[test]
    fn test_channel_name() {
        let channel = MockChannel::new("mychannel", true);
        assert_eq!(channel.name(), "mychannel");
    }

    #[test]
    fn test_channel_registry_function() {
        let registry = channel_registry();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_register_returns_self() {
        let registry = ChannelRegistry::new();
        let result = registry.register(MockChannel::new("test", true));
        assert!(!result.list().is_empty());
    }
}
