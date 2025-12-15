//! macOS Scale Factor Window Size Restoration Bug
//!
//! This example demonstrates a bug where macOS/AppKit incorrectly restores window size
//! when a window is programmatically resized on a low-DPI monitor and then manually
//! dragged back to a high-DPI (Retina) monitor.
//!
//! ## Requirements
//!
//! - macOS with dual monitor setup with DIFFERENT scale factors
//! - e.g., Monitor 0: High-DPI (Retina, scale factor 2.0)
//! - e.g., Monitor 1: Low-DPI (external, scale factor 1.0)
//!
//! ## Steps to Reproduce
//!
//! 1. Launch this example on the higher-scale monitor (Retina display)
//!    - Window appears at default 800x600 logical size
//!
//! 2. Press 'R' to trigger programmatic restore simulation
//!    - Window automatically moves to the other monitor and resizes to 600x400
//!    - Observe log: window is now 600x400 on the different-scale monitor
//!
//! 3. Manually drag the window back to the original monitor
//!    - BUG: Window jumps to 800x600 instead of staying at 600x400
//!    - Expected: Window should remain 600x400 logical
//!
//! ## Root Cause
//!
//! AppKit internally tracks "user intended size" per scale factor. When a window is
//! created at size X on scale=2, AppKit remembers X as the intended size for scale=2.
//! Programmatic resizes via `setContentSize` on scale=1 do NOT update AppKit's memory
//! of the scale=2 size. When the window returns to scale=2, AppKit "helpfully" restores
//! the original size.
//!
//! Manual resize (via `windowDidEndLiveResize`) DOES update AppKit's tracking, which is
//! why manually resizing the window before dragging back prevents the bug.
//!
//! ## Workaround
//!
//! If you manually resize the window (even slightly) on the target monitor before
//! dragging back, the bug does not occur - the window correctly maintains its size.

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::monitor::MonitorHandle;
use winit::window::{Window, WindowId};

/// Default window size (will be restored incorrectly after cross-monitor drag)
const DEFAULT_SIZE: LogicalSize<u32> = LogicalSize::new(800, 600);

/// Target size for "restore" operation (physical pixels, smaller than default to make bug obvious)
/// Using physical size so it's consistent regardless of scale factor at time of call
const RESTORE_SIZE: PhysicalSize<u32> = PhysicalSize::new(600, 400);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("=== macOS Scale Factor Window Size Restoration Bug ===");
    eprintln!();
    eprintln!("Instructions:");
    eprintln!("  1. Launch on higher-scale monitor (e.g., Retina, scale 2.0)");
    eprintln!("  2. Press 'R' to move to different-scale monitor and resize");
    eprintln!("  3. Drag window back to original monitor");
    eprintln!("  4. BUG: Window resets to {}x{} logical instead of staying {}x{} physical",
        DEFAULT_SIZE.width, DEFAULT_SIZE.height,
        RESTORE_SIZE.width, RESTORE_SIZE.height);
    eprintln!();
    eprintln!("Press 'Q' to quit");
    eprintln!();

    let event_loop = EventLoop::new()?;
    let mut app = App {
        window: None,
        target_monitor: None,
        initial_scale: None,
        restore_triggered: false,
        pending_resize: None,
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    window: Option<Window>,
    /// Monitor with different scale factor to move to
    target_monitor: Option<MonitorHandle>,
    /// Scale factor of the initial monitor
    initial_scale: Option<f64>,
    /// Has the restore operation been triggered?
    restore_triggered: bool,
    /// Pending resize to apply after scale change
    pending_resize: Option<PhysicalSize<u32>>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        // Enumerate monitors and find one with a different scale factor
        let monitors: Vec<MonitorHandle> = event_loop.available_monitors().collect();

        eprintln!("[MONITORS] Found {} monitor(s):", monitors.len());
        for (i, monitor) in monitors.iter().enumerate() {
            let pos = monitor.position();
            let size = monitor.size();
            let scale = monitor.scale_factor();
            eprintln!("  Monitor {}: {}x{} at ({}, {}), scale={}",
                i, size.width, size.height, pos.x, pos.y, scale);
        }

        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("macOS Scale Restore Bug")
                    .with_inner_size(DEFAULT_SIZE),
            )
            .expect("failed to create window");

        let current_scale = window.scale_factor();
        let size = window.inner_size();
        eprintln!();
        eprintln!("[INIT] Window created: {}x{} physical, scale={}", size.width, size.height, current_scale);

        // Find a monitor with a different scale factor
        let current_monitor = window.current_monitor();
        let target = monitors.iter().find(|m| {
            (m.scale_factor() - current_scale).abs() > 0.1
        }).cloned();

        if let Some(ref target) = target {
            let pos = target.position();
            let tsize = target.size();
            eprintln!("[TARGET] Will move to monitor at ({}, {}) {}x{} scale={}",
                pos.x, pos.y, tsize.width, tsize.height, target.scale_factor());
        } else {
            eprintln!();
            eprintln!("!!! WARNING: No monitor with different scale factor found !!!");
            eprintln!("!!! This bug requires dual monitors with different DPI !!!");
            if let Some(cm) = current_monitor {
                eprintln!("!!! Current monitor scale: {} !!!", cm.scale_factor());
            }
        }

        self.window = Some(window);
        self.target_monitor = target;
        self.initial_scale = Some(current_scale);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let window = match &self.window {
            Some(w) => w,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                match event.logical_key.as_ref() {
                    Key::Character("r") | Key::Character("R") => {
                        let target = match &self.target_monitor {
                            Some(t) => t,
                            None => {
                                eprintln!();
                                eprintln!("[ERROR] No target monitor with different scale factor!");
                                eprintln!("[ERROR] Cannot reproduce bug without dual monitors.");
                                return;
                            }
                        };

                        // Calculate position: center of target monitor
                        let target_pos = target.position();
                        let target_size = target.size();
                        let window_size = RESTORE_SIZE;

                        // Position window in center of target monitor
                        let x = target_pos.x + (target_size.width as i32 - window_size.width as i32) / 2;
                        let y = target_pos.y + (target_size.height as i32 - window_size.height as i32) / 2;
                        let new_position = PhysicalPosition::new(x, y);

                        eprintln!();
                        eprintln!("[RESTORE] Moving to target monitor at {:?}", new_position);
                        eprintln!("[RESTORE] Will resize to {}x{} physical AFTER scale change", RESTORE_SIZE.width, RESTORE_SIZE.height);

                        // Set position first (move to target monitor)
                        window.set_outer_position(new_position);

                        // Defer resize until after scale change
                        self.pending_resize = Some(RESTORE_SIZE);
                        self.restore_triggered = true;
                    }

                    Key::Character("q") | Key::Character("Q") | Key::Named(NamedKey::Escape) => {
                        event_loop.exit();
                    }

                    _ => {}
                }
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                eprintln!("[SCALE] Scale factor changed to {}", scale_factor);

                // Apply pending resize now that we're on the target monitor with correct scale
                if let Some(size) = self.pending_resize.take() {
                    eprintln!("[DEFERRED RESIZE] Now applying {}x{} physical at scale={}",
                        size.width, size.height, scale_factor);
                    let result = window.request_inner_size(size);
                    eprintln!("[DEFERRED RESIZE] request_inner_size returned: {:?}", result);
                    eprintln!();
                    eprintln!(">>> Now drag the window back to the original monitor <<<");
                    eprintln!(">>> Watch for incorrect size restoration <<<");
                }
            }

            WindowEvent::Resized(size) => {
                let scale = window.scale_factor();
                let logical_w = size.width as f64 / scale;
                let logical_h = size.height as f64 / scale;
                eprintln!(
                    "[RESIZE] {}x{} physical ({}x{} logical at scale {})",
                    size.width, size.height, logical_w as u32, logical_h as u32, scale
                );

                // Flag if we got the default size back (the bug)
                // Only check after restore was triggered AND we're back on original scale
                if let Some(initial_scale) = self.initial_scale {
                    if self.restore_triggered && (scale - initial_scale).abs() < 0.1 {
                        let default_physical_w = (DEFAULT_SIZE.width as f64 * scale) as u32;
                        let default_physical_h = (DEFAULT_SIZE.height as f64 * scale) as u32;

                        // Bug: size reset to default instead of staying at RESTORE_SIZE
                        if size.width == default_physical_w && size.height == default_physical_h {
                            eprintln!();
                            eprintln!("!!! BUG DETECTED: Window reset to default size {}x{} physical !!!",
                                size.width, size.height);
                            eprintln!("!!! Expected to stay near {}x{} physical !!!",
                                RESTORE_SIZE.width, RESTORE_SIZE.height);
                        }
                    }
                }
            }

            WindowEvent::Moved(pos) => {
                let scale = window.scale_factor();
                eprintln!("[MOVE] Position: ({}, {}) scale={}", pos.x, pos.y, scale);
            }

            _ => {}
        }
    }
}
