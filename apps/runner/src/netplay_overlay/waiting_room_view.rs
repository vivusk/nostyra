//! The waiting room: a dimmed full window with a centred column — title, the room
//! code (host only, large) with "Share this code", the roster (nick · role ·
//! ping), then the phase line ("Press Start to begin" / "Waiting for host..." /
//! sync progress bar / why it ended) and the host's lobby notice.

use netplay_session::lobby_state::LobbyPhase;
use netplay_session::room_api_types::SeatRole;
use video_renderer::{text_size, OverlayCanvas};

use super::{ascii, DIM_RGBA, MUTED_RGBA, TEXT_RGBA, WARN_RGBA};
use crate::netplay_runtime::NetplayRuntime;
use crate::on_screen_status::text_scale;

/// Progress bar width in font pixels.
const BAR_WIDTH: u32 = 120;

struct Column<'a> {
    canvas: &'a mut OverlayCanvas,
    width: u32,
    y: u32,
}

impl Column<'_> {
    fn line(&mut self, text: &str, scale: u32, rgba: [u8; 4]) {
        let (w, h) = text_size(text, scale);
        let x = self.width.saturating_sub(w) / 2;
        self.canvas.draw_text(x, self.y, text, scale, rgba);
        self.y += h + 4 * scale;
    }

    fn bar(&mut self, progress: f32, scale: u32) {
        let (w, h) = (BAR_WIDTH * scale, 4 * scale);
        let x = self.width.saturating_sub(w) / 2;
        self.canvas
            .fill_rect(x, self.y, w, h, [0x40, 0x40, 0x48, 0xFF]);
        let filled = (w as f32 * progress.clamp(0.0, 1.0)) as u32;
        self.canvas.fill_rect(x, self.y, filled, h, TEXT_RGBA);
        self.y += h + 4 * scale;
    }
}

fn role_label(role: SeatRole) -> &'static str {
    match role {
        SeatRole::Host => "host",
        SeatRole::Player => "player",
        SeatRole::Spectator => "watching",
    }
}

fn phase_line(runtime: &NetplayRuntime) -> (String, [u8; 4]) {
    let text = match runtime.phase() {
        LobbyPhase::Connecting => "Connecting...".to_owned(),
        LobbyPhase::Handshaking => "Joining the room...".to_owned(),
        LobbyPhase::InLobby if runtime.is_host() && runtime.can_start() => {
            "Press Start to begin".to_owned()
        }
        LobbyPhase::InLobby if runtime.is_host() => "Waiting for players...".to_owned(),
        LobbyPhase::InLobby => "Waiting for host...".to_owned(),
        LobbyPhase::Syncing => "Syncing...".to_owned(),
        LobbyPhase::Starting => "Starting...".to_owned(),
        LobbyPhase::Resyncing => "Resyncing...".to_owned(),
        LobbyPhase::Running => String::new(),
        LobbyPhase::Ended(_) => {
            let text = runtime
                .end_text()
                .unwrap_or_else(|| "Session ended".to_owned());
            return (ascii(&text), WARN_RGBA);
        }
    };
    (text, TEXT_RGBA)
}

pub(super) fn paint(
    canvas: &mut OverlayCanvas,
    (width, height): (u32, u32),
    runtime: &NetplayRuntime,
) {
    canvas.fill_rect(0, 0, width, height, DIM_RGBA);
    let scale = text_scale(height);
    let mut column = Column {
        canvas,
        width,
        y: height / 6,
    };
    column.line("NETPLAY", scale * 2, TEXT_RGBA);
    if let Some(code) = runtime.room_code() {
        column.line(&ascii(code), scale * 4, TEXT_RGBA);
        column.line("Share this code", scale, MUTED_RGBA);
    }
    for member in runtime.roster_lines() {
        let ping = member
            .ping_ms
            .map(|p| format!("  {p} ms"))
            .unwrap_or_default();
        let text = format!("{}  {}{ping}", ascii(&member.nick), role_label(member.role));
        column.line(&text, scale, TEXT_RGBA);
    }
    let (text, color) = phase_line(runtime);
    column.line(&text, scale, color);
    if *runtime.phase() == LobbyPhase::Syncing && !runtime.is_host() {
        column.bar(runtime.sync_progress(), scale);
    }
    if let Some(notice) = runtime.notice() {
        column.line(&ascii(notice), scale, MUTED_RGBA);
    }
}
