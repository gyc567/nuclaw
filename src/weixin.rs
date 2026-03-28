//! WeChat integration re-export
//!
//! Re-exports the WeChat module under the legacy `weixin` name for backwards compatibility.

pub use crate::wechat::WeChatClient as WeixinClient;
pub use crate::wechat::{
    extract_trigger_pure, ilink_errcode_to_error, is_allowed_sender_pure,
    is_duplicate_message_pure, load_registered_chats as load_weixin_registered_chats,
    load_router_state as load_weixin_router_state, parse_message_type_pure,
    should_auto_start_wechat as should_auto_start_weixin, truncate, GetQrcodeResponse,
    GetUpdatesResponse, IlinkMessage, LoginByQrcodeResponse, MediaDownloadResponse,
    SendMessageRequest, SendMessageResponse,
};
