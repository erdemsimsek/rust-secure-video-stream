use eframe::egui;
use streaming_capture::{spawn_camera_actor, CameraCommand, CameraEvent, CameraHandle};
use streaming_core::{Frame, PixelFormat};
use std::sync::Arc;
use tokio::sync::Mutex;

fn main() -> Result<(), eframe::Error> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Camera Viewer",
        options,
        Box::new(|_cc| Ok(Box::new(CameraViewerApp::new()))),
    )
}

struct CameraViewerApp {
    // Camera control
    camera_handle: Option<CameraHandle>,
    camera_state: CameraState,

    // Frame display
    current_frame: Arc<Mutex<Option<Frame>>>,
    texture: Option<egui::TextureHandle>,

    // Statistics
    stats: StreamStats,

    // Tokio runtime for async operations
    runtime: tokio::runtime::Runtime,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CameraState {
    Disconnected,
    Connecting,
    Connected,
    Streaming,
    Error,
}

#[derive(Default)]
struct StreamStats {
    frames_received: u64,
    fps: f32,
    last_frame_time: Option<std::time::Instant>,
}

impl CameraViewerApp {
    fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime");

        Self {
            camera_handle: None,
            camera_state: CameraState::Disconnected,
            current_frame: Arc::new(Mutex::new(None)),
            texture: None,
            stats: StreamStats::default(),
            runtime,
        }
    }

    fn connect_camera(&mut self) {
        self.camera_state = CameraState::Connecting;

        match spawn_camera_actor("/dev/video0") {
            Ok((handle, mut events)) => {
                self.camera_handle = Some(handle);
                self.camera_state = CameraState::Connected;

                // Spawn task to receive events
                let current_frame = Arc::clone(&self.current_frame);
                self.runtime.spawn(async move {
                    while let Some(event) = events.recv().await {
                        match event {
                            CameraEvent::FrameCaptured(frame) => {
                                let mut frame_lock = current_frame.lock().await;
                                *frame_lock = Some(frame);
                            }
                            CameraEvent::Error(e) => {
                                eprintln!("Camera error: {}", e);
                            }
                            _ => {}
                        }
                    }
                });
            }
            Err(e) => {
                eprintln!("Failed to connect to camera: {}", e);
                self.camera_state = CameraState::Error;
            }
        }
    }

    fn start_streaming(&mut self) {
        if let Some(handle) = &self.camera_handle {
            // Discover capabilities first
            if let Err(e) = handle.send_command(CameraCommand::DiscoverCapabilities) {
                eprintln!("Failed to discover capabilities: {}", e);
                return;
            }

            // Wait a bit for capabilities (in a real app, you'd handle this properly)
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Configure camera (using first available format/resolution)
            // In a real app, you'd wait for CapabilitiesDiscovered event
            if let Err(e) = handle.send_command(CameraCommand::SetConfiguration {
                width: 640,
                height: 480,
                fps: 30,
                format: PixelFormat::MJPG, // Try MJPEG first, fallback to YUYV if needed
            }) {
                eprintln!("Failed to configure camera: {}", e);
                return;
            }

            std::thread::sleep(std::time::Duration::from_millis(50));

            // Start streaming
            if let Err(e) = handle.send_command(CameraCommand::StartStreaming) {
                eprintln!("Failed to start streaming: {}", e);
                return;
            }

            self.camera_state = CameraState::Streaming;
        }
    }

    fn stop_streaming(&mut self) {
        if let Some(handle) = &self.camera_handle {
            if let Err(e) = handle.send_command(CameraCommand::StopStreaming) {
                eprintln!("Failed to stop streaming: {}", e);
            }
            self.camera_state = CameraState::Connected;
        }
    }

    fn disconnect_camera(&mut self) {
        if let Some(handle) = self.camera_handle.take() {
            if let Err(e) = handle.shutdown() {
                eprintln!("Failed to shutdown camera: {}", e);
            }
        }
        self.camera_state = CameraState::Disconnected;
        self.texture = None;
    }

    fn update_texture(&mut self, ctx: &egui::Context) {
        // Try to get the latest frame without blocking
        if let Ok(mut frame_lock) = self.current_frame.try_lock() {
            if let Some(frame) = frame_lock.take() {
                // Update stats
                self.stats.frames_received += 1;
                if let Some(last_time) = self.stats.last_frame_time {
                    let elapsed = last_time.elapsed().as_secs_f32();
                    if elapsed > 0.0 {
                        self.stats.fps = 0.9 * self.stats.fps + 0.1 * (1.0 / elapsed);
                    }
                }
                self.stats.last_frame_time = Some(std::time::Instant::now());

                // Convert frame to RGB
                let rgb_data = convert_frame_to_rgb(&frame);

                // Create egui ColorImage
                let color_image = egui::ColorImage::from_rgb(
                    [frame.width as usize, frame.height as usize],
                    &rgb_data,
                );

                // Create or update texture
                if let Some(texture) = &mut self.texture {
                    texture.set(color_image, egui::TextureOptions::LINEAR);
                } else {
                    self.texture = Some(ctx.load_texture(
                        "camera_frame",
                        color_image,
                        egui::TextureOptions::LINEAR,
                    ));
                }
            }
        }
    }
}

impl eframe::App for CameraViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update texture if streaming
        if self.camera_state == CameraState::Streaming {
            self.update_texture(ctx);
            ctx.request_repaint(); // Keep refreshing for video
        }

        // Top panel - controls
        egui::TopBottomPanel::top("controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Camera Viewer");
                ui.separator();

                match self.camera_state {
                    CameraState::Disconnected => {
                        if ui.button("Connect").clicked() {
                            self.connect_camera();
                        }
                    }
                    CameraState::Connecting => {
                        ui.spinner();
                        ui.label("Connecting...");
                    }
                    CameraState::Connected => {
                        if ui.button("Start").clicked() {
                            self.start_streaming();
                        }
                        ui.separator();
                        if ui.button("Disconnect").clicked() {
                            self.disconnect_camera();
                        }
                    }
                    CameraState::Streaming => {
                        ui.colored_label(egui::Color32::GREEN, "● Streaming");
                        if ui.button("Stop").clicked() {
                            self.stop_streaming();
                        }
                        ui.separator();
                        if ui.button("Disconnect").clicked() {
                            self.stop_streaming();
                            self.disconnect_camera();
                        }
                    }
                    CameraState::Error => {
                        ui.colored_label(egui::Color32::RED, "⚠ Error");
                        if ui.button("Retry").clicked() {
                            self.connect_camera();
                        }
                    }
                }
            });
        });

        // Central panel - video display
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                if let Some(texture) = &self.texture {
                    // Display the video frame
                    ui.image(egui::load::SizedTexture::new(
                        texture.id(),
                        texture.size_vec2(),
                    ));
                } else {
                    ui.heading("No video stream");
                    ui.label("Click 'Connect' to start");
                }

                ui.separator();

                // Statistics
                ui.horizontal(|ui| {
                    ui.label(format!("FPS: {:.1}", self.stats.fps));
                    ui.separator();
                    ui.label(format!("Frames: {}", self.stats.frames_received));
                });
            });
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Clean shutdown
        self.disconnect_camera();
    }
}

/// Convert frame data to RGB format for egui display
fn convert_frame_to_rgb(frame: &Frame) -> Vec<u8> {
    match frame.format {
        PixelFormat::MJPG => {
            // Decode MJPEG using the image crate
            match image::load_from_memory_with_format(&frame.data, image::ImageFormat::Jpeg) {
                Ok(img) => {
                    let rgb_img = img.to_rgb8();
                    rgb_img.into_raw()
                }
                Err(e) => {
                    eprintln!("Failed to decode MJPEG: {}", e);
                    // Return black image as fallback
                    vec![0u8; (frame.width * frame.height * 3) as usize]
                }
            }
        }
        PixelFormat::YUYV => {
            // Convert YUYV to RGB
            yuyv_to_rgb(&frame.data, frame.width as usize, frame.height as usize)
        }
        _ => {
            eprintln!("Unsupported pixel format: {:?}", frame.format);
            // Return black image as fallback
            vec![0u8; (frame.width * frame.height * 3) as usize]
        }
    }
}

/// Convert YUYV (YUV 4:2:2) to RGB
fn yuyv_to_rgb(yuyv_data: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(width * height * 3);

    for y in 0..height {
        for x in (0..width).step_by(2) {
            let yuyv_index = (y * width * 2) + (x * 2);

            if yuyv_index + 3 >= yuyv_data.len() {
                break;
            }

            let y0 = yuyv_data[yuyv_index] as f32;
            let u = yuyv_data[yuyv_index + 1] as f32;
            let y1 = yuyv_data[yuyv_index + 2] as f32;
            let v = yuyv_data[yuyv_index + 3] as f32;

            // Convert first pixel (Y0, U, V)
            let (r0, g0, b0) = yuv_to_rgb_pixel(y0, u, v);
            rgb.push(r0);
            rgb.push(g0);
            rgb.push(b0);

            // Convert second pixel (Y1, U, V)
            let (r1, g1, b1) = yuv_to_rgb_pixel(y1, u, v);
            rgb.push(r1);
            rgb.push(g1);
            rgb.push(b1);
        }
    }

    rgb
}

/// Convert a single YUV pixel to RGB
fn yuv_to_rgb_pixel(y: f32, u: f32, v: f32) -> (u8, u8, u8) {
    // YUV to RGB conversion formula
    let c = y - 16.0;
    let d = u - 128.0;
    let e = v - 128.0;

    let r = (1.164 * c + 1.596 * e).clamp(0.0, 255.0) as u8;
    let g = (1.164 * c - 0.391 * d - 0.813 * e).clamp(0.0, 255.0) as u8;
    let b = (1.164 * c + 2.018 * d).clamp(0.0, 255.0) as u8;

    (r, g, b)
}
