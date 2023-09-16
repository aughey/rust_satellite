use std::str::FromStr;

use streamdeck::{StreamDeck, Colour};

fn main() {
    // enable tracing
    tracing_subscriber::fmt::init();

     // Connect to device
     let mut deck =  StreamDeck::connect(0x0fd9, 0x0084, None).unwrap();

     let serial = deck.serial().unwrap();
     println!(
         "Connected to device {}", serial
     );

     deck.set_button_rgb(5, &Colour::from_str("ff0000").unwrap()).unwrap();
     deck.set_blocking(true).unwrap();
     loop {
        let buttons = deck.read_buttons(None);
        if let Ok(buttons) = buttons {
            for (index,button) in buttons.into_iter().enumerate() {
                println!("Button {} pressed", button);
                deck.set_button_rgb(index as u8, &Colour::from_str("00ff00").unwrap()).unwrap();
            }
        }
     }
}