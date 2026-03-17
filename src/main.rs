use std::sync::Arc;
use std::time::Duration;

use egui_winit::EventResponse;
use gameboy_core::util::Controls;
use instant::Instant;
use pixels::winit::keyboard::{Key, NamedKey};
use tracing::error;

use pixels::winit::{
    application::ApplicationHandler,
    error::EventLoopError,
    event::{ElementState, KeyEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
    {event::WindowEvent, event_loop::ControlFlow},
};

use pixels::wgpu;

use crate::debugger::Debugger;
use gameboy_core::{Cycles, GameboyColor};

mod debugger;

fn main() -> Result<(), EventLoopError> {
    let args = std::env::args().collect::<Vec<_>>();

    let rom = args
        .get(1)
        .map(|fname| {
            std::fs::read(fname)
                .unwrap_or_else(|e| panic!("Could not open ROM file with error: {e}"))
        })
        .unwrap_or_else(|| panic!("Please provide path to ROM file as an argument."));

    let emulator = GameboyColor::new(rom);

    // let printlog = args
    //     .get(2)
    //     .map(|fname| fname.trim() == "log")
    //     .unwrap_or(false);

    // if printlog {
    //     emulator.cpu.registers = gameboy_core::cpu::Registers::new_single(
    //         0x00, 0x13, 0x00, 0xD8, 0x01, 0x4D, 0x01, 0xB0, 0xFFFE, 0x0100,
    //     );
    // } else {
        tracing_subscriber::fmt()
            // .with_max_level(tracing::Level::TRACE)
            .with_target(false)
            // .with_thread_names(true)
            // .with_thread_ids(true)
            .init();
    // }

    let mut app = Application {
        emulator: Some(Emulator {
            gameboy: emulator,
            debugger: Some(Debugger::new()),
            // debugger: None,
        }),
        graphics: None,
    };

    // if printlog {
    //     // SET LY TO 0x90!
    //     let emulator = app.emulator.as_mut().unwrap();
    //     if let Some(debugger) = emulator.debugger.as_mut() {
    //         debugger.set_running();
    //     }
    //     loop {
    //         match emulator.update::<true>(None) {
    //             ApplicationUpdate::Exit => break,
    //             _ => (),
    //         }
    //     }
    //     Ok(())
    // } else {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.run_app(&mut app)
    // }
}

pub struct Application {
    emulator: Option<Emulator>,
    graphics: Option<GraphicsState>,
}

struct Emulator {
    gameboy: GameboyColor,
    debugger: Option<Debugger>,
}

pub enum ApplicationUpdate {
    Continue,
    Render,
    WaitUntil(Instant),
    Exit,
}

impl Emulator {
    pub fn update<const LOG: bool>(&mut self, next: Option<&mut Instant>) -> ApplicationUpdate {
        let mut update = ApplicationUpdate::Continue;

        let max_cycles = next.map(|next| {
            let new = Instant::now();
            let between = new - *next;
            *next += GraphicsState::CYCLE_TIME;
            update = ApplicationUpdate::WaitUntil(*next);
            (GraphicsState::CLOCK_SPEED as f64
                * between.as_secs_f64()
                * self.debugger.as_mut().map(|d| d.speed()).unwrap_or(1.0)) as usize
        });

        let mut cycles = Cycles::new(0);

        while self
            .debugger
            .as_mut()
            .map(|d| d.should_step(&self.gameboy))
            .unwrap_or(true)
            && max_cycles.map(|max| cycles <= max).unwrap_or(true)
        {
            if LOG {
                if let Some(debugger) = self.debugger.as_mut() {
                    debugger.log(&self.gameboy);
                }
            }
            match self.gameboy.cycle() {
                Ok(result) => {
                    cycles += result.cpu.cycles;
                    if let Some(debugger) = self.debugger.as_mut() {
                        debugger.on_cycle(result.cpu);
                    }
                    if result.render {
                        return ApplicationUpdate::Render;
                    }
                }
                Err(err) => {
                    error!("Error during frame: {err}");
                    if let Some(debugger) = self.debugger.as_mut() {
                        debugger.error(err);
                    } else {
                        return ApplicationUpdate::Exit;
                    }
                }
            }
        }
        update
    }
}

struct GraphicsState {
    window: Arc<Window>,
    pixels: pixels::Pixels<'static>,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    egui_shapes: Vec<egui::epaint::ClippedPrimitive>,
    next: Instant,
}

impl GraphicsState {
    const CYCLE_TIME: Duration = Duration::new(0, 16600000);
    const CLOCK_SPEED: usize = 4194304;
    // const FRAME_RATE: usize = 60;
    // const CYCLES_PER_FRAME: usize = Self::CLOCK_SPEED / Self::FRAME_RATE;
}

impl ApplicationHandler for Application {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes().with_title(
            self.emulator
                .as_ref()
                .map(|e| format!("Gameboy Emulator - {}", e.gameboy.title()))
                .unwrap_or_else(|| "Unknown".to_string()),
        );
        match event_loop.create_window(window_attributes) {
            Ok(window) => {
                let window = Arc::new(window);
                let mut pixels =
                    pixels::Pixels::new(160, 144, pixels::SurfaceTexture::new(&window))
                        .expect("Could not create window!");

                pixels.clear_color(wgpu::Color::GREEN);

                let egui_state = egui_winit::State::new(
                    Default::default(),
                    egui::ViewportId::ROOT,
                    &window,
                    Some(window.scale_factor() as f32),
                    None,
                    None,
                );
                let egui_renderer = egui_wgpu::Renderer::new(
                    pixels.device(),
                    pixels.render_texture_format(),
                    Default::default(),
                );

                self.graphics = Some(GraphicsState {
                    window: window.clone(),
                    pixels,
                    egui_state,
                    egui_renderer,
                    egui_shapes: Vec::new(),
                    next: Instant::now(),
                });
            }
            Err(e) => panic!("Could not create window: {e}"),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(graphics) = self.graphics.as_mut() {
            let EventResponse { repaint, .. } = graphics
                .egui_state
                .on_window_event(&graphics.window, &event);
            if repaint {
                graphics.window.request_redraw();
            }
        }
        match event {
            WindowEvent::CloseRequested => {
                if let Some(window) = self.graphics.as_ref().map(|g| &g.window) {
                    if window.id() == window_id {
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::Resized(new_size) => {
                if let Some(graphics) = self.graphics.as_mut() {
                    if let Err(e) = graphics.pixels.resize(new_size) {
                        error!("Failed to resize pixels: {}", e);
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: key,
                        state,
                        ..
                    },
                ..
            } => {
                // tracing::info!("Key event: {key:?} is {:?}\r", state);
                if let Some(gameboy) = self.emulator.as_mut().map(|e| &mut e.gameboy) {
                    match key.as_ref() {
                        Key::Named(NamedKey::ArrowUp) => gameboy.update_input((Controls::Up, state != ElementState::Pressed)),
                        Key::Named(NamedKey::ArrowDown) => gameboy.update_input((Controls::Down, state != ElementState::Pressed)),
                        Key::Named(NamedKey::ArrowLeft) => gameboy.update_input((Controls::Left, state != ElementState::Pressed)),
                        Key::Named(NamedKey::ArrowRight) => gameboy.update_input((Controls::Right, state != ElementState::Pressed)),
                        Key::Character("a") => gameboy.update_input((Controls::Start, state != ElementState::Pressed)),
                        Key::Character("s") => gameboy.update_input((Controls::Select, state != ElementState::Pressed)),
                        Key::Character("z") => gameboy.update_input((Controls::A, state != ElementState::Pressed)),
                        Key::Character("x") => gameboy.update_input((Controls::B, state != ElementState::Pressed)),
                        _ => (),
                    }
                }
            },
            WindowEvent::RedrawRequested => {
                if let Some(emulator) = self.emulator.as_mut() {
                    match emulator.update::<false>(self.graphics.as_mut().map(|g| &mut g.next)) {
                        ApplicationUpdate::Continue => (),
                        ApplicationUpdate::Exit => event_loop.exit(),
                        ApplicationUpdate::WaitUntil(instant) => {
                            event_loop.set_control_flow(ControlFlow::WaitUntil(instant))
                        }
                        ApplicationUpdate::Render => {
                            if let Some(graphics) = self.graphics.as_mut() {
                                emulator.gameboy.frame_to_rgba(graphics.pixels.frame_mut());
                            }
                        }
                    }
                    if let Some(graphics) = self.graphics.as_mut() {
                        if let Some(debugger) = emulator.debugger.as_mut() {
                            let raw_input = graphics.egui_state.take_egui_input(&graphics.window);

                            let egui_output =
                                graphics.egui_state.egui_ctx().run(raw_input, |ctx| {
                                    debugger.window(
                                        &mut emulator.gameboy,
                                        ctx,
                                        graphics.window.inner_size(),
                                    );
                                });

                            graphics.egui_state.handle_platform_output(
                                &graphics.window,
                                egui_output.platform_output,
                            );

                            for (id, image_delta) in egui_output.textures_delta.set {
                                graphics.egui_renderer.update_texture(
                                    graphics.pixels.device(),
                                    graphics.pixels.queue(),
                                    id,
                                    &image_delta,
                                );
                            }

                            for id in egui_output.textures_delta.free {
                                graphics.egui_renderer.free_texture(&id);
                            }

                            let pixels_per_point =
                                graphics.egui_state.egui_ctx().pixels_per_point();
                            graphics.egui_shapes = graphics
                                .egui_state
                                .egui_ctx()
                                .tessellate(egui_output.shapes, pixels_per_point);
                        }

                        let window = graphics.window.as_ref();
                        window.pre_present_notify();

                        graphics
                            .pixels
                            .render_with(|encoder, output, ctx| {
                                ctx.scaling_renderer.render(encoder, output);

                                if emulator.debugger.is_none() {
                                    return Ok(());
                                }

                                let screen_descriptor = egui_wgpu::ScreenDescriptor {
                                    pixels_per_point: window.scale_factor() as f32,
                                    size_in_pixels: window.inner_size().into(),
                                };

                                let cmd_buffers = graphics.egui_renderer.update_buffers(
                                    &ctx.device,
                                    &ctx.queue,
                                    encoder,
                                    &graphics.egui_shapes,
                                    &screen_descriptor,
                                );

                                ctx.queue.submit(cmd_buffers);

                                let mut egui_pass = encoder
                                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                                        label: Some("egui"),
                                        color_attachments: &[Some(
                                            wgpu::RenderPassColorAttachment {
                                                view: &output,
                                                resolve_target: None,
                                                ops: wgpu::Operations {
                                                    load: wgpu::LoadOp::Load,
                                                    store: wgpu::StoreOp::Store,
                                                },
                                                depth_slice: None,
                                            },
                                        )],
                                        depth_stencil_attachment: None,
                                        timestamp_writes: None,
                                        occlusion_query_set: None,
                                    })
                                    .forget_lifetime();

                                graphics.egui_renderer.render(
                                    &mut egui_pass,
                                    &graphics.egui_shapes,
                                    &screen_descriptor,
                                );

                                Ok(())
                            })
                            .unwrap();
                    }
                }
            }
            _ => (),
        }
    }
}
