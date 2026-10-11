//! Two vendor pairings captured from a real Joy-Con 2 (R): one from a Mac, one from a ROG Ally
//! on CachyOS. Shared by the protocol, receiver and hub tests.
#![allow(dead_code)]

use deskpuck_ble::vendor::Secrets;
use deskpuck_core::pairing::HostAddress;

pub fn bytes(hex: &str) -> Vec<u8> {
    hex.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect()
}

pub fn block(hex: &str) -> [u8; 16] {
    bytes(hex).try_into().unwrap()
}

/// One pairing as captured from a real Joy-Con 2 (R): what was sent, then what it answered.
pub struct Transcript {
    pub host: &'static str,
    pub a1: &'static str,
    pub a2: &'static str,
    pub exchange: [(&'static str, &'static str); 4],
}

pub const B1: &str = "5c f6 ee 79 2c df 05 e1 ba 2b 63 25 c4 1a 5f 10";

pub const MAC: Transcript = Transcript {
    host: "FC:B2:14:D1:5C:A1",
    a1: "ba 0c 9d f9 fe 94 23 5c 37 1b cc 80 d6 70 d3 fa",
    a2: "ce 08 c4 59 5f 25 19 b3 77 f6 97 60 4b c1 21 4a",
    exchange: [
        (
            "15 91 01 01 00 0e 00 00 00 02 a1 5c d1 14 b2 fc a0 5c d1 14 b2 fc",
            "15 01 01 01 10 78 00 00 01 04 01 72 2b 2a bf ef e0",
        ),
        (
            "15 91 01 04 00 11 00 00 00 ba 0c 9d f9 fe 94 23 5c 37 1b cc 80 d6 70 d3 fa",
            "15 01 01 04 10 78 00 00 01 5c f6 ee 79 2c df 05 e1 ba 2b 63 25 c4 1a 5f 10",
        ),
        (
            "15 91 01 02 00 11 00 00 00 ce 08 c4 59 5f 25 19 b3 77 f6 97 60 4b c1 21 4a",
            "15 01 01 02 10 78 00 00 01 23 3b 63 d7 3f 7d bd e1 54 61 4a c6 d1 01 78 fd",
        ),
        ("15 91 01 03 00 01 00 00 00", "15 01 01 03 10 78 00 00 01"),
    ],
};

pub const ALLY: Transcript = Transcript {
    host: "14:AC:60:2B:4C:EA",
    a1: "0e cb b2 1c 14 3d f8 6a 65 e7 b6 4f 24 43 e9 61",
    a2: "52 d7 f0 e9 c9 21 55 14 6e 41 11 6a 12 fb 1f 0a",
    exchange: [
        (
            "15 91 01 01 00 0e 00 00 00 02 ea 4c 2b 60 ac 14 e9 4c 2b 60 ac 14",
            "15 01 01 01 10 78 00 00 01 04 01 72 2b 2a bf ef e0",
        ),
        (
            "15 91 01 04 00 11 00 00 00 0e cb b2 1c 14 3d f8 6a 65 e7 b6 4f 24 43 e9 61",
            "15 01 01 04 10 78 00 00 01 5c f6 ee 79 2c df 05 e1 ba 2b 63 25 c4 1a 5f 10",
        ),
        (
            "15 91 01 02 00 11 00 00 00 52 d7 f0 e9 c9 21 55 14 6e 41 11 6a 12 fb 1f 0a",
            "15 01 01 02 10 78 00 00 01 c4 d3 8a 1a 2f 27 ab d2 59 52 e6 6a a4 e0 f8 c7",
        ),
        ("15 91 01 03 00 01 00 00 00", "15 01 01 03 10 78 00 00 01"),
    ],
};

pub fn secrets(t: &Transcript) -> Secrets {
    Secrets { host: HostAddress::parse(t.host).unwrap(), a1: block(t.a1), a2: block(t.a2) }
}

pub fn answer(t: &Transcript, step: usize) -> Vec<u8> {
    bytes(t.exchange[step].1)
}

pub fn request(t: &Transcript, step: usize) -> Vec<u8> {
    bytes(t.exchange[step].0)
}
