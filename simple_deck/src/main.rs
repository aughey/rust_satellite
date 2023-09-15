use streamdeck::StreamDeck;

fn main() {
     // Connect to device
     let mut deck =  StreamDeck::connect(0x0fd9, 0x0084, None).unwrap();

     let serial = deck.serial().unwrap();
     println!(
         "Connected to device {}", serial
     );
}