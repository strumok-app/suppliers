use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use anyhow::anyhow;
use base64::{prelude::BASE64_STANDARD, Engine};

type AesCbcDec = cbc::Decryptor<aes::Aes256>;
type AesCbcEnc = cbc::Encryptor<aes::Aes256>;

pub fn decrypt_base64_aes(key: &[u8], iv: &[u8], ct_base64: &[u8]) -> anyhow::Result<Vec<u8>> {
    let ct = BASE64_STANDARD.decode(ct_base64)?;
    decrypt_aes(key, iv, &ct)
}

pub fn decrypt_aes(key: &[u8], iv: &[u8], ct: &[u8]) -> anyhow::Result<Vec<u8>> {
    let cipher = AesCbcDec::new_from_slices(key, iv).map_err(|e| anyhow!(e))?;

    let pt = cipher
        .decrypt_padded_vec_mut::<Pkcs7>(ct)
        .map_err(|e| anyhow!(e))?;

    Ok(pt)
}

pub fn encrypt_aes(key: &[u8], iv: &[u8], pt: &[u8]) -> anyhow::Result<Vec<u8>> {
    let cipher = AesCbcEnc::new_from_slices(key, iv).map_err(|e| anyhow!(e))?;

    let ct = cipher.encrypt_padded_vec_mut::<Pkcs7>(pt);

    Ok(ct)
}
