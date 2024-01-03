use std::cell::RefCell;
use std::io::{BufRead, Read};
use std::io::{BufReader, Write};
use std::net::TcpStream;

use anyhow::Result;
use companion::Command;
use elgato_streamdeck_local::{HidDevice, HidError};
use image::codecs::png::PngDecoder;

struct StreamWrapper {
    stream: RefCell<std::net::TcpStream>,
    readbuf: RefCell<BufReader<std::net::TcpStream>>,
}

impl HidDevice for StreamWrapper {
    fn send_feature_report(&self, payload: &[u8]) -> Result<(), HidError> {
        self.stream
            .borrow_mut()
            .write_all(format!("send_feature_report {}\n", payload.len()).as_bytes())
            .unwrap();
        self.stream.borrow_mut().write_all(payload).unwrap();
        _ = self.stream.borrow_mut().flush();
        // Wait for an ok back
        let mut line = String::new();
        self.readbuf.borrow_mut().read_line(&mut line).unwrap();
        if line.trim() != "OK" {
            return Err(HidError {});
        }
        Ok(())
    }

    fn get_feature_report(&self, buf: &mut [u8]) -> Result<(), HidError> {
        self.stream
            .borrow_mut()
            .write_all(format!("get_feature_report {} {}\n", buf[0], buf.len()).as_bytes())
            .unwrap();
        _ = self.stream.borrow_mut().flush();
        // read line
        let bytes_read = self.stream.borrow_mut().read(buf).unwrap();
        if bytes_read != buf.len() {
            return Err(HidError {});
        }
        Ok(())
    }

    fn read_timeout(&self, buf: &mut [u8], timeout: i32) -> Result<(), HidError> {
        self.stream
            .borrow_mut()
            .write_all(format!("tryread {}\n", buf.len()).as_bytes())
            .unwrap();
        _ = self.stream.borrow_mut().flush();
        // read line
        let mut line = String::new();
        self.readbuf.borrow_mut().read_line(&mut line).unwrap();

        let bytes_read = line.trim().parse::<usize>().unwrap();

        if bytes_read == 0 {
            return Err(HidError {});
        }
        // read into buffer
        self.stream
            .borrow_mut()
            .read_exact(&mut buf[..bytes_read])
            .unwrap();

        Ok(())
    }

    fn read(&self, buf: &mut [u8]) -> Result<(), HidError> {
        self.stream
            .borrow_mut()
            .write_all(format!("read {}\n", buf.len()).as_bytes())
            .unwrap();
        _ = self.stream.borrow_mut().flush();

        let bytes_read = self.stream.borrow_mut().read(buf).unwrap();
        if bytes_read != buf.len() {
            return Err(HidError {});
        }
        Ok(())
    }

    fn write(&self, buf: &[u8]) -> Result<usize, HidError> {
        self.stream
            .borrow_mut()
            .write_all(format!("write {}\n", buf.len()).as_bytes())
            .unwrap();
        self.stream.borrow_mut().write_all(buf).unwrap();
        _ = self.stream.borrow_mut().flush();
        // Read OK back
        let mut line = String::new();
        self.readbuf.borrow_mut().read_line(&mut line).unwrap();
        if line.trim() != "OK" {
            return Err(HidError {});
        }

        Ok(buf.len())
    }
}

fn main() -> Result<()> {
    // Connect to the teensy_sim
    let stream = std::net::TcpStream::connect("raspberrypi:12345")?;

    let stream = StreamWrapper {
        stream: RefCell::new(stream.try_clone()?),
        readbuf: RefCell::new(BufReader::new(stream)),
    };

    // Connect to companion
    let mut companion_stream = std::net::TcpStream::connect("host.docker.internal:16622")?;
    let mut companion_stream_reader = companion_stream.try_clone()?;
    companion_stream_reader.set_nonblocking(true)?;

    teensy_lib::run_teensy(
        move || {
            let mut buf = [0; 1];
            let bytes_read = companion_stream_reader.read(&mut buf);
            match bytes_read {
                Ok(0) => Ok(None),
                Ok(_) => Ok(Some(buf[0])),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
                Err(e) => Err(e.into()),
            }
        },
        |buf| {
            companion_stream.write_all(buf).unwrap();
            companion_stream.flush().unwrap();
            Ok(())
        },
        stream,
    )?;

    Ok(())
}