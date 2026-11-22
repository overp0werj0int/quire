//! Pointer gestures: the hold, fingers resting on a touchpad without moving, which apps take as
//! the signal to stop kinetic scrolling (`zwp_pointer_gesture_hold_v1`, version 3), and, for
//! Quire, the pinch (`zwp_pointer_gesture_pinch_v1`), sent as `PinchGesture` as macOS sends it.

use std::ops::Deref;
use std::sync::Mutex;
use std::time::Duration;

use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::{delegate_dispatch, Connection, Dispatch, Proxy, QueueHandle};
use sctk::reexports::protocols::wp::pointer_gestures::zv1::client::{
    zwp_pointer_gesture_hold_v1::{self, ZwpPointerGestureHoldV1},
    zwp_pointer_gesture_pinch_v1::{self, ZwpPointerGesturePinchV1},
    zwp_pointer_gestures_v1::ZwpPointerGesturesV1,
};

use sctk::compositor::SurfaceData;
use sctk::globals::GlobalData;

use crate::event::{TouchPhase, WindowEvent};
use crate::platform_impl::wayland::state::WinitState;
use crate::platform_impl::wayland::{self, DeviceId, WindowId};

/// The pointer gestures manager, bound only at version 3, which has the hold.
pub struct PointerGesturesState {
    manager: ZwpPointerGesturesV1,
}

impl PointerGesturesState {
    pub fn new(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WinitState>,
    ) -> Result<Self, BindError> {
        let manager = globals.bind(queue_handle, 3..=3, GlobalData)?;
        Ok(Self { manager })
    }
}

impl Deref for PointerGesturesState {
    type Target = ZwpPointerGesturesV1;

    fn deref(&self) -> &Self::Target {
        &self.manager
    }
}

/// The window a hold began over, for its end, which names no surface.
#[derive(Debug, Default)]
pub struct HoldData {
    window: Mutex<Option<WindowId>>,
}

impl Dispatch<ZwpPointerGesturesV1, GlobalData, WinitState> for PointerGesturesState {
    fn event(
        _state: &mut WinitState,
        _proxy: &ZwpPointerGesturesV1,
        _event: <ZwpPointerGesturesV1 as Proxy>::Event,
        _data: &GlobalData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
    }
}

impl Dispatch<ZwpPointerGestureHoldV1, HoldData, WinitState> for PointerGesturesState {
    fn event(
        state: &mut WinitState,
        _proxy: &ZwpPointerGestureHoldV1,
        event: <ZwpPointerGestureHoldV1 as Proxy>::Event,
        data: &HoldData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
        let (window_id, phase, time) = match event {
            zwp_pointer_gesture_hold_v1::Event::Begin { time, surface, .. } => {
                let parent = surface
                    .data::<SurfaceData>()
                    .and_then(|data| data.parent_surface().cloned())
                    .unwrap_or(surface);
                let window_id = wayland::make_wid(&parent);
                *data.window.lock().unwrap() = Some(window_id);
                (window_id, TouchPhase::Started, time)
            },
            zwp_pointer_gesture_hold_v1::Event::End { time, cancelled, .. } => {
                let Some(window_id) = data.window.lock().unwrap().take() else { return };
                let phase = if cancelled != 0 { TouchPhase::Cancelled } else { TouchPhase::Ended };
                (window_id, phase, time)
            },
            _ => return,
        };
        let device_id = crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(DeviceId));
        let time = Some(Duration::from_millis(u64::from(time)));
        state
            .events_sink
            .push_window_event(WindowEvent::HoldGesture { device_id, phase, time }, window_id);
    }
}

delegate_dispatch!(WinitState: [ZwpPointerGesturesV1: GlobalData] => PointerGesturesState);
delegate_dispatch!(WinitState: [ZwpPointerGestureHoldV1: HoldData] => PointerGesturesState);
delegate_dispatch!(WinitState: [ZwpPointerGesturePinchV1: PinchData] => PointerGesturesState);

/// The window a pinch began over, for its updates and end, which name no surface, and the scale
/// its last update reached.
#[derive(Debug, Default)]
pub struct PinchData {
    window: Mutex<Option<WindowId>>,
    scale: Mutex<f64>,
}

impl Dispatch<ZwpPointerGesturePinchV1, PinchData, WinitState> for PointerGesturesState {
    fn event(
        state: &mut WinitState,
        _proxy: &ZwpPointerGesturePinchV1,
        event: <ZwpPointerGesturePinchV1 as Proxy>::Event,
        data: &PinchData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
        // The compositor gives the scale since the pinch began; macOS, and so winit, gives each
        // step's own, such that the product of (1 + delta) is that scale.
        let (window_id, phase, delta) = match event {
            zwp_pointer_gesture_pinch_v1::Event::Begin { surface, .. } => {
                let parent = surface
                    .data::<SurfaceData>()
                    .and_then(|data| data.parent_surface().cloned())
                    .unwrap_or(surface);
                let window_id = wayland::make_wid(&parent);
                *data.window.lock().unwrap() = Some(window_id);
                *data.scale.lock().unwrap() = 1.0;
                (window_id, TouchPhase::Started, 0.0)
            },
            zwp_pointer_gesture_pinch_v1::Event::Update { scale, .. } => {
                let Some(window_id) = *data.window.lock().unwrap() else { return };
                let mut last = data.scale.lock().unwrap();
                let delta = if *last > 0.0 && scale > 0.0 { scale / *last - 1.0 } else { 0.0 };
                *last = scale;
                (window_id, TouchPhase::Moved, delta)
            },
            zwp_pointer_gesture_pinch_v1::Event::End { cancelled, .. } => {
                let Some(window_id) = data.window.lock().unwrap().take() else { return };
                let phase = if cancelled != 0 { TouchPhase::Cancelled } else { TouchPhase::Ended };
                (window_id, phase, 0.0)
            },
            _ => return,
        };
        let device_id = crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(DeviceId));
        state
            .events_sink
            .push_window_event(WindowEvent::PinchGesture { device_id, delta, phase }, window_id);
    }
}
