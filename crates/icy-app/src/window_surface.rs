//! Native swapchain and window alpha presentation for the pinned JSNIR renderer.

use std::sync::Arc;

use icy_core::platform::{WindowBackgroundBlur, has_wayland_surface};
use jsnir_gpu::{
    Result,
    renderer::{Draw, Renderer},
};
use winit::{event_loop::OwnedDisplayHandle, window::Window};

pub struct Surface {
    pub window: Arc<Window>,
    pub target: wgpu::Surface<'static>,
    pub renderer: Renderer,
    adapter: wgpu::Adapter,
    config: wgpu::SurfaceConfiguration,
    view_format: wgpu::TextureFormat,
    alpha_mode: Option<wgpu::CompositeAlphaMode>,
    alpha: Option<AlphaPresentation>,
    blur: Option<WindowBackgroundBlur>,
}

impl Surface {
    pub fn open(
        display: &OwnedDisplayHandle,
        window: Arc<Window>,
    ) -> Result<(wgpu::Instance, Self)> {
        // Match JSNIR's backend selection and only initialize GL if the primary backend fails.
        let instance = |backends| {
            let mut descriptor = wgpu::InstanceDescriptor::new_with_display_handle_from_env(
                Box::new(display.clone()),
            );
            if std::env::var_os("WGPU_BACKEND").is_none() {
                descriptor.backends = backends;
            }
            wgpu::Instance::new(descriptor)
        };
        let primary = instance(wgpu::Backends::PRIMARY);
        match Self::new(&primary, window.clone()) {
            Ok(surface) => Ok((primary, surface)),
            Err(_) => {
                let gl = instance(wgpu::Backends::GL);
                let surface = Self::new(&gl, window)?;
                Ok((gl, surface))
            }
        }
    }

    pub fn new(instance: &wgpu::Instance, window: Arc<Window>) -> Result<Self> {
        let target = instance.create_surface(window.clone())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&target),
            ..Default::default()
        }))?;
        let size = window.inner_size();
        let mut config = target
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("wgpu found no surface format for the window")?;
        let view_format = config.format.remove_srgb_suffix();
        config.view_formats = vec![view_format];
        let capabilities = target.get_capabilities(&adapter);
        if capabilities.usages.contains(wgpu::TextureUsages::COPY_SRC) {
            config.usage |= wgpu::TextureUsages::COPY_SRC;
        }
        let alpha_mode = if has_wayland_surface(window.as_ref())
            && matches!(
                view_format,
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
            ) {
            [
                wgpu::CompositeAlphaMode::PreMultiplied,
                wgpu::CompositeAlphaMode::PostMultiplied,
            ]
            .into_iter()
            .find(|mode| capabilities.alpha_modes.contains(mode))
        } else {
            None
        };
        let renderer = Renderer::new(&adapter, view_format)?;
        target.configure(&renderer.device, &config);
        Ok(Self {
            window,
            target,
            renderer,
            adapter,
            config,
            view_format,
            alpha_mode,
            alpha: None,
            blur: None,
        })
    }

    pub fn supports_transparency(&self) -> bool {
        self.alpha_mode.is_some()
    }

    /// Returns whether JSNIR's GPU resources were replaced and its view needs resetting.
    pub fn set_translucent(&mut self, translucent: bool) -> Result<bool> {
        let Some(alpha_mode) = self.alpha_mode else {
            return Ok(false);
        };
        if translucent == self.alpha.is_some() {
            return Ok(false);
        }
        let format = if translucent {
            AlphaPresentation::FORMAT
        } else {
            self.view_format
        };
        let renderer = Renderer::new(&self.adapter, format)?;
        self.config.alpha_mode = if translucent {
            alpha_mode
        } else {
            wgpu::CompositeAlphaMode::Auto
        };
        self.alpha = translucent
            .then(|| AlphaPresentation::new(&renderer.device, self.view_format, alpha_mode));
        self.window.set_transparent(translucent);
        if translucent && self.blur.is_none() {
            self.blur = WindowBackgroundBlur::attach(self.window.clone());
        }
        if let Some(blur) = &self.blur {
            blur.set_enabled(translucent);
        }
        self.target.configure(&renderer.device, &self.config);
        self.renderer = renderer;
        Ok(true)
    }

    pub fn resize(&mut self) {
        let size = self.window.inner_size();
        if size.width > 0 && size.height > 0 {
            self.config.width = size.width;
            self.config.height = size.height;
            self.target.configure(&self.renderer.device, &self.config);
        }
    }

    pub fn draw(
        &mut self,
        target: &wgpu::Texture,
        pixels: [u32; 2],
        scale: f32,
        mut draws: Vec<Draw>,
    ) {
        if let Some(alpha) = &mut self.alpha {
            alpha.draw(&mut self.renderer, target, pixels, scale, &mut draws);
        } else {
            self.renderer.draw(target, pixels, scale, &draws);
        }
    }
}

struct AlphaPresentation {
    pipeline: wgpu::RenderPipeline,
    output_format: wgpu::TextureFormat,
    frame: Option<(wgpu::Texture, wgpu::BindGroup)>,
}

impl AlphaPresentation {
    const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    fn new(
        device: &wgpu::Device,
        output_format: wgpu::TextureFormat,
        mode: wgpu::CompositeAlphaMode,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("window_alpha.wgsl"));
        let straight_alpha = f64::from(mode == wgpu::CompositeAlphaMode::PostMultiplied);
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("icy window alpha"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                targets: &[Some(output_format.into())],
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &[("straight_alpha", straight_alpha)],
                    ..Default::default()
                },
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            output_format,
            frame: None,
        }
    }

    fn draw(
        &mut self,
        renderer: &mut Renderer,
        target: &wgpu::Texture,
        pixels: [u32; 2],
        scale: f32,
        draws: &mut Vec<Draw>,
    ) {
        let device = &renderer.device;
        if self
            .frame
            .as_ref()
            .is_none_or(|(texture, _)| [texture.width(), texture.height()] != pixels)
        {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("icy window alpha intermediate"),
                size: wgpu::Extent3d {
                    width: pixels[0],
                    height: pixels[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Self::FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &texture.create_view(&Default::default()),
                    ),
                }],
            });
            self.frame = Some((texture, bindings));
        }
        let (texture, bindings) = self.frame.as_ref().unwrap();
        let [width, height] = pixels.map(|side| side as f32 / scale);
        let rect = [0., 0., width, height];
        // JSNIR's root clear is opaque black, while its intermediate surfaces clear to transparent.
        // Add the scene's premultiplied RGBA to that black in a float target, then remove the +1 alpha.
        draws.insert(
            0,
            Draw::PushSurface {
                source: rect,
                dest: rect,
                radius: [0.; 4],
                opacity: 1.,
                clip: rect,
                round_clip: None,
                additive: true,
                cache_key: None,
            },
        );
        draws.push(Draw::PopSurface);
        renderer.draw(texture, pixels, scale, draws);

        let view = target.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.output_format),
            ..Default::default()
        });
        let mut encoder = renderer.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bindings, &[]);
            pass.draw(0..3, 0..1);
        }
        renderer.queue.submit([encoder.finish()]);
    }
}
