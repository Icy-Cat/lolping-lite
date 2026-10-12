//! Minimal en/zh UI strings.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    En,
    Zh,
}

pub struct Strings {
    pub app_title: &'static str,
    pub debug_pin: &'static str,
    pub settings: &'static str,
    pub trigger: &'static str,
    pub emote_trigger: &'static str,
    pub ping_size: &'static str,
    pub emote_size: &'static str,
    pub volume: &'static str,
    pub muted: &'static str,
    pub language: &'static str,
    pub launch_at_login: &'static str,
    pub room: &'static str,
    pub create_room: &'static str,
    pub join_room: &'static str,
    pub leave_room: &'static str,
    pub room_code: &'static str,
    pub paused: &'static str,
    pub game_check: &'static str,
    pub game_check_desc: &'static str,
}

pub fn strings(lang: Lang) -> Strings {
    match lang {
        Lang::En => Strings {
            app_title: "lolPing-lite",
            debug_pin: "Pin debug UI",
            settings: "Settings",
            trigger: "Ping trigger",
            emote_trigger: "Emote trigger",
            ping_size: "Ping size",
            emote_size: "Emote size",
            volume: "Volume",
            muted: "Muted",
            language: "Language",
            launch_at_login: "Open at login",
            room: "Room",
            create_room: "Create room",
            join_room: "Join",
            leave_room: "Leave",
            room_code: "Room code",
            paused: "Paused",
            game_check: "Pause in fullscreen apps",
            game_check_desc: "Games and other fullscreen apps keep every click: trigger + click passes through there, and gestures resume on the desktop",
        },
        Lang::Zh => Strings {
            app_title: "lolPing-lite",
            debug_pin: "钉住调试界面",
            settings: "设置",
            trigger: "Ping 触发键",
            emote_trigger: "表情触发键",
            ping_size: "Ping 大小",
            emote_size: "表情大小",
            volume: "音量",
            muted: "静音",
            language: "语言",
            launch_at_login: "登录时打开",
            room: "房间",
            create_room: "创建房间",
            join_room: "加入",
            leave_room: "离开",
            room_code: "房间码",
            paused: "已暂停",
            game_check: "全屏程序中暂停",
            game_check_desc: "游戏等全屏程序独占点击：触发键 + 单击原样交给程序，回到桌面后手势恢复正常",
        },
    }
}
