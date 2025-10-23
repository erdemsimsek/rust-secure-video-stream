//! Camera capture library for video streaming.
//!
//! Provides camera discovery, capability querying, and frame capture functionality
//! using V4L2 on Linux.

use std::path::Path;
use std::time::SystemTime;
use rscam::Camera;
use streaming_core::{CameraCapabilities, FormatCapability, PixelFormat, Resolution, Frame};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("Capabilities not discovered. Call discover_capabilities() first.")]
    CapabilitiesNotDiscovered,

    #[error("Unsupported format by camera: {0:?}")]
    UnsupportedFormat(PixelFormat),

    #[error("Resolution {0}x{1} not supported for format {2:?}")]
    UnsupportedResolution(u32, u32, PixelFormat),

    #[error("Camera not configured")]
    NotConfigured,

    #[error("IO error: {0}")]
    IoError(String),
}


pub struct CaptureConfig {
    pub format: PixelFormat,
    pub resolution: Resolution,
    pub fps: u32,
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


pub trait CameraDevice {
    fn start(&self);
    fn capture(&self);
}

/// Represents a camera device instance with its capabilities
pub struct CameraInstance {
    camera: Camera,
    pub name: String,
    pub capabilities: Option<CameraCapabilities>,
    is_configured: bool,
    current_config: Option<CaptureConfig>,
    frame_sequence: usize,
}

impl CameraInstance {
    /// Create a new camera instance from device path
    ///
    /// # Arguments
    /// * `name` - Full device path (e.g., "/dev/video0")
    ///
    /// # Panics
    /// Panics if the camera device cannot be opened
    pub fn new(name: String) -> Self {
        let camera = Camera::new(&name).map_err(
            |e| CaptureError::IoError(format!("Failed to open camera: {}", e))
        );

        Self {
            camera: camera.unwrap(),
            name,
            capabilities: None,
            is_configured: false,
            current_config: None,
            frame_sequence: 0
        }
    }

    pub fn configure(&mut self, width: u32, height: u32, fps: u32, format: PixelFormat) -> Result<(), CaptureError> {

        if let Some(caps) = &self.capabilities {
            let pixel_format = caps.formats.iter().find(|cap| cap.format == format).ok_or(CaptureError::UnsupportedFormat(format))?;
            let resolution = pixel_format.resolutions.iter().find(|res| res.width == width && res.height == height).ok_or(CaptureError::UnsupportedResolution(width, height, format))?;

            let fourcc = format.to_fourcc();

            let config = rscam::Config {
                interval: (1, fps),
                resolution: (width, height),
                format: &fourcc.to_vec(),
                ..Default::default()
            };

            self.camera.start(&config).map_err(|e| CaptureError::IoError(format!("Failed to configure camera: {}", e)))?;

            self.is_configured = true;
            self.current_config = Some(CaptureConfig::new(format, *resolution, fps));

            return Ok(());
        }
        Err(CaptureError::CapabilitiesNotDiscovered)
    }

    pub fn capture_frame(&mut self) -> Result<Frame, CaptureError> {
        if !self.is_configured {
            return Err(CaptureError::NotConfigured);
        }

        let captured_frame = self.camera.capture().map_err(|e| CaptureError::IoError(format!("Failed to capture frame: {}", e)))?;

        self.frame_sequence += 1;

        let frame = Frame {
            format: PixelFormat::from_fourcc(&captured_frame.format),
            width: captured_frame.resolution.0,
            height: captured_frame.resolution.1,
            timestamp: SystemTime::now(),
            sequence: self.frame_sequence,
            data: captured_frame.to_vec()
        };

        return Ok(frame);
    }


    /// Discover all supported formats and resolutions for this camera
    pub fn discover_capabilities(&mut self) {
        let mut formats = Vec::new();

        // Iterate through all supported formats
        for format_result in self.camera.formats() {
            if let Ok(format) = format_result {
                let pixel_format = PixelFormat::from_fourcc(&format.format);

                // Get resolutions for this specific format
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

    /// Print camera capabilities in human-readable format
    pub fn print_capabilities(&self) {
        if let Some(caps) = &self.capabilities {
            println!("Camera: {}", self.name);
            for format_cap in &caps.formats {
                println!("  Format: {:?}", format_cap.format);
                for res in &format_cap.resolutions {
                    println!("    - {}x{}", res.width, res.height);
                }
            }
        } else {
            println!("Camera: {} - Capabilities not discovered", self.name);
        }
    }

    /// Get reference to discovered capabilities
    pub fn get_capabilities(&self) -> Option<&CameraCapabilities> {

        self.capabilities.as_ref()
    }
}

impl std::fmt::Display for CameraInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
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
