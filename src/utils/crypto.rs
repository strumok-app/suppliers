use aes::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit, block_padding::Pkcs7};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use anyhow::anyhow;

type AesCbc128Dec = cbc::Decryptor<aes::Aes128>;
type AesCbcDec = cbc::Decryptor<aes::Aes256>;
type AesCbcEnc = cbc::Encryptor<aes::Aes256>;

pub fn decrypt_aes128(key: &[u8], iv: &[u8], ct: &[u8]) -> anyhow::Result<Vec<u8>> {
    let cipher = AesCbc128Dec::new_from_slices(key, iv)
        .map_err(|e| anyhow!("aes cbc chiper init fails: {e}"))?;

    let pt = cipher
        .decrypt_padded_vec::<Pkcs7>(ct)
        .map_err(|e| anyhow!("aes cbc decrypt fails {e}"))?;

    Ok(pt)
}

pub fn decrypt_aes(key: &[u8], iv: &[u8], ct: &[u8]) -> anyhow::Result<Vec<u8>> {
    let cipher = AesCbcDec::new_from_slices(key, iv)
        .map_err(|e| anyhow!("aes cbc chiper init fails: {e}"))?;

    let pt = cipher
        .decrypt_padded_vec::<Pkcs7>(ct)
        .map_err(|e| anyhow!("aes cbc decrypt fails {e}"))?;

    Ok(pt)
}

pub fn decrypt_aes_gcm(key: &[u8], iv: &[u8], ct: &[u8]) -> anyhow::Result<Vec<u8>> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| anyhow!("aes gcm key init fails: {e}"))?;

    let nonce = Nonce::try_from(iv).map_err(|e| anyhow!("aes gcm nonce init fails: {e}"))?;

    let pt = cipher
        .decrypt(&nonce, ct)
        .map_err(|e| anyhow!("aes gcm decrypt fails: {e:?}"))?;

    Ok(pt)
}

pub fn encrypt_aes(key: &[u8], iv: &[u8], pt: &[u8]) -> anyhow::Result<Vec<u8>> {
    let cipher = AesCbcEnc::new_from_slices(key, iv).map_err(|e| anyhow!(e))?;

    let ct = cipher.encrypt_padded_vec::<Pkcs7>(pt);

    Ok(ct)
}
