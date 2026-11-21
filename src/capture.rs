//! Capturing the game's window on Windows, where Blue Archive runs, the way Aoi
//! (bluearchive-aoi, its `watch::window`) does: Windows Graphics Capture of the window of
//! `BlueArchive.exe`, cut to the client area. The game is only looked at: nothing is sent
//! to it. A frame comes with where the client area lies on the desktop, for `overlay`.

use std::{
    sync::mpsc::{Receiver, SyncSender, sync_channel},
    thread,
    time::{Duration, Instant},
};

use image::RgbImage;
use windows_capture::{
    capture::{CaptureControl, Context, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
    window::Window,
};

const GAME: &str = "BlueArchive.exe";
/// How often the game's window is looked for.
const LOOK_INTERVAL: Duration = Duration::from_secs(1);
/// How often a frame is taken: the marks over the formation dialog follow its list as it
/// scrolls, and reading that dialog took 68 ms a frame (2026-11-17).
const INTERVAL: Duration = Duration::from_millis(100);

type CaptureError = Box<dyn std::error::Error + Send + Sync>;

/// What happens to the game's window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seen {
    /// The window is found and its frames come.
    Found,
    /// The window closed.
    Gone,
    /// The window is there and cannot be captured: why.
    Failed(String),
}

/// A frame of the game's screen (its window's client area).
pub struct Shot {
    pub image: RgbImage,
    /// The screen's top left corner on the desktop, in physical pixels; `None` when
    /// Windows did not tell where the client area lies.
    pub origin: Option<(i32, i32)>,
    /// Whether the game's window is the one in front.
    pub in_front: bool,
}

/// Left, top, width and height in a frame.
type Place = (u32, u32, u32, u32);

/// A window's client area in physical pixels, whatever DPI awareness the process runs
/// with: where it lies in the capture's frames (left, top, width, height), and its top
/// left corner on the desktop. A frame holds the window's visible frame (DWM's extended
/// frame bounds): the title bar above the client area and a border around it.
fn client_area(window: isize) -> Option<(Place, (i32, i32))> {
    use windows_sys::Win32::{
        Foundation::{POINT, RECT},
        Graphics::{
            Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute},
            Gdi::ClientToScreen,
        },
        UI::{
            HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext},
            WindowsAndMessaging::GetClientRect,
        },
    };
    let empty = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let (mut client, mut frame, mut origin) = (empty, empty, POINT { x: 0, y: 0 });
    // SAFETY: the rects and the point outlive the calls; the thread's awareness is restored.
    let ok = unsafe {
        let previous = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let ok = GetClientRect(window as _, &raw mut client) != 0
            && ClientToScreen(window as _, &raw mut origin) != 0
            && DwmGetWindowAttribute(
                window as _,
                DWMWA_EXTENDED_FRAME_BOUNDS as u32,
                (&raw mut frame).cast(),
                size_of::<RECT>() as u32,
            ) == 0;
        SetThreadDpiAwarenessContext(previous);
        ok
    };
    let offset = |a: i32, b: i32| u32::try_from(a - b).ok();
    if !ok {
        return None;
    }
    Some((
        (
            offset(origin.x, frame.left)?,
            offset(origin.y, frame.top)?,
            offset(client.right, client.left)?,
            offset(client.bottom, client.top)?,
        ),
        (origin.x, origin.y),
    ))
}

/// Whether a window is the one in front.
fn in_front(window: isize) -> bool {
    // SAFETY: the call takes nothing and returns a handle that is only compared.
    let front = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
    front as isize == window
}

/// The client area `(left, top, width, height)` of a window's frame, `frame_width` pixels
/// wide in RGBA, as RGB rows.
fn client_rgb(
    frame: &[u8],
    frame_width: u32,
    (left, top, width, height): (u32, u32, u32, u32),
) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
    for y in top..top + height {
        let row = (y * frame_width + left) as usize * 4;
        for pixel in frame[row..row + width as usize * 4].as_chunks::<4>().0 {
            rgb.extend_from_slice(&pixel[..3]);
        }
    }
    rgb
}

struct Capture {
    frames: SyncSender<Shot>,
    /// The game's window handle.
    window: isize,
    taken: Option<Instant>,
}

impl GraphicsCaptureApiHandler for Capture {
    type Flags = (SyncSender<Shot>, isize);
    type Error = CaptureError;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let (frames, window) = ctx.flags;
        Ok(Self {
            frames,
            window,
            taken: None,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        _control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        // Frames come faster than asked (Windows before 24H2 ignores the minimum update
        // interval): one not due is left before its copy is made.
        if self.taken.is_some_and(|taken| taken.elapsed() < INTERVAL) {
            return Ok(());
        }
        self.taken = Some(Instant::now());
        // The frame holds the title bar and a border around the client area.
        let mut buffer = frame.buffer()?;
        let (width, height) = (buffer.width(), buffer.height());
        let client =
            client_area(self.window).filter(|&((l, t, w, h), _)| l + w <= width && t + h <= height);
        let (left, top, w, h) = client.map_or((0, 0, width, height), |(area, _)| area);
        let pixels = buffer.as_nopadding_buffer()?;
        if let Some(image) = RgbImage::from_raw(w, h, client_rgb(pixels, width, (left, top, w, h)))
        {
            // A frame come while the last is still being read is left out.
            let _ = self.frames.try_send(Shot {
                image,
                origin: client.map(|(_, origin)| origin),
                in_front: in_front(self.window),
            });
        }
        Ok(())
    }
}

/// Whether a process's exe path is the game's.
fn is_game(path: &str) -> bool {
    path.rsplit('\\')
        .next()
        .is_some_and(|name| name.eq_ignore_ascii_case(GAME))
}

/// The path of a process's exe. The game runs as administrator, which denies
/// `Window::process_name`; the limited query is allowed.
fn process_path(pid: u32) -> Option<String> {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{
            OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        },
    };
    let mut buffer = [0u16; 1024];
    let mut len = buffer.len() as u32;
    // SAFETY: the handle is checked and closed; the buffer outlives the call and `len`
    // holds its size.
    let ok = unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let ok = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            buffer.as_mut_ptr(),
            &raw mut len,
        );
        CloseHandle(process);
        ok
    };
    (ok != 0).then(|| String::from_utf16_lossy(&buffer[..len as usize]))
}

fn game_window() -> Option<Window> {
    Window::enumerate().ok()?.into_iter().find(|window| {
        window
            .process_id()
            .ok()
            .and_then(process_path)
            .is_some_and(|path| is_game(&path))
            && window.title().is_ok_and(|title| !title.is_empty())
    })
}

fn capture(
    window: Window,
    frames: SyncSender<Shot>,
) -> Result<CaptureControl<Capture, CaptureError>, String> {
    let settings = Settings::new(
        window,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Custom(INTERVAL),
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        (frames, window.as_raw_hwnd() as isize),
    );
    Capture::start_free_threaded(settings).map_err(|err| err.to_string())
}

/// Keeps Windows from running the app at efficiency speeds (`EcoQoS`) while its window is not
/// in front: the frames then take longer to read. Aoi's GPUI window was slowed so on
/// 2026-11-07, reading the same frames at 178% of a core against 147%.
fn run_at_full_speed() {
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE,
        ProcessPowerThrottling, SetProcessInformation,
    };
    // Controlled (the mask), and off (the state).
    let state = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        StateMask: 0,
    };
    // SAFETY: the state outlives the call, which reads its size in bytes of it. A refusal
    // only leaves the reading slower.
    unsafe {
        SetProcessInformation(
            GetCurrentProcess(),
            ProcessPowerThrottling,
            (&raw const state).cast(),
            size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        );
    }
}

/// The frames of the game's window, several a second while it is open; the window is looked
/// for again whenever it is gone. What happens to the window is told through `tell`.
pub fn frames(tell: impl Fn(Seen) + Send + 'static) -> Receiver<Shot> {
    let (sender, receiver) = sync_channel(1);
    thread::spawn(move || {
        run_at_full_speed();
        let mut session: Option<(Window, CaptureControl<Capture, CaptureError>)> = None;
        let mut failed = false;
        loop {
            if session
                .as_ref()
                .is_some_and(|(window, control)| !window.is_valid() || control.is_finished())
            {
                if let Some((_, control)) = session.take() {
                    let _ = control.stop();
                }
                tell(Seen::Gone);
            }
            if session.is_none()
                && let Some(window) = game_window()
            {
                match capture(window, sender.clone()) {
                    Ok(control) => {
                        session = Some((window, control));
                        failed = false;
                        tell(Seen::Found);
                    }
                    // Told once, tried again every look.
                    Err(message) if !failed => {
                        failed = true;
                        tell(Seen::Failed(message));
                    }
                    Err(_) => {}
                }
            }
            thread::sleep(LOOK_INTERVAL);
        }
    });
    receiver
}

#[cfg(test)]
mod tests {
    use super::{client_rgb, is_game};

    #[test]
    fn the_game_is_known_by_its_exe_name() {
        assert!(is_game(r"D:\Games\BlueArchive\BlueArchive.exe"));
        assert!(is_game(r"C:\Nexon\bluearchive.EXE"));
        assert!(!is_game(r"C:\Tools\BlueArchive.exe.bak"));
        assert!(!is_game(r"C:\Tools\NotBlueArchive.exe"));
    }

    #[test]
    fn the_client_area_is_cut_out_of_the_windows_frame() {
        // A frame 3 wide and 2 tall in RGBA, each pixel's channels its number.
        let frame: Vec<u8> = (0..6u8).flat_map(|n| [n, n, n, 255]).collect();
        assert_eq!(
            client_rgb(&frame, 3, (1, 0, 2, 2)),
            [1, 1, 1, 2, 2, 2, 4, 4, 4, 5, 5, 5],
            "the two right columns, without the alpha"
        );
    }
}
