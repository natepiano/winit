//! Types useful for interacting with a user's monitors.
//!
//! If you want to get basic information about a monitor, you can use the
//! [`MonitorHandle`] type. This is retrieved from one of the following
//! methods, which return an iterator of [`MonitorHandle`]:
//! - [`ActiveEventLoop::available_monitors`][crate::event_loop::ActiveEventLoop::available_monitors].
//! - [`Window::available_monitors`][crate::window::Window::available_monitors].
use crate::dpi::{PhysicalPosition, PhysicalSize, Position};
use crate::platform_impl;

/// Deprecated! Use `VideoModeHandle` instead.
#[deprecated = "Renamed to `VideoModeHandle`"]
pub type VideoMode = VideoModeHandle;

/// Describes a fullscreen video mode of a monitor.
///
/// Can be acquired with [`MonitorHandle::video_modes`].
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct VideoModeHandle {
    pub(crate) video_mode: platform_impl::VideoModeHandle,
}

impl std::fmt::Debug for VideoModeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.video_mode.fmt(f)
    }
}

impl PartialOrd for VideoModeHandle {
    fn partial_cmp(&self, other: &VideoModeHandle) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VideoModeHandle {
    fn cmp(&self, other: &VideoModeHandle) -> std::cmp::Ordering {
        self.monitor().cmp(&other.monitor()).then(
            self.size()
                .cmp(&other.size())
                .then(
                    self.refresh_rate_millihertz()
                        .cmp(&other.refresh_rate_millihertz())
                        .then(self.bit_depth().cmp(&other.bit_depth())),
                )
                .reverse(),
        )
    }
}

impl VideoModeHandle {
    /// Returns the resolution of this video mode.
    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        self.video_mode.size()
    }

    /// Returns the bit depth of this video mode, as in how many bits you have
    /// available per color. This is generally 24 bits or 32 bits on modern
    /// systems, depending on whether the alpha channel is counted or not.
    ///
    /// ## Platform-specific
    ///
    /// - **Wayland / Orbital:** Always returns 32.
    /// - **iOS:** Always returns 32.
    #[inline]
    pub fn bit_depth(&self) -> u16 {
        self.video_mode.bit_depth()
    }

    /// Returns the refresh rate of this video mode in mHz.
    #[inline]
    pub fn refresh_rate_millihertz(&self) -> u32 {
        self.video_mode.refresh_rate_millihertz()
    }

    /// Returns the monitor that this video mode is valid for. Each monitor has
    /// a separate set of valid video modes.
    #[inline]
    pub fn monitor(&self) -> MonitorHandle {
        MonitorHandle { inner: self.video_mode.monitor() }
    }
}

impl std::fmt::Display for VideoModeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}x{} @ {} mHz ({} bpp)",
            self.size().width,
            self.size().height,
            self.refresh_rate_millihertz(),
            self.bit_depth()
        )
    }
}

/// Handle to a monitor.
///
/// Allows you to retrieve information about a given monitor and can be used in [`Window`] creation.
///
/// [`Window`]: crate::window::Window
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MonitorHandle {
    pub(crate) inner: platform_impl::MonitorHandle,
}

impl MonitorHandle {
    /// Returns a human-readable name of the monitor.
    ///
    /// Returns `None` if the monitor doesn't exist anymore.
    #[inline]
    pub fn name(&self) -> Option<String> {
        self.inner.name()
    }

    /// Returns the monitor's resolution.
    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        self.inner.size()
    }

    /// Returns the top-left corner position of the monitor relative to the larger full
    /// screen area.
    #[inline]
    pub fn position(&self) -> PhysicalPosition<i32> {
        self.inner.position()
    }

    /// The monitor refresh rate used by the system.
    ///
    /// Return `Some` if succeed, or `None` if failed, which usually happens when the monitor
    /// the window is on is removed.
    ///
    /// When using exclusive fullscreen, the refresh rate of the [`VideoModeHandle`] that was
    /// used to enter fullscreen should be used instead.
    #[inline]
    pub fn refresh_rate_millihertz(&self) -> Option<u32> {
        self.inner.refresh_rate_millihertz()
    }

    /// Returns the scale factor of the underlying monitor. To map logical pixels to physical
    /// pixels and vice versa, use [`Window::scale_factor`].
    ///
    /// See the [`dpi`] module for more information.
    ///
    /// ## Platform-specific
    ///
    /// - **X11:** Can be overridden using the `WINIT_X11_SCALE_FACTOR` environment variable.
    /// - **Wayland:** May differ from [`Window::scale_factor`].
    /// - **Android:** Always returns 1.0.
    ///
    /// [`Window::scale_factor`]: crate::window::Window::scale_factor
    #[inline]
    pub fn scale_factor(&self) -> f64 {
        self.inner.scale_factor()
    }

    /// Returns all fullscreen video modes supported by this monitor.
    ///
    /// ## Platform-specific
    ///
    /// - **Web:** Always returns an empty iterator
    #[inline]
    pub fn video_modes(&self) -> impl Iterator<Item = VideoModeHandle> {
        self.inner.video_modes().map(|video_mode| VideoModeHandle { video_mode })
    }
}

/// A monitor's logical bounds and scale factor, used for determining which
/// monitor a physical or logical position targets.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MonitorBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
}

impl MonitorBounds {
    /// Create `MonitorBounds` from physical position, physical size, and scale factor.
    ///
    /// All platforms report monitor position and size in physical pixels. This
    /// constructor converts to logical coordinates by dividing by the scale factor,
    /// producing the uniform logical-space representation that
    /// `scale_factor_for_physical_position` and `scale_factor_for_logical_position` expect.
    pub fn from_physical(
        position: PhysicalPosition<i32>,
        size: PhysicalSize<u32>,
        scale: f64,
    ) -> Self {
        Self {
            x: position.x as f64 / scale,
            y: position.y as f64 / scale,
            width: size.width as f64 / scale,
            height: size.height as f64 / scale,
            scale,
        }
    }
}

/// Determine the scale factor of the target monitor for a given position.
///
/// Monitor bounds are in logical coordinates. For `Physical` positions, each
/// monitor's scale factor is used to convert to logical before checking bounds.
/// For `Logical` positions, bounds are checked directly.
///
/// Returns `None` if no monitor contains the position.
pub(crate) fn resolve_scale_factor(position: &Position, monitors: &[MonitorBounds]) -> Option<f64> {
    for monitor in monitors {
        if monitor.width <= 0.0 || monitor.scale <= 0.0 {
            continue;
        }

        let (logical_x, logical_y) = match position {
            Position::Physical(p) => (p.x as f64 / monitor.scale, p.y as f64 / monitor.scale),
            Position::Logical(l) => (l.x, l.y),
        };

        if logical_x >= monitor.x
            && logical_x < monitor.x + monitor.width
            && logical_y >= monitor.y
            && logical_y < monitor.y + monitor.height
        {
            return Some(monitor.scale);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use crate::dpi::{LogicalPosition, PhysicalPosition};

    use super::*;

    fn monitor(x: f64, y: f64, width: f64, height: f64, scale: f64) -> MonitorBounds {
        MonitorBounds { x, y, width, height, scale }
    }

    fn physical(x: i32, y: i32) -> Position {
        Position::Physical(PhysicalPosition::new(x, y))
    }

    fn logical(x: f64, y: f64) -> Position {
        Position::Logical(LogicalPosition::new(x, y))
    }

    // Typical macOS layout: Retina MacBook (2x) as primary at origin,
    // external 1x display to the upper-left.
    //
    // CG coordinate space (logical points, origin top-left of primary):
    //   Monitor 0 (2x): (0, 0) 1728x1117
    //   Monitor 1 (1x): (-816, -1440) 3440x1440
    fn macos_layout() -> Vec<MonitorBounds> {
        vec![monitor(0.0, 0.0, 1728.0, 1117.0, 2.0), monitor(-816.0, -1440.0, 3440.0, 1440.0, 1.0)]
    }

    // --- Physical position tests ---

    #[test]
    fn physical_position_on_2x_monitor() {
        let monitors = macos_layout();
        let result = resolve_scale_factor(&physical(400, 600), &monitors);
        assert_eq!(result, Some(2.0));
    }

    #[test]
    fn physical_position_on_1x_monitor() {
        let monitors = macos_layout();
        let result = resolve_scale_factor(&physical(-500, -1000), &monitors);
        assert_eq!(result, Some(1.0));
    }

    #[test]
    fn high_to_low_cross_monitor_restore() {
        let monitors = macos_layout();
        let result = resolve_scale_factor(&physical(-716, -1340), &monitors);
        assert_eq!(result, Some(1.0));
    }

    #[test]
    fn low_to_high_cross_monitor_restore() {
        let monitors = macos_layout();
        let result = resolve_scale_factor(&physical(400, 400), &monitors);
        assert_eq!(result, Some(2.0));
    }

    #[test]
    fn physical_position_outside_all_monitors_returns_none() {
        let monitors = macos_layout();
        let result = resolve_scale_factor(&physical(99999, 99999), &monitors);
        assert_eq!(result, None);
    }

    #[test]
    fn single_monitor_always_matches() {
        let monitors = vec![monitor(0.0, 0.0, 1920.0, 1080.0, 1.0)];
        let result = resolve_scale_factor(&physical(500, 300), &monitors);
        assert_eq!(result, Some(1.0));
    }

    #[test]
    fn single_retina_monitor() {
        let monitors = vec![monitor(0.0, 0.0, 1728.0, 1117.0, 2.0)];
        let result = resolve_scale_factor(&physical(1000, 800), &monitors);
        assert_eq!(result, Some(2.0));
    }

    #[test]
    fn three_monitors_mixed_scales() {
        let monitors = vec![
            monitor(-1920.0, 0.0, 1920.0, 1080.0, 1.0),
            monitor(0.0, 0.0, 1728.0, 1117.0, 2.0),
            monitor(1728.0, 0.0, 2560.0, 1440.0, 1.5),
        ];

        let result = resolve_scale_factor(&physical(-1000, 500), &monitors);
        assert_eq!(result, Some(1.0));

        let result = resolve_scale_factor(&physical(1000, 800), &monitors);
        assert_eq!(result, Some(2.0));

        let result = resolve_scale_factor(&physical(5000, 600), &monitors);
        assert_eq!(result, Some(1.5));
    }

    #[test]
    fn zero_width_monitor_is_skipped() {
        let monitors =
            vec![monitor(0.0, 0.0, 0.0, 0.0, 2.0), monitor(0.0, 0.0, 1920.0, 1080.0, 1.0)];
        let result = resolve_scale_factor(&physical(500, 300), &monitors);
        assert_eq!(result, Some(1.0));
    }

    #[test]
    fn zero_scale_monitor_is_skipped() {
        let monitors =
            vec![monitor(0.0, 0.0, 1920.0, 1080.0, 0.0), monitor(0.0, 0.0, 1920.0, 1080.0, 1.0)];
        let result = resolve_scale_factor(&physical(500, 300), &monitors);
        assert_eq!(result, Some(1.0));
    }

    #[test]
    fn empty_monitor_list_returns_none() {
        let result = resolve_scale_factor(&physical(500, 300), &[]);
        assert_eq!(result, None);
    }

    // --- Logical position tests ---

    #[test]
    fn logical_position_on_2x_monitor() {
        let monitors = macos_layout();
        let result = resolve_scale_factor(&logical(200.0, 300.0), &monitors);
        assert_eq!(result, Some(2.0));
    }

    #[test]
    fn logical_position_on_1x_monitor() {
        let monitors = macos_layout();
        let result = resolve_scale_factor(&logical(-500.0, -1000.0), &monitors);
        assert_eq!(result, Some(1.0));
    }

    #[test]
    fn logical_position_outside_all_monitors_returns_none() {
        let monitors = macos_layout();
        let result = resolve_scale_factor(&logical(99999.0, 99999.0), &monitors);
        assert_eq!(result, None);
    }

    #[test]
    fn logical_position_three_monitors_mixed_scales() {
        let monitors = vec![
            monitor(-1920.0, 0.0, 1920.0, 1080.0, 1.0),
            monitor(0.0, 0.0, 1728.0, 1117.0, 2.0),
            monitor(1728.0, 0.0, 2560.0, 1440.0, 1.5),
        ];

        let result = resolve_scale_factor(&logical(-1000.0, 500.0), &monitors);
        assert_eq!(result, Some(1.0));

        let result = resolve_scale_factor(&logical(500.0, 500.0), &monitors);
        assert_eq!(result, Some(2.0));

        let result = resolve_scale_factor(&logical(2000.0, 500.0), &monitors);
        assert_eq!(result, Some(1.5));
    }

    #[test]
    fn logical_empty_monitor_list_returns_none() {
        let result = resolve_scale_factor(&logical(500.0, 300.0), &[]);
        assert_eq!(result, None);
    }
}
