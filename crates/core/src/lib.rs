

use std::time::SystemTime;

#[derive(Debug)]
pub struct Frame {
    pub format: PixelFormat,
    pub width: u32,
    pub height: u32,
    pub timestamp: SystemTime,
    pub sequence: usize,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    MJPG,
    YUYV,
    RGB3,
    BGR3,
    YU12,
    YV12,
}

impl PixelFormat {
    /// Convert from V4L2 fourcc bytes to PixelFormat
    pub fn from_fourcc(fourcc: &[u8; 4]) -> Self {
        match fourcc {
            b"MJPG" => PixelFormat::MJPG,
            b"YUYV" => PixelFormat::YUYV,
            b"RGB3" => PixelFormat::RGB3,
            b"BGR3" => PixelFormat::BGR3,
            b"YU12" => PixelFormat::YU12,
            b"YV12" => PixelFormat::YV12,
            _ => PixelFormat::YUYV,  // Default fallback
        }
    }

    pub fn to_fourcc(&self) -> [u8; 4] {
        match self {
            PixelFormat::MJPG => *b"MJPG",
            PixelFormat::YUYV => *b"YUYV",
            PixelFormat::RGB3 => *b"RGB3",
            PixelFormat::BGR3 => *b"BGR3",
            PixelFormat::YU12 => *b"YU12",
            PixelFormat::YV12 => *b"YV12",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct FormatCapability {
    pub format: PixelFormat,
    pub resolutions: Vec<Resolution>,
}

#[derive(Debug, Clone)]
pub struct CameraCapabilities {
    pub formats: Vec<FormatCapability>,
}