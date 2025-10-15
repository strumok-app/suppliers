use base64::{Engine, prelude::BASE64_URL_SAFE};
use rc4::{KeyInit, Rc4, StreamCipher};

// b64: u8cBwTi1CM4XE3BkwG5Ble3AxWgnhKiXD9Cr279yNW0=
const RC4_KEY_L: [u8; 32] = [
    0xbb_u8, 0xc7_u8, 0x01_u8, 0xc1_u8, 0x38_u8, 0xb5_u8, 0x08_u8, 0xce_u8, 0x17_u8, 0x13_u8,
    0x70_u8, 0x64_u8, 0xc0_u8, 0x6e_u8, 0x41_u8, 0x95_u8, 0xed_u8, 0xc0_u8, 0xc5_u8, 0x68_u8,
    0x27_u8, 0x84_u8, 0xa8_u8, 0x97_u8, 0x0f_u8, 0xd0_u8, 0xab_u8, 0xdb_u8, 0xbf_u8, 0x72_u8,
    0x35_u8, 0x6d_u8,
];
// b64: t00NOJ/Fl3wZtez1xU6/YvcWDoXzjrDHJLL2r/IWgcY=
const RC4_KEY_G: [u8; 32] = [
    0xb7_u8, 0x4d_u8, 0x0d_u8, 0x38_u8, 0x9f_u8, 0xc5_u8, 0x97_u8, 0x7c_u8, 0x19_u8, 0xb5_u8,
    0xec_u8, 0xf5_u8, 0xc5_u8, 0x4e_u8, 0xbf_u8, 0x62_u8, 0xf7_u8, 0x16_u8, 0x0e_u8, 0x85_u8,
    0xf3_u8, 0x8e_u8, 0xb0_u8, 0xc7_u8, 0x24_u8, 0xb2_u8, 0xf6_u8, 0xaf_u8, 0xf2_u8, 0x16_u8,
    0x81_u8, 0xc6_u8,
];
// b64: S7I+968ZY4Fo3sLVNH/ExCNq7gjuOHjSRgSqh6SsPJc=
const RC4_KEY_B: [u8; 32] = [
    0x4b_u8, 0xb2_u8, 0x3e_u8, 0xf7_u8, 0xaf_u8, 0x19_u8, 0x63_u8, 0x81_u8, 0x68_u8, 0xde_u8,
    0xc2_u8, 0xd5_u8, 0x34_u8, 0x7f_u8, 0xc4_u8, 0xc4_u8, 0x23_u8, 0x6a_u8, 0xee_u8, 0x08_u8,
    0xee_u8, 0x38_u8, 0x78_u8, 0xd2_u8, 0x46_u8, 0x04_u8, 0xaa_u8, 0x87_u8, 0xa4_u8, 0xac_u8,
    0x3c_u8, 0x97_u8,
];
// b64: 7D4Q8i8dApRj6UWxXbIBEa1UqvjI+8W0UvPH9talJK8=
const RC4_KEY_M: [u8; 32] = [
    0xec_u8, 0x3e_u8, 0x10_u8, 0xf2_u8, 0x2f_u8, 0x1d_u8, 0x02_u8, 0x94_u8, 0x63_u8, 0xe9_u8,
    0x45_u8, 0xb1_u8, 0x5d_u8, 0xb2_u8, 0x01_u8, 0x11_u8, 0xad_u8, 0x54_u8, 0xaa_u8, 0xf8_u8,
    0xc8_u8, 0xfb_u8, 0xc5_u8, 0xb4_u8, 0x52_u8, 0xf3_u8, 0xc7_u8, 0xf6_u8, 0xd6_u8, 0xa5_u8,
    0x24_u8, 0xaf_u8,
];
// b64: 0JsmfWZA1kwZeWLk5gfV5g41lwLL72wHbam5ZPfnOVE=
const RC4_KEY_F: [u8; 32] = [
    0xd0_u8, 0x9b_u8, 0x26_u8, 0x7d_u8, 0x66_u8, 0x40_u8, 0xd6_u8, 0x4c_u8, 0x19_u8, 0x79_u8,
    0x62_u8, 0xe4_u8, 0xe6_u8, 0x07_u8, 0xd5_u8, 0xe6_u8, 0x0e_u8, 0x35_u8, 0x97_u8, 0x02_u8,
    0xcb_u8, 0xef_u8, 0x6c_u8, 0x07_u8, 0x6d_u8, 0xa9_u8, 0xb9_u8, 0x64_u8, 0xf7_u8, 0xe7_u8,
    0x39_u8, 0x51_u8,
];
// b64: pGjzSCtS4izckNAOhrY5unJnO2E1VbrU+tXRYG24vTo=
const SEED32_A: [u8; 32] = [
    0xa4_u8, 0x68_u8, 0xf3_u8, 0x48_u8, 0x2b_u8, 0x52_u8, 0xe2_u8, 0x2c_u8, 0xdc_u8, 0x90_u8,
    0xd0_u8, 0x0e_u8, 0x86_u8, 0xb6_u8, 0x39_u8, 0xba_u8, 0x72_u8, 0x67_u8, 0x3b_u8, 0x61_u8,
    0x35_u8, 0x55_u8, 0xba_u8, 0xd4_u8, 0xfa_u8, 0xd5_u8, 0xd1_u8, 0x60_u8, 0x6d_u8, 0xb8_u8,
    0xbd_u8, 0x3a_u8,
];
// b64: dFcKX9Qpu7mt/AD6mb1QF4w+KqHTKmdiqp7penubAKI=
const SEED32_V: [u8; 32] = [
    0x74_u8, 0x57_u8, 0x0a_u8, 0x5f_u8, 0xd4_u8, 0x29_u8, 0xbb_u8, 0xb9_u8, 0xad_u8, 0xfc_u8,
    0x00_u8, 0xfa_u8, 0x99_u8, 0xbd_u8, 0x50_u8, 0x17_u8, 0x8c_u8, 0x3e_u8, 0x2a_u8, 0xa1_u8,
    0xd3_u8, 0x2a_u8, 0x67_u8, 0x62_u8, 0xaa_u8, 0x9e_u8, 0xe9_u8, 0x7a_u8, 0x7b_u8, 0x9b_u8,
    0x00_u8, 0xa2_u8,
];
// b64: owp1QIY/kBiRWrRn9TLN2CdZsLeejzHhfJwdiQMjg3w=
const SEED32_N: [u8; 32] = [
    0xa3_u8, 0x0a_u8, 0x75_u8, 0x40_u8, 0x86_u8, 0x3f_u8, 0x90_u8, 0x18_u8, 0x91_u8, 0x5a_u8,
    0xb4_u8, 0x67_u8, 0xf5_u8, 0x32_u8, 0xcd_u8, 0xd8_u8, 0x27_u8, 0x59_u8, 0xb0_u8, 0xb7_u8,
    0x9e_u8, 0x8f_u8, 0x31_u8, 0xe1_u8, 0x7c_u8, 0x9c_u8, 0x1d_u8, 0x89_u8, 0x03_u8, 0x23_u8,
    0x83_u8, 0x7c_u8,
];
// b64: H1XbRvXOvZAhyyPaO68vgIUgdAHn68Y6mrwkpIpEue8=
const SEED32_P: [u8; 32] = [
    0x1f_u8, 0x55_u8, 0xdb_u8, 0x46_u8, 0xf5_u8, 0xce_u8, 0xbd_u8, 0x90_u8, 0x21_u8, 0xcb_u8,
    0x23_u8, 0xda_u8, 0x3b_u8, 0xaf_u8, 0x2f_u8, 0x80_u8, 0x85_u8, 0x20_u8, 0x74_u8, 0x01_u8,
    0xe7_u8, 0xeb_u8, 0xc6_u8, 0x3a_u8, 0x9a_u8, 0xbc_u8, 0x24_u8, 0xa4_u8, 0x8a_u8, 0x44_u8,
    0xb9_u8, 0xef_u8,
];
// b64: 2Nmobf/mpQ7+Dxq1/olPSDj3xV8PZkPbKaucJvVckL0=
const SEED32_K: [u8; 32] = [
    0xd8_u8, 0xd9_u8, 0xa8_u8, 0x6d_u8, 0xff_u8, 0xe6_u8, 0xa5_u8, 0x0e_u8, 0xfe_u8, 0x0f_u8,
    0x1a_u8, 0xb5_u8, 0xfe_u8, 0x89_u8, 0x4f_u8, 0x48_u8, 0x38_u8, 0xf7_u8, 0xc5_u8, 0x5f_u8,
    0x0f_u8, 0x66_u8, 0x43_u8, 0xdb_u8, 0x29_u8, 0xab_u8, 0x9c_u8, 0x26_u8, 0xf5_u8, 0x5c_u8,
    0x90_u8, 0xbd_u8,
];
// b64: Rowe+rg/0g==
const PREFIX_KEY_O: [u8; 7] = [
    0x46_u8, 0x8c_u8, 0x1e_u8, 0xfa_u8, 0xb8_u8, 0x3f_u8, 0xd2_u8,
];
// b64: 8cULcnOMJVY8AA==
const PREFIX_KEY_V: [u8; 10] = [
    0xf1_u8, 0xc5_u8, 0x0b_u8, 0x72_u8, 0x73_u8, 0x8c_u8, 0x25_u8, 0x56_u8, 0x3c_u8, 0x00_u8,
];
// b64: n2+Og2Gth8Hh
const PREFIX_KEY_L: [u8; 9] = [
    0x9f_u8, 0x6f_u8, 0x8e_u8, 0x83_u8, 0x61_u8, 0xad_u8, 0x87_u8, 0xc1_u8, 0xe1_u8,
];
// b64: aRpvzH+yoA==
const PREFIX_KEY_P: [u8; 7] = [
    0x69_u8, 0x1a_u8, 0x6f_u8, 0xcc_u8, 0x7f_u8, 0xb2_u8, 0xa0_u8,
];
// b64: ZB4oBi0=
const PREFIX_KEY_W: [u8; 5] = [0x64_u8, 0x1e_u8, 0x28_u8, 0x06_u8, 0x2d_u8];

// ==== schedule section =====
type BitOp = fn(u8) -> u8;

fn add(n: u8, c: u8) -> u8 {
    c.wrapping_add(n)
}

fn sub(n: u8, c: u8) -> u8 {
    c.wrapping_sub(n)
}

fn xor(n: u8, c: u8) -> u8 {
    c ^ n
}

fn rotl(n: u8, c: u8) -> u8 {
    c.rotate_left(n.into())
}

fn sub19(v: u8) -> u8 {
    sub(19, v)
}

fn sub48(v: u8) -> u8 {
    sub(48, v)
}

fn sub170(v: u8) -> u8 {
    sub(170, v)
}

fn xor8(v: u8) -> u8 {
    xor(8, v)
}

fn xor83(v: u8) -> u8 {
    xor(83, v)
}

fn xor163(v: u8) -> u8 {
    xor(163, v)
}

fn xor241(v: u8) -> u8 {
    xor(241, v)
}

fn add82(v: u8) -> u8 {
    add(82, v)
}

fn add176(v: u8) -> u8 {
    add(176, v)
}

fn add223(v: u8) -> u8 {
    add(223, v)
}

fn rotl4(v: u8) -> u8 {
    rotl(4, v)
}

const SCHEDULE_C: [BitOp; 10] = [
    sub48, sub19, xor241, sub19, add223, sub19, sub170, sub19, sub48, xor8,
];

const SCHEDULE_Y: [BitOp; 10] = [
    rotl4, add223, rotl4, xor163, sub48, add82, add223, sub48, xor83, rotl4,
];

const SCHEDULE_B: [BitOp; 10] = [
    sub19, add82, sub48, sub170, rotl4, sub48, sub170, xor8, add82, xor163,
];

const SCHEDULE_J: [BitOp; 10] = [
    add223, rotl4, add223, xor83, sub19, add223, sub170, add223, sub170, xor83,
];

const SCHEDULE_E: [BitOp; 10] = [
    add82, xor83, xor163, add82, sub170, xor8, xor241, add82, add176, rotl4,
];

// === schedule section ===

fn transform(input: &[u8], seed: &[u8], prefix: &[u8], schedule: &[BitOp]) -> Vec<u8> {
    let seed_len = seed.len();
    let prefix_len = prefix.len();
    let schedule_len = schedule.len();

    let mut output = Vec::with_capacity(input.len() + prefix_len);
    for (i, b) in input.iter().enumerate() {
        if i < prefix_len {
            output.push(prefix[i]);
        }

        let op = schedule[i % schedule_len];
        let x = *b ^ seed[i % seed_len];
        let r = op(x);

        output.push(r);
    }

    output
}

pub fn calc(input: &str) -> String {
    let mut bytes = input.as_bytes().to_vec();
    // println!("input: {bytes:?}");

    // RC4 1
    let mut rc4 = Rc4::new(&RC4_KEY_L.into());
    rc4.apply_keystream(&mut bytes);
    // println!("RC 1: {bytes:?}");

    // Step C
    bytes = transform(&bytes, &SEED32_A, &PREFIX_KEY_O, &SCHEDULE_C);

    // RC4 2
    rc4 = Rc4::new(&RC4_KEY_G.into());
    rc4.apply_keystream(&mut bytes);

    // Step Y
    bytes = transform(&bytes, &SEED32_V, &PREFIX_KEY_V, &SCHEDULE_Y);

    // RC4 3
    rc4 = Rc4::new(&RC4_KEY_B.into());
    rc4.apply_keystream(&mut bytes);

    // Step B
    bytes = transform(&bytes, &SEED32_N, &PREFIX_KEY_L, &SCHEDULE_B);

    // RC4 4
    rc4 = Rc4::new(&RC4_KEY_M.into());
    rc4.apply_keystream(&mut bytes);

    // Step J
    bytes = transform(&bytes, &SEED32_P, &PREFIX_KEY_P, &SCHEDULE_J);

    // RC4 5
    rc4 = Rc4::new(&RC4_KEY_F.into());
    rc4.apply_keystream(&mut bytes);

    // Step E
    bytes = transform(&bytes, &SEED32_K, &PREFIX_KEY_W, &SCHEDULE_E);

    BASE64_URL_SAFE.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shoudl_add() {
        let res = add(223, 32);
        assert_eq!(res, 255u8)
    }

    #[test]
    fn should_sub() {
        let res = sub(170, 32);
        assert_eq!(res, 118u8)
    }

    #[test]
    fn should_xor() {
        let res = xor(163, 32);
        assert_eq!(res, 131u8)
    }

    #[test]
    fn shoudl_rotl() {
        let res = rotl(4, 32);
        assert_eq!(res, 2);
    }

    #[test]
    fn should_calc_vrf() {
        let input = "67890 The quick brown fox jumps over the lazy dog 12345";
        let output = "ZBYeRCjYBk0tkZnKW4kTuWBYw-81e-csvu6v17UY4zchviixt67VJ_tj_1EmtDAQKmgIhARRw0Zd0bzjp-76YVgGyrsDNdbbRJ5wsw5YRfwNVgSKzMe1gwTPciDY";
        let result = calc(input);

        assert_eq!(result, output);
    }
}
