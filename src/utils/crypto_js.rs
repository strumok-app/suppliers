use anyhow::anyhow;
use md5::{Digest, Md5};

use super::crypto;

pub fn decrypt_aes_no_salt(password: &[u8], text: &[u8]) -> anyhow::Result<String> {
    let salt = &text[8..16];
    let ct = &text[16..];

    decrypt_aes(password, salt, ct)
}

pub fn decrypt_aes(password: &[u8], salt: &[u8], ct: &[u8]) -> anyhow::Result<String> {
    let (key, iv) = derive_key_and_iv(password, salt);

    let pt = crypto::decrypt_aes(&key, &iv, ct)?;

    String::from_utf8(pt).map_err(|e| anyhow!(e))
}

fn derive_key_and_iv(password: &[u8], salt: &[u8]) -> (Vec<u8>, Vec<u8>) {
    const KEY_LENGTH: usize = 32;
    const IV_LENGTH: usize = 16;
    const HASH_SIZE: usize = KEY_LENGTH + IV_LENGTH;
    const DIGIT_SIZE: usize = 16;

    let mut hash: [u8; HASH_SIZE] = [0; HASH_SIZE];
    let mut calculatated = 0;

    while calculatated < HASH_SIZE {
        let mut hasher = Md5::new();
        if calculatated > 0 {
            hasher.update(&hash[(calculatated - DIGIT_SIZE)..calculatated])
        }

        let out = hasher.chain_update(password).chain_update(salt).finalize();

        hash[calculatated..(calculatated + DIGIT_SIZE)].copy_from_slice(out.as_slice());

        calculatated += DIGIT_SIZE;
    }

    (hash[0..KEY_LENGTH].to_vec(), hash[KEY_LENGTH..].to_vec())
}

#[test]
fn should_derive_key_and_iv() {
    const PASSWORD: &[u8] = &[
        54, 105, 68, 117, 114, 77, 99, 50, 108, 82, 65, 121, 71, 85, 69, 72, 80, 73, 73, 116,
    ];
    const SALT: &[u8] = &[129, 183, 235, 45, 252, 39, 206, 174];
    const RESULT_KEY: &[u8] = &[
        120, 45, 195, 34, 78, 14, 231, 234, 67, 192, 62, 118, 235, 45, 97, 233, 50, 86, 206, 75,
        148, 31, 180, 1, 72, 43, 144, 123, 82, 233, 193, 178,
    ];
    const RESULT_IV: &[u8] = &[
        62, 204, 205, 162, 189, 145, 136, 18, 170, 229, 108, 237, 29, 86, 47, 51,
    ];

    let (key, iv) = derive_key_and_iv(PASSWORD, SALT);

    assert_eq!(RESULT_KEY, key);
    assert_eq!(RESULT_IV, iv);
}

#[test]
fn should_decrypt_aes() {
    const PASSWORD: &[u8] = &[
        100, 102, 52, 88, 103, 74, 53, 84, 75, 68, 119, 89, 114, 77, 50, 53, 103, 71, 117, 65, 122,
        49, 107, 78, 77, 109, 103, 97, 81, 79, 107, 52, 121, 90, 90, 110, 105, 89, 71, 108, 82, 83,
        56, 107,
    ];
    const SALT: &[u8] = &[182, 242, 87, 51, 224, 177, 50, 101];
    const CT: &[u8] = &[
        247, 255, 113, 62, 111, 215, 232, 131, 35, 125, 147, 4, 105, 62, 171, 126, 139, 90, 0, 226,
        51, 170, 172, 244, 160, 44, 45, 23, 184, 177, 168, 11, 122, 21, 69, 24, 158, 152, 73, 226,
        189, 17, 6, 189, 38, 29, 208, 15, 169, 231, 46, 215, 29, 85, 23, 217, 53, 178, 140, 73, 43,
        181, 152, 43, 136, 134, 123, 144, 121, 181, 145, 113, 66, 183, 247, 190, 160, 229, 143,
        226, 150, 94, 51, 245, 245, 85, 185, 222, 130, 171, 242, 255, 244, 145, 2, 97, 104, 75,
        226, 119, 240, 114, 166, 252, 13, 252, 137, 135, 133, 159, 190, 161, 0, 91, 29, 62, 215,
        105, 45, 59, 187, 15, 58, 116, 49, 152, 146, 240, 67, 115, 172, 64, 35, 93, 214, 123, 62,
        203, 214, 220, 237, 73, 110, 227, 202, 104, 146, 145, 20, 111, 229, 147, 171, 167, 191,
        207, 142, 190, 14, 184, 80, 117, 48, 20, 89, 146, 97, 52, 240, 82, 189, 7, 203, 233, 156,
        51, 190, 231, 178, 24, 97, 51, 27, 246, 82, 220, 41, 219, 237, 76, 161, 128, 89, 160, 43,
        5, 29, 217, 218, 191, 228, 246, 208, 34, 159, 223, 10, 120, 240, 193, 150, 87, 55, 139, 43,
        170, 122, 0, 28, 38, 71, 168, 173, 90, 202, 216, 99, 205, 210, 89, 27, 180, 149, 201, 35,
        125, 112, 30, 236, 4, 140, 227, 33, 147, 108, 231, 115, 140, 93, 2, 90, 5, 119, 36, 136,
        190, 147, 181, 246, 107, 120, 189, 205, 64, 78, 123, 172, 6, 46, 183, 199, 74, 118, 250,
        130, 181, 241, 236, 90, 171, 148, 109, 124, 95, 106, 34, 200, 66, 175, 105, 59, 172, 93,
        10, 250, 223, 108, 232, 160, 60, 80, 189, 62, 59, 54, 68, 165, 201, 45, 133, 117, 152, 72,
        95, 15, 174, 231, 83, 146, 9, 103, 125, 4, 30, 47, 59, 206, 130, 128, 214, 52, 68, 41, 129,
        112, 126, 67, 177, 150, 65, 118, 200, 105, 47, 150, 73, 123, 189, 64, 23, 129, 64, 81, 107,
        223, 121, 64, 208, 149, 155, 173, 56, 19, 220, 248, 8, 252, 205, 201, 123, 223, 52, 34,
        165, 198, 24, 53, 22, 146, 199, 57, 143, 130, 194,
    ];
    const RES: &str = r#"[{"file":"https://ee.netmagcdn.com:2228/hls-playback/48a539d0616d329c4a73a34604f66d4a2b3475e8d0dad0f1b69a76299550ae95580994d563c865f7ee21eadf67f429ed5682616afc55cf30b4d1965d8a485093298bf23d82912e88ba68ea236415c68aef6cf62c60404977c39bf2f9d2210a56ea162c83e61dc37aa69f59809c3f2bfde050ade1775ab0b0348c1d40e9c895159b4235b9a3ef17f88bfe60dfa92ff204/master.m3u8","type":"hls"}]"#;

    let res = decrypt_aes(PASSWORD, SALT, CT).unwrap();

    assert_eq!(RES, res);
}
