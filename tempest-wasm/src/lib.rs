use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    log("🌪️ Tempest WASM Engine Online");
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    _config: wgpu::SurfaceConfiguration,
    _canvas: web_sys::HtmlCanvasElement,
    render_pipeline: wgpu::RenderPipeline,
    time_buffer: wgpu::Buffer,
    time_bind_group: wgpu::BindGroup,
    start_time: f64,
}

impl State {
    async fn new(canvas: web_sys::HtmlCanvasElement) -> Result<Self, String> {
        #[cfg(target_arch = "wasm32")]
        {
            let width = canvas.width();
            let height = canvas.height();

            let instance = wgpu::Instance::default();

            let surface = instance
                .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
                .map_err(|e| format!("Failed to create wgpu surface: {:?}", e))?;

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .map_err(|e| format!("Failed to find a suitable wgpu adapter: {:?}", e))?;

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("Tempest Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                    memory_hints: wgpu::MemoryHints::default(),
                    experimental_features: wgpu::ExperimentalFeatures::default(),
                    trace: wgpu::Trace::Off,
                })
                .await
                .map_err(|e| format!("Failed to create wgpu device: {:?}", e))?;

            let surface_caps = surface.get_capabilities(&adapter);
            if surface_caps.formats.is_empty() {
                return Err("No supported surface formats found".to_string());
            }
            let surface_format = surface_caps
                .formats
                .iter()
                .copied()
                .find(|f| f.is_srgb())
                .unwrap_or(surface_caps.formats[0]);

            let present_mode = surface_caps
                .present_modes
                .first()
                .copied()
                .ok_or_else(|| "No supported present modes found".to_string())?;

            let alpha_mode = surface_caps
                .alpha_modes
                .first()
                .copied()
                .ok_or_else(|| "No supported alpha modes found".to_string())?;

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width,
                height,
                present_mode,
                alpha_mode,
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            // Time uniform buffer & bind group setup
            let time_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Time Buffer"),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let time_bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Time Bind Group Layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

            let time_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Time Bind Group"),
                layout: &time_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: time_buffer.as_entire_binding(),
                }],
            });

            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Vortex Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
            });

            let render_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Render Pipeline Layout"),
                    bind_group_layouts: &[Some(&time_bind_group_layout)],
                    immediate_size: 0,
                });

            let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            });

            let start_time = web_sys::window()
                .and_then(|w| w.performance())
                .map(|p| p.now())
                .unwrap_or(0.0);

            Ok(Self {
                surface,
                device,
                queue,
                _config: config,
                _canvas: canvas,
                render_pipeline,
                time_buffer,
                time_bind_group,
                start_time,
            })
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = canvas;
            Err("WASM Dashboard state only available on wasm32 target".to_string())
        }
    }

    pub fn resize(&mut self, new_width: u32, new_height: u32) {
        if new_width > 0 && new_height > 0 {
            self._config.width = new_width;
            self._config.height = new_height;
            self.surface.configure(&self.device, &self._config);
        }
    }

    pub fn render(&mut self) -> Result<(), String> {
        // Update the elapsed time uniform
        let timestamp = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);
        let elapsed = ((timestamp - self.start_time) / 1000.0) as f32;
        let mut time_bytes = [0u8; 16];
        time_bytes[0..4].copy_from_slice(&elapsed.to_ne_bytes());
        self.queue.write_buffer(&self.time_buffer, 0, &time_bytes);

        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => return Err("Failed to acquire next surface texture".to_string()),
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.time_bind_group, &[]);
            // Draw full-screen triangle (3 vertices)
            render_pass.draw(0..3, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

#[wasm_bindgen]
pub struct Dashboard {
    state: Rc<RefCell<State>>,
    _loop_closure: Closure<dyn FnMut(f64)>,
}

#[wasm_bindgen]
impl Dashboard {
    pub fn resize(&mut self, width: u32, height: u32) {
        if let Ok(mut state) = self.state.try_borrow_mut() {
            state.resize(width, height);
            let _ = state.render();
        }
    }

    pub fn render(&mut self) {
        if let Ok(mut state) = self.state.try_borrow_mut() {
            let _ = state.render();
        }
    }
}
fn start_animation_loop(state: Rc<RefCell<State>>) -> Closure<dyn FnMut(f64)> {
    let f: Rc<RefCell<Option<js_sys::Function>>> = Rc::new(RefCell::new(None));
    let g = f.clone();

    let closure = Closure::wrap(Box::new(move |_timestamp: f64| {
        if let Ok(mut state) = state.try_borrow_mut() {
            let _ = state.render();
        }

        if let Some(window) = web_sys::window() {
            if let Some(ref closure) = *f.borrow() {
                let _ = window.request_animation_frame(closure);
            }
        }
    }) as Box<dyn FnMut(f64)>);

    *g.borrow_mut() = Some(closure.as_ref().unchecked_ref::<js_sys::Function>().clone());

    if let Some(window) = web_sys::window() {
        let _ = window.request_animation_frame(g.borrow().as_ref().unwrap());
    }

    closure
}

#[wasm_bindgen]
pub async fn initialize_dashboard(canvas_id: &str) -> Result<Dashboard, JsValue> {
    log(&format!("📍 Initializing Vortex on canvas: {}", canvas_id));

    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no global `window` exists"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("should have a document on window"))?;
    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or_else(|| JsValue::from_str(&format!("should have canvas with id {}", canvas_id)))?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| JsValue::from_str("element is not a canvas"))?;

    let mut width = canvas.client_width() as u32;
    let mut height = canvas.client_height() as u32;
    if width == 0 {
        width = 300;
    }
    if height == 0 {
        height = 300;
    }
    canvas.set_width(width);
    canvas.set_height(height);

    log(&format!("🛠️ WGPU State init: {}x{}", width, height));
    let state = State::new(canvas)
        .await
        .map_err(|e| JsValue::from_str(&e))?;
    let state_rc = Rc::new(RefCell::new(state));

    let loop_closure = start_animation_loop(state_rc.clone());

    log("✅ Dashboard instance ready");
    Ok(Dashboard {
        state: state_rc,
        _loop_closure: loop_closure,
    })
}
