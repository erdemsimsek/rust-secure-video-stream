//! Camera capture library for video streaming.
//!
//! Provides camera discovery, capability querying, and frame capture functionality
//! using V4L2 on Linux.

use std::path::Path;
use std::time::SystemTime;
use rscam::{Camera};
use streaming_core::{CameraCapabilities, FormatCapability, PixelFormat, Resolution, Frame};
use thiserror::Error;
use tokio::sync::mpsc;

#[derive(Debug, Error)]
pub enum CameraError {
    #[error("Interface not found")]
    InterfaceNotFound,

    #[error("Capabilities not discovered. Call discover_capabilities() first.")]
    CapabilitiesNotDiscovered,

    #[error("Unsupported format by camera: {0:?}")]
    UnsupportedFormat(PixelFormat),

    #[error("Resolution {0}x{1} not supported for format {2:?}")]
    UnsupportedResolution(u32, u32, PixelFormat),

    #[error("Camera not configured")]
    NotConfigured,

    #[error("Already streaming")]
    AlreadyStreaming,

    #[error("Not streaming")]
    NotStreaming,

    #[error("IO error: {0}")]
    IoError(String),
}

/// Commands that can be sent to the camera actor to control its behavior.
///
/// These commands are sent through the `CameraHandle` to the actor thread,
/// which processes them and sends back corresponding events.
pub enum CameraCommand {
    /// Change the camera device (e.g., from /dev/video0 to /dev/video1)
    SetInterface(String),

    /// Discover the formats and resolutions supported by the camera
    DiscoverCapabilities,

    /// Query the current camera configuration
    GetConfiguration,

    /// Set camera format, resolution, and frame rate
    SetConfiguration{ width: u32, height: u32, fps: u32, format: PixelFormat},

    /// Start capturing frames continuously
    StartStreaming,

    /// Stop capturing frames
    StopStreaming,

    /// Shutdown the actor thread gracefully
    Shutdown
}

/// Events sent by the camera actor in response to commands or during streaming.
///
/// These events are received through the event channel returned by [`spawn_camera_actor`].
#[derive(Debug)]
pub enum CameraEvent {
    /// Camera device was successfully changed
    InterfaceChanged,

    /// Camera capabilities have been discovered
    CapabilitiesDiscovered(CameraCapabilities),

    /// Current camera configuration retrieved
    ConfigurationRetrieved(CaptureConfig),

    /// Camera successfully configured with new settings
    Configured,

    /// A frame was captured (continuous during streaming)
    FrameCaptured(Frame),

    /// Frame capture has started
    StreamingStarted,

    /// Frame capture has stopped
    StreamingStopped,

    /// Actor thread has shut down
    ShutdownComplete,

    /// An error occurred while processing a command
    Error(CameraError),
}

#[derive(PartialEq, Debug)]
enum CameraState {
    Idle,
    Configured,
    Streaming,
}

/// Camera configuration specifying format, resolution, and frame rate.
#[derive(Clone, Debug)]
pub struct CaptureConfig {
    /// Pixel format (e.g., MJPEG, YUYV)
    pub format: PixelFormat,

    /// Frame resolution (width and height)
    pub resolution: Resolution,

    /// Frames per second
    pub fps: u32,
}

struct CameraActor {
    camera: Camera,
    name : String,
    state: CameraState,
    capabilities: Option<CameraCapabilities>,
    config: Option<CaptureConfig>,
    frame_sequence: usize,
}

/// Handle for controlling a camera actor.
///
/// This handle allows sending commands to the camera actor thread and
/// provides a method to gracefully shut down the actor.
///
/// # Examples
///
/// ```no_run
/// use streaming_capture::{spawn_camera_actor, CameraCommand, CameraEvent, PixelFormat};
///
/// let (handle, mut events) = spawn_camera_actor("/dev/video0")?;
///
/// // Discover camera capabilities
/// handle.send_command(CameraCommand::DiscoverCapabilities)?;
///
/// // Configure camera
/// handle.send_command(CameraCommand::SetConfiguration {
///     width: 1280,
///     height: 720,
///     fps: 30,
///     format: PixelFormat::MJPG,
/// })?;
///
/// // Start streaming
/// handle.send_command(CameraCommand::StartStreaming)?;
///
/// // Process events...
///
/// // Shutdown when done
/// handle.shutdown()?;
/// # Ok::<(), streaming_capture::CameraError>(())
/// ```
pub struct CameraHandle {
    command_tx: mpsc::Sender<CameraCommand>,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

impl CaptureConfig {
    pub fn new(format: PixelFormat, resolution: Resolution, fps: u32) -> Self {
        Self {
            format,
            resolution,
            fps,
        }
    }
}

impl CameraActor {
    fn new(device_path: &str) -> Result<Self, CameraError> {
        let camera = Camera::new(&device_path)
            .map_err(|e| CameraError::IoError(format!("Failed to open: {}", e)))?;

        Ok(Self {
            camera,
            name: device_path.to_string(),
            state: CameraState::Idle,
            capabilities: None,
            config: None,
            frame_sequence: 0,
        })
    }

    fn set_interface(&mut self, device_path: &str) -> Result<(), CameraError> {
        if self.state == CameraState::Streaming {
            self.stop_streaming()?;
        }

        let new_camera = Camera::new(&device_path).map_err(|e| CameraError::IoError(format!("Failed to open: {}", e)))?;
        self.camera = new_camera;
        self.name = device_path.to_string();
        self.state = CameraState::Idle;
        self.capabilities = None;
        self.frame_sequence = 0;

        Ok(())
    }

    fn discover_capabilities(&mut self) {
        let mut formats = Vec::new();

        for format in self.camera.formats() {
            if let Ok(format) = format {
                let pixel_format = PixelFormat::from_fourcc(&format.format);
                if let Ok(resolution_info) = self.camera.resolutions(&format.format) {
                    let resolutions = match resolution_info {
                        rscam::ResolutionInfo::Discretes(sizes) => {
                            sizes
                                .into_iter()
                                .map(|(w, h)| Resolution {
                                    width: w,
                                    height: h,
                                })
                                .collect()
                        }
                        _ => Vec::new(),
                    };

                    formats.push(FormatCapability {
                        format: pixel_format,
                        resolutions,
                    });
                }
            }
        }

        self.capabilities = Some(CameraCapabilities { formats });
    }

    fn set_configuration(&mut self, width: u32, height: u32, fps: u32, format: PixelFormat) -> Result<(), CameraError> {

        if let Some(capabilities) = &self.capabilities {
            let pixel_format = capabilities.formats.iter().find(|cap| cap.format == format).ok_or(CameraError::UnsupportedFormat(format))?;
            let resolution = pixel_format.resolutions.iter().find(|res| res.width == width && res.height == height).ok_or(CameraError::UnsupportedResolution(width, height, format))?;

            let config = CaptureConfig{
                format,
                resolution: *resolution,
                fps: fps,
            };

            self.config = Some(config);

            self.state = CameraState::Configured;
            return Ok(());
        }
        Err(CameraError::CapabilitiesNotDiscovered)
    }

    fn get_configuration(&self) -> Result<CaptureConfig, CameraError> {
        if self.state == CameraState::Configured || self.state == CameraState::Streaming {
            let config = self.config.as_ref().unwrap().clone();
            return Ok(config);
        }
        return Err(CameraError::NotConfigured);
    }

    fn capture_frame(&mut self) -> Result<Frame, CameraError> {
        if self.state != CameraState::Streaming {
            return Err(CameraError::NotStreaming);
        }

        let captured_frame = self.camera.capture()
            .map_err(|e| CameraError::IoError(format!("Failed to capture frame: {}", e)))?;

        self.frame_sequence += 1;

        let frame = Frame {
            format: PixelFormat::from_fourcc(&captured_frame.format),
            width: captured_frame.resolution.0,
            height: captured_frame.resolution.1,
            timestamp: SystemTime::now(),
            sequence: self.frame_sequence,
            data: captured_frame.to_vec()
        };

        Ok(frame)
    }

    fn start_streaming(&mut self) -> Result<(), CameraError> {
        if self.state == CameraState::Configured {
            let config = self.config.as_ref().unwrap();
            let rscam_config = rscam::Config {
                interval: (1, config.fps),
                resolution: (config.resolution.width, config.resolution.height),
                format: &config.format.to_fourcc(),
                ..Default::default()
            };

            self.camera.start(&rscam_config).map_err(|e| CameraError::IoError(format!("Failed to configure camera: {}", e)))?;
            self.state = CameraState::Streaming;
            return Ok(());
        }
        else if self.state == CameraState::Streaming {
            return Err(CameraError::AlreadyStreaming);
        }
        return Err(CameraError::NotConfigured);
    }

    fn stop_streaming(&mut self)  -> Result<(), CameraError> {
        if self.state == CameraState::Streaming {
            self.camera.stop().map_err(|e| CameraError::IoError(format!("Failed to stop camera: {}", e)))?;
            return Ok(());
        }
        return Err(CameraError::NotStreaming);
    }
}

impl CameraHandle{
    /// Send a command to the camera actor.
    ///
    /// # Arguments
    ///
    /// * `command` - The command to send
    ///
    /// # Errors
    ///
    /// Returns an error if the command channel is closed (actor has shut down).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use streaming_capture::{spawn_camera_actor, CameraCommand, PixelFormat};
    /// # let (handle, events) = spawn_camera_actor("/dev/video0")?;
    /// handle.send_command(CameraCommand::DiscoverCapabilities)?;
    /// # Ok::<(), streaming_capture::CameraError>(())
    /// ```
    pub fn send_command(&self, command: CameraCommand) -> Result<(), CameraError> {
        return self.command_tx.blocking_send(command).map_err(|_| CameraError::IoError("Failed to send command".to_string()));
    }

    /// Gracefully shut down the camera actor and wait for the thread to exit.
    ///
    /// This sends a `Shutdown` command to the actor, which will stop any ongoing
    /// streaming, send a `ShutdownComplete` event, and exit its loop. This method
    /// then waits for the actor thread to finish.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The command cannot be sent (channel closed)
    /// - The actor thread panicked
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use streaming_capture::{spawn_camera_actor};
    /// # let (handle, events) = spawn_camera_actor("/dev/video0")?;
    /// // ... use the camera ...
    /// handle.shutdown()?;
    /// # Ok::<(), streaming_capture::CameraError>(())
    /// ```
    pub fn shutdown(mut self) -> Result<(), CameraError> {
        // Send shutdown command
        self.send_command(CameraCommand::Shutdown)?;

        // Wait for thread to finish
        if let Some(handle) = self.join_handle.take() {
            handle.join()
                .map_err(|_| CameraError::IoError("Thread panicked".to_string()))?;
        }
        Ok(())
    }
}

/// Spawn a camera actor thread for the specified device.
///
/// This creates a dedicated thread to manage camera operations asynchronously.
/// The camera is controlled by sending commands through the returned `CameraHandle`,
/// and events are received through the returned channel.
///
/// # Arguments
///
/// * `device_path` - Path to the camera device (e.g., "/dev/video0")
///
/// # Returns
///
/// Returns a tuple of:
/// - `CameraHandle` - Used to send commands to the actor
/// - `mpsc::Receiver<CameraEvent>` - Channel to receive events from the actor
///
/// # Errors
///
/// Returns an error if the camera device cannot be opened.
///
/// # Examples
///
/// ```no_run
/// use streaming_capture::{spawn_camera_actor, CameraCommand, CameraEvent, PixelFormat};
///
/// // Spawn the actor
/// let (handle, mut events) = spawn_camera_actor("/dev/video0")?;
///
/// // Discover capabilities
/// handle.send_command(CameraCommand::DiscoverCapabilities)?;
///
/// // Wait for capabilities event
/// while let Ok(event) = events.blocking_recv() {
///     match event {
///         CameraEvent::CapabilitiesDiscovered(caps) => {
///             println!("Camera supports {} formats", caps.formats.len());
///             break;
///         }
///         _ => {}
///     }
/// }
///
/// // Shutdown when done
/// handle.shutdown()?;
/// # Ok::<(), streaming_capture::CameraError>(())
/// ```
pub fn spawn_camera_actor(device_path: &str) -> Result<(CameraHandle, mpsc::Receiver<CameraEvent>), CameraError> {
    let actor = CameraActor::new(device_path)?;

    let (command_tx, command_rx) = mpsc::channel(10);
    let (event_tx, event_rx) = mpsc::channel(100);

    let join_handle = std::thread::spawn(move || {
        camera_actor_loop(actor, command_rx, event_tx);
    });

    let handle = CameraHandle{
        command_tx,
        join_handle: Some(join_handle),
    };

    return Ok((handle, event_rx));
}

fn camera_actor_loop(mut actor: CameraActor, mut command_rx: mpsc::Receiver<CameraCommand>, event_tx: mpsc::Sender<CameraEvent>) {
    loop {
        // Try to receive command (non-blocking)
        match command_rx.try_recv() {
            Ok(command) => {
                match command {
                    CameraCommand::SetInterface(device_path) => {
                        match actor.set_interface(&device_path) {
                            Ok(()) => {
                                let _ = event_tx.blocking_send(CameraEvent::InterfaceChanged);
                            }
                            Err(e) => {
                                let _ = event_tx.blocking_send(CameraEvent::Error(e));
                            }
                        }
                    }
                    CameraCommand::DiscoverCapabilities => {
                        actor.discover_capabilities();
                        if let Some(caps) = actor.capabilities.clone() {
                            let _ = event_tx.blocking_send(CameraEvent::CapabilitiesDiscovered(caps));
                        }
                    }
                    CameraCommand::GetConfiguration => {
                        match actor.get_configuration() {
                            Ok(config) => {
                                let _ = event_tx.blocking_send(CameraEvent::ConfigurationRetrieved(config));
                            }
                            Err(e) => {
                                let _ = event_tx.blocking_send(CameraEvent::Error(e));
                            }
                        }
                    }
                    CameraCommand::SetConfiguration{ width, height, fps, format } => {
                        match actor.set_configuration(width, height, fps, format) {
                            Ok(()) => {
                                let _ = event_tx.blocking_send(CameraEvent::Configured);
                            }
                            Err(e) => {
                                let _ = event_tx.blocking_send(CameraEvent::Error(e));
                            }
                        }
                    }
                    CameraCommand::StartStreaming => {
                        match actor.start_streaming() {
                            Ok(()) => {
                                let _ = event_tx.blocking_send(CameraEvent::StreamingStarted);
                            }
                            Err(e) => {
                                let _ = event_tx.blocking_send(CameraEvent::Error(e));
                            }
                        }
                    }
                    CameraCommand::StopStreaming => {
                        match actor.stop_streaming() {
                            Ok(()) => {
                                let _ = event_tx.blocking_send(CameraEvent::StreamingStopped);
                            }
                            Err(e) => {
                                let _ = event_tx.blocking_send(CameraEvent::Error(e));
                            }
                        }
                    }
                    CameraCommand::Shutdown => {
                        // Stop streaming if active
                        if actor.state == CameraState::Streaming {
                            let _ = actor.stop_streaming();
                        }
                        // Send shutdown event
                        let _ = event_tx.blocking_send(CameraEvent::ShutdownComplete);
                        // Exit loop - thread will end naturally
                        break;
                    }
                }
            }
            Err(mpsc::error::TryRecvError::Empty) => {
                // No command waiting - that's fine, continue
            }
            Err(mpsc::error::TryRecvError::Disconnected) => {
                // Channel closed - clean shutdown
                if actor.state == CameraState::Streaming {
                    let _ = actor.stop_streaming();
                }
                break;
            }
        }

        // If streaming, capture and send frame
        if actor.state == CameraState::Streaming {
            if let Ok(frame) = actor.capture_frame() {
                let _ = event_tx.blocking_send(CameraEvent::FrameCaptured(frame));
            }
        }
    }
}

/// Discover all camera devices available in /dev/
///
/// Returns a vector of full device paths (e.g., "/dev/video0", "/dev/video1")
pub fn discover_cameras() -> Vec<String> {
    const VIDEO_INTERFACE_PATH: &str = "/dev/";
    const VIDEO_INTERFACE_PREFIX: &str = "video";

    let mut result = Vec::new();

    if let Ok(entries) = Path::new(VIDEO_INTERFACE_PATH).read_dir() {
        for entry in entries {
            if let Ok(entry) = entry {
                let filename = entry.file_name().to_string_lossy().to_string();
                if filename.starts_with(VIDEO_INTERFACE_PREFIX) {
                    result.push(format!("{}{}", VIDEO_INTERFACE_PATH, filename));
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_cameras() {
        // This will only pass if cameras exist on the system
        let cameras = discover_cameras();
        println!("Found cameras: {:?}", cameras);
    }
}
