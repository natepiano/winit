//! Demonstrates a bug where `outer_position()` returns a different value than what was set via
//! `set_outer_position()` on X11.
//!
//! # The Bug
//!
//! On X11, after calling `set_outer_position(position)`, both `outer_position()` and
//! `WindowEvent::Moved` report a position offset by the window manager's title bar height.
//!
//! # Expected Behavior
//!
//! - `set_outer_position(pos)` sets the window frame position to `pos`
//! - `outer_position()` returns `pos`
//! - `WindowEvent::Moved` reports `pos`
//!
//! # Actual Behavior
//!
//! - `set_outer_position(pos)` sets the window frame position to `pos`
//! - `outer_position()` returns `pos + (0, title_bar_height)` (wrong!)
//! - `WindowEvent::Moved` reports `pos + (0, title_bar_height)` (wrong!)
//!
//! # Reproduction
//!
//! Run with X11 (not Wayland):
//! ```
//! WAYLAND_DISPLAY= cargo run --example x11_outer_position_mismatch
//! ```
//!
//! Press 'T' to test. The example will:
//! 1. Call `set_outer_position` with a known value
//! 2. Wait for `WindowEvent::Moved`
//! 3. Compare the reported position with what was set
//!
//! # Impact
//!
//! Applications cannot reliably read back the position they set. Any code that sets a position
//! and later queries it will see a different value, offset by the title bar height.

use std::error::Error;

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::dpi::Position;
use winit::event::ElementState;
use winit::event::KeyEvent;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::EventLoop;
use winit::keyboard::Key;
use winit::keyboard::NamedKey;
use winit::window::Window;
use winit::window::WindowAttributes;
use winit::window::WindowId;

#[path = "util/fill.rs"]
mod fill;
#[path = "util/tracing.rs"]
mod tracing;

#[derive(Default)]
struct App {
    window: Option<Box<dyn Window>>,
    test_count: u32,
    pending_test: Option<PhysicalPosition<i32>>,
}

impl App {
    fn run_test(&mut self) {
        let Some(window) = self.window.as_ref() else {
            eprintln!("No window available");
            return;
        };

        self.test_count += 1;
        let test_y = 200 + (self.test_count as i32 * 50); // Different Y each test
        let test_position = PhysicalPosition::new(200, test_y);

        eprintln!("\n=== Test {} ===", self.test_count);
        eprintln!("Calling set_outer_position({:?})", test_position);

        self.pending_test = Some(test_position);
        window.set_outer_position(Position::Physical(test_position));
    }

    fn check_moved(&mut self, reported_position: PhysicalPosition<i32>) {
        let Some(expected) = self.pending_test.take() else {
            return;
        };

        let delta_y = reported_position.y - expected.y;

        if delta_y == 0 {
            eprintln!("OK: Position matches what was set.");
            return;
        }

        eprintln!("Moved event reported ({}, {}) - Y is offset by {} pixels from what we set.\n",
            reported_position.x, reported_position.y, delta_y);

        // Query positions to show the state
        if let Some(window) = self.window.as_ref() {
            let outer = window.outer_position().ok();
            let surface = window.surface_position();
            eprintln!("Querying positions after the move:");
            eprintln!("  outer_position()   = {:?}", outer);
            eprintln!("  surface_position() = {:?}", surface);
            eprintln!();
            eprintln!("On this system, every set_outer_position/outer_position cycle");
            eprintln!("adds {} pixels to Y. With surface_position() returning (0, 0),", delta_y);
            eprintln!("there's no way to determine the actual frame position.");
        }
    }
}

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let window_attributes = WindowAttributes::default()
            .with_title("X11 outer_position mismatch - Press T to test");

        self.window = match event_loop.create_window(window_attributes) {
            Ok(window) => {
                eprintln!("Window created. Press 'T' to test the outer_position bug.");
                eprintln!("Run with: WAYLAND_DISPLAY= cargo run --example x11_outer_position_mismatch\n");
                Some(window)
            }
            Err(err) => {
                eprintln!("Error creating window: {err}");
                event_loop.exit();
                return;
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        _: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Moved(position) => {
                self.check_moved(PhysicalPosition::new(position.x, position.y));
            }
            WindowEvent::KeyboardInput {
                event: KeyEvent { logical_key, state: ElementState::Pressed, .. },
                ..
            } => match logical_key.as_ref() {
                Key::Character("t") | Key::Character("T") => {
                    self.run_test();
                }
                Key::Named(NamedKey::Escape) => {
                    event_loop.exit();
                }
                _ => {}
            },
            WindowEvent::SurfaceResized(_) => {
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(window) = self.window.as_ref() {
                    window.pre_present_notify();
                    fill::fill_window(window.as_ref());
                }
            }
            _ => {}
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    tracing::init();

    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        eprintln!("WARNING: WAYLAND_DISPLAY is set. This bug is X11-specific.");
        eprintln!("Run with: WAYLAND_DISPLAY= cargo run --example x11_outer_position_mismatch\n");
    }

    let event_loop = EventLoop::new()?;
    event_loop.run_app(App::default())?;

    Ok(())
}
