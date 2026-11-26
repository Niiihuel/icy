//! Requests compositor blur for an existing Wayland toplevel.
use std::{any::Any, sync::Arc};

use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use wayland_backend::sys::client::{Backend, ObjectId};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, delegate_noop,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{
        wl_compositor::WlCompositor, wl_region::WlRegion, wl_registry::WlRegistry,
        wl_surface::WlSurface,
    },
};
use wayland_protocols::ext::background_effect::v1::client::{
    ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1,
    ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1,
};

struct State;
impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
delegate_noop!(State: ignore WlCompositor);
delegate_noop!(State: ignore WlRegion);
delegate_noop!(State: ignore ExtBackgroundEffectManagerV1);
delegate_noop!(State: ignore ExtBackgroundEffectSurfaceV1);

pub struct WindowBackgroundBlur {
    compositor: WlCompositor,
    effect: ExtBackgroundEffectSurfaceV1,
    queue: EventQueue<State>,
    _window: Arc<dyn Any>,
}

impl WindowBackgroundBlur {
    pub fn attach<W: HasDisplayHandle + HasWindowHandle + 'static>(window: Arc<W>) -> Option<Self> {
        let RawDisplayHandle::Wayland(display) = window.display_handle().ok()?.as_raw() else {
            return None;
        };
        let RawWindowHandle::Wayland(surface) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        // SAFETY: The handles come from the same window and the effect retains that window.
        // The foreign backend neither owns nor closes its display.
        let backend = unsafe { Backend::from_foreign_display(display.display.as_ptr().cast()) };
        let id =
            unsafe { ObjectId::from_ptr(WlSurface::interface(), surface.surface.as_ptr().cast()) }
                .ok()?;
        let connection = Connection::from_backend(backend);
        let surface = WlSurface::from_id(&connection, id).ok()?;
        let (globals, queue) = registry_queue_init::<State>(&connection).ok()?;
        let handle = queue.handle();
        let compositor: WlCompositor = globals.bind(&handle, 1..=1, ()).ok()?;
        let manager: ExtBackgroundEffectManagerV1 = globals.bind(&handle, 1..=1, ()).ok()?;
        let effect = manager.get_background_effect(&surface, &handle, ());
        manager.destroy();
        Some(Self {
            compositor,
            effect,
            queue,
            _window: window,
        })
    }

    /// The new region takes effect on the next buffer presentation.
    pub fn set_enabled(&self, enabled: bool) {
        if !enabled {
            self.effect.set_blur_region(None);
            return;
        }
        let region = self.compositor.create_region(&self.queue.handle(), ());
        // The compositor clips this to the surface, including after a resize.
        region.add(0, 0, i32::MAX, i32::MAX);
        self.effect.set_blur_region(Some(&region));
        region.destroy();
    }
}

impl Drop for WindowBackgroundBlur {
    fn drop(&mut self) {
        self.effect.destroy();
    }
}
