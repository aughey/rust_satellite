use std::time::Duration;

use elgato_streamdeck::{images::ImageRect, list_devices, new_hidapi, StreamDeck};
use image::Rgb;

fn main() {
    // Create instance of HidApi
    let hid = new_hidapi().unwrap();

    // List devices and unsafely take first one
    let (kind, serial) = list_devices(&hid).remove(0);

    // Connect to the device
    let device = StreamDeck::connect(&hid, kind, &serial).expect("Failed to connect");

    // Print out some info from the device
    println!(
        "Connected to '{}' with version '{}'",
        device.serial_number().unwrap(),
        device.firmware_version().unwrap()
    );

    // Set device brightness
    device.set_brightness(35).unwrap();

    // Use image-rs to load an image
    let image = image::open("download.png").unwrap();

    // Write it to the device
    device.set_button_image(7, image).unwrap();

    let green = image::ImageBuffer::from_pixel(120, 120, image::Rgb([0, 255, 0]));
    let black = image::ImageBuffer::from_pixel(120, 120, image::Rgb([0, 0, 0]));

    let gradient_lcd = {
        let width = 800;
        let height = 100;
        let mut img = image::ImageBuffer::<image::Rgb<u8>, Vec<u8>>::new(width, height);

        for x in 0..width {
            // Calculate the weight of the green component based on the current x-coordinate.
            // This will linearly increase the green component and linearly decrease the red
            // component as x goes from 0 to width.
            let green_weight = x as f32 / width as f32;
            let red_weight = 1.0 - green_weight;

            let red = (255.0 * red_weight) as u8;
            let green = (255.0 * green_weight) as u8;

            for y in 0..height {
                img.put_pixel(x, y, Rgb([red, green, 0]));
            }
        }
        img
    };

    loop {
        let buttons = device.read_input(Some(Duration::from_secs(1)));
        if let Ok(buttons) = buttons {
            match buttons {
                elgato_streamdeck::StreamDeckInput::NoData => {}
                elgato_streamdeck::StreamDeckInput::ButtonStateChange(buttons) => {
                    println!("Button {:?} pressed", buttons);
                    for (index, button) in buttons.into_iter().enumerate() {
                        if button {
                            _ = device.set_button_image(
                                index as u8,
                                image::DynamicImage::ImageRgb8(green.clone()),
                            );
                        } else {
                            _ = device.set_button_image(
                                index as u8,
                                image::DynamicImage::ImageRgb8(black.clone()),
                            );
                        }
                    }
                }
                elgato_streamdeck::StreamDeckInput::EncoderStateChange(encoder) => {
                    println!("Encoder {:?} changed", encoder);
                }

                elgato_streamdeck::StreamDeckInput::EncoderTwist(encoder) => {
                    println!("Encoder {:?} twisted", encoder);
                }
                elgato_streamdeck::StreamDeckInput::TouchScreenPress(x, y) => {
                    println!("Touchscreen pressed at {},{}", x, y);
                }
                elgato_streamdeck::StreamDeckInput::TouchScreenLongPress(x, y) => {
                    println!("Touchscreen long pressed at {},{}", x, y);
                }
                elgato_streamdeck::StreamDeckInput::TouchScreenSwipe(from, to) => {
                    device
                        .write_lcd(
                            0,
                            0,
                            &ImageRect::from_image(image::DynamicImage::ImageRgb8(
                                gradient_lcd.clone(),
                            ))
                            .unwrap(),
                        )
                        .unwrap();
                    println!(
                        "Touchscreen swiped from {},{} to {},{}",
                        from.0, from.1, to.0, to.1
                    );
                }
            }
        }
    }
}
