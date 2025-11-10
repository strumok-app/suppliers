use base64::{Engine, prelude::BASE64_STANDARD};
use rc4::{Key, KeyInit, Rc4, StreamCipher, consts::U32};

const RC4_KEYS: [&str; 5] = [
    "FgxyJUQDPUGSzwbAq/ToWn4/e8jYzvabE+dLMb1XU1o=",
    "CQx3CLwswJAnM1VxOqX+y+f3eUns03ulxv8Z+0gUyik=",
    "fAS+otFLkKsKAJzu3yU+rGOlbbFVq+u+LaS6+s1eCJs=",
    "Oy45fQVK9kq9019+VysXVlz1F9S1YwYKgXyzGlZrijo=",
    "aoDIdXezm2l3HrcnQdkPJTDT8+W6mcl2/02ewBHfPzg=",
];

const SEEDS32: [&str; 5] = [
    "yH6MXnMEcDVWO/9a6P9W92BAh1eRLVFxFlWTHUqQ474=",
    "RK7y4dZ0azs9Uqz+bbFB46Bx2K9EHg74ndxknY9uknA=",
    "rqr9HeTQOg8TlFiIGZpJaxcvAaKHwMwrkqojJCpcvoc=",
    "/4GPpmZXYpn5RpkP7FC/dt8SXz7W30nUZTe8wb+3xmU=",
    "wsSGSBXKWA9q1oDJpjtJddVxH+evCfL5SO9HZnUDFU8=",
];

const PREFIX_KEYS: [&str; 5] = [
    "l9PavRg=",
    "Ml2v7ag1Jg==",
    "i/Va0UxrbMo=",
    "WFjKAHGEkQM=",
    "5Rr27rWd",
];

const SCHEDULE_0: [fn(u8) -> u8; 10] = [
    |c| c.wrapping_sub(223),
    |c| c.rotate_right(4),
    |c| c.rotate_right(4),
    |c| c.wrapping_add(234),
    |c| c.rotate_right(7),
    |c| c.rotate_right(2),
    |c| c.rotate_right(7),
    |c| c.wrapping_sub(223),
    |c| c.rotate_right(7),
    |c| c.rotate_right(6),
];
const SCHEDULE_1: [fn(u8) -> u8; 10] = [
    |c| c.wrapping_add(19),
    |c| c.rotate_right(7),
    |c| c.wrapping_add(19),
    |c| c.rotate_right(6),
    |c| c.wrapping_add(19),
    |c| c.rotate_right(1),
    |c| c.wrapping_add(19),
    |c| c.rotate_right(6),
    |c| c.rotate_right(7),
    |c| c.rotate_right(4),
];
const SCHEDULE_2: [fn(u8) -> u8; 10] = [
    |c| c.wrapping_sub(223),
    |c| c.rotate_right(1),
    |c| c.wrapping_add(19),
    |c| c.wrapping_sub(223),
    |c| c.rotate_left(2),
    |c| c.wrapping_sub(223),
    |c| c.wrapping_add(19),
    |c| c.rotate_left(1),
    |c| c.rotate_left(2),
    |c| c.rotate_left(1),
];
const SCHEDULE_3: [fn(u8) -> u8; 10] = [
    |c| c.wrapping_add(19),
    |c| c.rotate_left(1),
    |c| c.rotate_left(1),
    |c| c.rotate_right(1),
    |c| c.wrapping_add(234),
    |c| c.rotate_left(1),
    |c| c.wrapping_sub(223),
    |c| c.rotate_left(6),
    |c| c.rotate_left(4),
    |c| c.rotate_left(1),
];
const SCHEDULE_4: [fn(u8) -> u8; 10] = [
    |c| c.rotate_right(1),
    |c| c.rotate_left(1),
    |c| c.rotate_left(6),
    |c| c.rotate_right(1),
    |c| c.rotate_left(2),
    |c| c.rotate_right(4),
    |c| c.rotate_left(1),
    |c| c.rotate_left(1),
    |c| c.wrapping_sub(223),
    |c| c.rotate_left(2),
];

fn transform(
    input: &[u8],
    init_seed_bytes: &[u8],
    prefix_key_bytes: &[u8],
    schedule: &[fn(u8) -> u8],
) -> Vec<u8> {
    let prefix_len = prefix_key_bytes.len();
    let mut out = Vec::new();
    for i in 0..input.len() {
        if i < prefix_len {
            out.push(prefix_key_bytes[i]);
        }
        let transformed = schedule[i % 10](input[i] ^ init_seed_bytes[i % 32]);
        out.push(transformed);
    }
    out
}

pub fn calc(input: &str) -> String {
    let input = urlencoding::encode(input);
    let mut bytes = input.as_bytes().to_vec();

    // 0
    let key = BASE64_STANDARD.decode(RC4_KEYS[0]).unwrap();
    let seed = BASE64_STANDARD.decode(SEEDS32[0]).unwrap();
    let prefix = BASE64_STANDARD.decode(PREFIX_KEYS[0]).unwrap();

    let mut rc4 = Rc4::new(Key::<U32>::from_slice(&key));
    rc4.apply_keystream(&mut bytes);

    bytes = transform(&bytes, &seed, &prefix, &SCHEDULE_0);

    // 1
    let key = BASE64_STANDARD.decode(RC4_KEYS[1]).unwrap();
    let seed = BASE64_STANDARD.decode(SEEDS32[1]).unwrap();
    let prefix = BASE64_STANDARD.decode(PREFIX_KEYS[1]).unwrap();

    let mut rc4 = Rc4::new(Key::<U32>::from_slice(&key));
    rc4.apply_keystream(&mut bytes);

    bytes = transform(&bytes, &seed, &prefix, &SCHEDULE_1);
    // 2
    let key = BASE64_STANDARD.decode(RC4_KEYS[2]).unwrap();
    let seed = BASE64_STANDARD.decode(SEEDS32[2]).unwrap();
    let prefix = BASE64_STANDARD.decode(PREFIX_KEYS[2]).unwrap();

    let mut rc4 = Rc4::new(Key::<U32>::from_slice(&key));
    rc4.apply_keystream(&mut bytes);

    bytes = transform(&bytes, &seed, &prefix, &SCHEDULE_2);
    // 3
    let key = BASE64_STANDARD.decode(RC4_KEYS[3]).unwrap();
    let seed = BASE64_STANDARD.decode(SEEDS32[3]).unwrap();
    let prefix = BASE64_STANDARD.decode(PREFIX_KEYS[3]).unwrap();

    let mut rc4 = Rc4::new(Key::<U32>::from_slice(&key));
    rc4.apply_keystream(&mut bytes);

    bytes = transform(&bytes, &seed, &prefix, &SCHEDULE_3);
    // 4
    let key = BASE64_STANDARD.decode(RC4_KEYS[4]).unwrap();
    let seed = BASE64_STANDARD.decode(SEEDS32[4]).unwrap();
    let prefix = BASE64_STANDARD.decode(PREFIX_KEYS[4]).unwrap();

    let mut rc4 = Rc4::new(Key::<U32>::from_slice(&key));
    rc4.apply_keystream(&mut bytes);

    bytes = transform(&bytes, &seed, &prefix, &SCHEDULE_4);

    let mut encoded = BASE64_STANDARD.encode(&bytes);
    encoded = encoded.replace("+", "-").replace("/", "_").replace("=", "");
    encoded
}

#[cfg(test)]
mod test {
    use crate::suppliers::mangafire::vrf::calc;

    #[test]
    fn test_vrf() {
        assert_eq!(
            calc("67890@ The quick brown fox jumps over the lazy dog @12345"),
            // "ZBYeRCjYBk0tkZnKW4kTuWBYw-81e-csvu6v17UY4zchviixt67VJ\
            //  _tjpFEsOXB-a8X4ZFpDoDbPq8ms-7IyN95vmLVdP5vWSoTAl4ZbIB\
            //  E8xijci8emrkdEYmArOPMUq5KAc3KEabUzHkNwjBtwvs0fQR7nDpI"
            "5fcaUfZo7rW1-Z3vTEvXO5sJBfP2zuTM2NIVmftpuGhYgy8c-Yl92\
			 uQOuxzYksgVMUWKu7h-Pt5_6c0KZ2c1BpRQwVCIkRycge1pensQ__\
			 YViJZddxqB5PvElml6UdQ1h4w8kCFftPUYNoSHTqNBX0HfFg"
        )
    }
}
