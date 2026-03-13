//! Telegram policy enums

use serde::{Deserialize, Serialize};

/// DM policy enumeration
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DMPolicy {
    #[serde(rename = "pairing")]
    Pairing,
    #[serde(rename = "allowlist")]
    Allowlist,
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "disabled")]
    Disabled,
}

impl DMPolicy {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "pairing" => DMPolicy::Pairing,
            "allowlist" => DMPolicy::Allowlist,
            "open" => DMPolicy::Open,
            "disabled" => DMPolicy::Disabled,
            _ => DMPolicy::Pairing,
        }
    }
}

/// Group policy enumeration
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum GroupPolicy {
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "allowlist")]
    Allowlist,
    #[serde(rename = "disabled")]
    Disabled,
}

impl GroupPolicy {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "open" => GroupPolicy::Open,
            "allowlist" => GroupPolicy::Allowlist,
            "disabled" => GroupPolicy::Disabled,
            _ => GroupPolicy::Allowlist,
        }
    }
}

/// Stream mode for message preview
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum StreamMode {
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "block")]
    Block,
}

impl StreamMode {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "off" => StreamMode::Off,
            "partial" => StreamMode::Partial,
            "block" => StreamMode::Block,
            _ => StreamMode::Partial,
        }
    }
}

/// Chunk mode for text splitting
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ChunkMode {
    #[serde(rename = "length")]
    Length,
    #[serde(rename = "newline")]
    Newline,
}

impl ChunkMode {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "length" => ChunkMode::Length,
            "newline" => ChunkMode::Newline,
            _ => ChunkMode::Length,
        }
    }
}

/// Reply mode for threading
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ReplyMode {
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "first")]
    First,
    #[serde(rename = "all")]
    All,
}

impl ReplyMode {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "off" => ReplyMode::Off,
            "first" => ReplyMode::First,
            "all" => ReplyMode::All,
            _ => ReplyMode::Off,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dm_policy_from_str() {
        assert_eq!(DMPolicy::parse("pairing"), DMPolicy::Pairing);
        assert_eq!(DMPolicy::parse("allowlist"), DMPolicy::Allowlist);
        assert_eq!(DMPolicy::parse("open"), DMPolicy::Open);
        assert_eq!(DMPolicy::parse("disabled"), DMPolicy::Disabled);
        assert_eq!(DMPolicy::parse("unknown"), DMPolicy::Pairing);
    }

    #[test]
    fn test_dm_policy_case_insensitive() {
        assert_eq!(DMPolicy::parse("PAIRING"), DMPolicy::Pairing);
        assert_eq!(DMPolicy::parse("Open"), DMPolicy::Open);
    }

    #[test]
    fn test_group_policy_from_str() {
        assert_eq!(GroupPolicy::parse("open"), GroupPolicy::Open);
        assert_eq!(GroupPolicy::parse("allowlist"), GroupPolicy::Allowlist);
        assert_eq!(GroupPolicy::parse("disabled"), GroupPolicy::Disabled);
        assert_eq!(GroupPolicy::parse("unknown"), GroupPolicy::Allowlist);
    }

    #[test]
    fn test_group_policy_case_insensitive() {
        assert_eq!(GroupPolicy::parse("OPEN"), GroupPolicy::Open);
        assert_eq!(GroupPolicy::parse("Allowlist"), GroupPolicy::Allowlist);
    }

    #[test]
    fn test_stream_mode_parse() {
        assert_eq!(StreamMode::parse("off"), StreamMode::Off);
        assert_eq!(StreamMode::parse("partial"), StreamMode::Partial);
        assert_eq!(StreamMode::parse("block"), StreamMode::Block);
        assert_eq!(StreamMode::parse("unknown"), StreamMode::Partial);
    }

    #[test]
    fn test_chunk_mode_parse() {
        assert_eq!(ChunkMode::parse("length"), ChunkMode::Length);
        assert_eq!(ChunkMode::parse("newline"), ChunkMode::Newline);
        assert_eq!(ChunkMode::parse("unknown"), ChunkMode::Length);
    }

    #[test]
    fn test_reply_mode_parse() {
        assert_eq!(ReplyMode::parse("off"), ReplyMode::Off);
        assert_eq!(ReplyMode::parse("first"), ReplyMode::First);
        assert_eq!(ReplyMode::parse("all"), ReplyMode::All);
        assert_eq!(ReplyMode::parse("unknown"), ReplyMode::Off);
    }

    #[test]
    fn test_policy_serialization() {
        let dm = DMPolicy::Allowlist;
        let json = serde_json::to_string(&dm).unwrap();
        assert!(json.contains("allowlist"));

        let group = GroupPolicy::Open;
        let json = serde_json::to_string(&group).unwrap();
        assert!(json.contains("open"));
    }
}
