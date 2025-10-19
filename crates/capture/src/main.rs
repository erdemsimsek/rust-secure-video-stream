use rscam::{Camera, Config};
use chrono::{DateTime, TimeZone, Utc};

use image::load_from_memory;

fn main() {

    let camera = Camera::new("/dev/video0");

    if let Ok(mut camera) = camera {

        let config = Config{
            interval: (1,30),
            resolution: (1280, 720),
            format: b"MJPG",
            ..Default::default()
        };


        camera.start(&config).unwrap();

        for i in 1..10 {
            let frame = camera.capture().unwrap();
            println!("Frame {} Timestamp{}", i, Utc.timestamp_nanos(frame.get_timestamp() as i64 * 1000));

            image::load_from_memory(&frame).unwrap().save(format!("frame{}.jpg", i)).unwrap();
        }
    }

    // if let Ok(camera) = camera {
    //     println!("Supported formats:");
    //     for format in camera.formats() {
    //         if let Ok(format) = format {
    //             println!("{:?}", format);
    //             if let Ok(resolution) = camera.resolutions(&format.format) {
    //                 println!("Supported resolutions: {:?}", resolution);
    //             }
    //         }
    //     }
    // }


}
