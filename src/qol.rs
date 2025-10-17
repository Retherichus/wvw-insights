use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use winapi::shared::windef::{HWND, RECT};
use winapi::um::winuser::{
    ClipCursor, GetForegroundWindow, GetWindowRect,
};

static MOUSE_LOCK_ACTIVE: AtomicBool = AtomicBool::new(false);
static GW2_WINDOW: AtomicUsize = AtomicUsize::new(0);

/// Initializes the GW2 window handle - should be called once at startup
pub fn init_window_handle() {
    unsafe {
        let hwnd = GetForegroundWindow();
        if !hwnd.is_null() {
            GW2_WINDOW.store(hwnd as usize, Ordering::Relaxed);
            log::info!("Captured GW2 window handle: {:?}", hwnd);
        }
    }
}

/// Enables mouse locking to the game window
pub fn enable_mouse_lock() {
    MOUSE_LOCK_ACTIVE.store(true, Ordering::Relaxed);
    // Capture the window handle immediately when enabling
    // This allows toggling to work in real-time without plugin reload
    init_window_handle();
    log::info!("Mouse lock enabled");
}

/// Disables mouse locking
pub fn disable_mouse_lock() {
    MOUSE_LOCK_ACTIVE.store(false, Ordering::Relaxed);
    unsafe {
        ClipCursor(std::ptr::null());
    }
    log::info!("Mouse lock disabled");
}

/// Checks if the GW2 window is focused and applies/removes mouse lock accordingly
/// This should be called every frame from the render function
pub fn update_mouse_lock() {
    if !MOUSE_LOCK_ACTIVE.load(Ordering::Relaxed) {
        return;
    }

    unsafe {
        let gw2_hwnd_val = GW2_WINDOW.load(Ordering::Relaxed);
        
        // If we don't have the GW2 window handle yet, try to capture it
        if gw2_hwnd_val == 0 {
            init_window_handle();
            return;
        }
        
        let gw2_hwnd = gw2_hwnd_val as HWND;
        let foreground_hwnd = GetForegroundWindow();
        
        // Only lock if GW2 is the foreground window
        if foreground_hwnd == gw2_hwnd {
            let mut rect: RECT = std::mem::zeroed();
            if GetWindowRect(gw2_hwnd, &mut rect) != 0 {
                ClipCursor(&rect);
            }
        } else {
            // GW2 is not focused, release the lock
            ClipCursor(std::ptr::null());
        }
    }
}