//! X11: Missing Moved event when WM moves+resizes window
//!
//! When a WM moves and resizes a window together (e.g., keyboard snap/tile),
//! only SurfaceResized is emitted - Moved is missing even though position changed.
//!
//! ## Reproduce
//!
//! 1. `WAYLAND_DISPLAY= cargo run --example x11_missing_move_on_resize`
//! 2. Use keyboard snap (KDE: Meta+Arrow, GNOME: Super+Arrow)
//!
//! ## Workaround
//!
//! Query `window.outer_position()` after resize events.

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("WAYLAND_DISPLAY")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        eprintln!("!!! WARNING: Running on Wayland - this bug is X11-specific !!!");
        eprintln!("!!! Run with: WAYLAND_DISPLAY= cargo run --example x11_missing_move_on_resize !!!");
        eprintln!();
    }

    eprintln!("=== X11: Missing Moved on Resize ===");
    eprintln!();
    eprintln!("Use keyboard snap (KDE: Meta+Arrow, GNOME: Super+Arrow)");
    eprintln!("BUG: SurfaceResized fires but Moved does not when position changes");
    eprintln!();
    eprintln!("Press 'Q' to quit");
    eprintln!();

    let event_loop = EventLoop::new()?;
    event_loop.run_app(App::default())?;
    Ok(())
}

/// Position change detection state machine
#[derive(Debug, Clone)]
enum PositionState {
    /// No position change pending - normal operation
    Idle {
        last_position: Option<(i32, i32)>,
    },
    /// Resize changed position - waiting to see if Moved event follows
    /// If Moved arrives: transition to Idle (not a bug)
    /// If any other event arrives: report bug, transition to Idle
    WaitingForMove {
        last_position: (i32, i32),
        actual_position: (i32, i32),
    },
}

impl Default for PositionState {
    fn default() -> Self {
        Self::Idle { last_position: None }
    }
}

impl PositionState {
    fn last_position(&self) -> Option<(i32, i32)> {
        match self {
            Self::Idle { last_position } => *last_position,
            Self::WaitingForMove { last_position, .. } => Some(*last_position),
        }
    }
}

#[derive(Default)]
struct App {
    window: Option<Box<dyn Window>>,
    state: PositionState,
    bug_count: u32,
}

impl App {
    /// Handle Moved event - position update from winit
    fn on_moved(&mut self, pos: (i32, i32)) {
        // Moved arrived - if we were waiting, this means resize+move came together (not a bug)
        self.state = PositionState::Idle {
            last_position: Some(pos),
        };
    }

    /// Handle Resize event - check if position changed
    fn on_resize(&mut self, actual_position: (i32, i32)) {
        // First, check if we were waiting for a move that never came
        if let PositionState::WaitingForMove { last_position, actual_position: pending_pos } = self.state {
            self.report_bug(last_position, pending_pos);
        }

        // Now check if this resize changed position
        let last = self.state.last_position();
        if let Some(last_pos) = last {
            if actual_position != last_pos {
                // Position changed - wait to see if Moved follows
                self.state = PositionState::WaitingForMove {
                    last_position: last_pos,
                    actual_position,
                };
                return;
            }
        }

        // No position change, stay/go to Idle
        self.state = PositionState::Idle {
            last_position: last.or(Some(actual_position)),
        };
    }

    /// Handle any other event - if waiting for move, report bug
    fn on_other_event(&mut self) {
        if let PositionState::WaitingForMove { last_position, actual_position } = self.state {
            self.report_bug(last_position, actual_position);
            self.state = PositionState::Idle {
                last_position: Some(actual_position),
            };
        }
    }

    fn report_bug(&mut self, last_position: (i32, i32), actual_position: (i32, i32)) {
        self.bug_count += 1;
        eprintln!();
        eprintln!("!!! BUG: SurfaceResized without Moved !!!");
        eprintln!("    Last Moved: ({}, {})", last_position.0, last_position.1);
        eprintln!("    Actual now: ({}, {})", actual_position.0, actual_position.1);
    }
}

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window_attributes = WindowAttributes::default()
            .with_title("X11: Missing Moved on Resize")
            .with_surface_size(LogicalSize::new(800, 600))
            .with_resizable(true);

        let window = match event_loop.create_window(window_attributes) {
            Ok(w) => w,
            Err(err) => {
                eprintln!("Failed to create window: {err}");
                event_loop.exit();
                return;
            }
        };

        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let window = match &self.window {
            Some(w) => w,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => {
                self.on_other_event();
                eprintln!();
                eprintln!("=== Summary ===");
                eprintln!("Missing Moved events detected: {}", self.bug_count);
                event_loop.exit();
            }

            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.on_other_event();
                if matches!(
                    event.logical_key.as_ref(),
                    Key::Character("q") | Key::Character("Q") | Key::Named(NamedKey::Escape)
                ) {
                    eprintln!();
                    eprintln!("=== Summary ===");
                    eprintln!("Missing Moved events detected: {}", self.bug_count);
                    event_loop.exit();
                }
            }

            WindowEvent::Moved(pos) => {
                self.on_moved((pos.x, pos.y));
            }

            WindowEvent::SurfaceResized(_) => {
                let actual = window.outer_position().ok();
                if let Some(pos) = actual {
                    self.on_resize((pos.x, pos.y));
                }
                self.window.as_ref().unwrap().request_redraw();
            }

            WindowEvent::RedrawRequested => {
                self.on_other_event();
                let window = self.window.as_ref().unwrap();
                window.pre_present_notify();
                fill::fill_window(window.as_ref());
            }

            _ => {}
        }
    }
}
