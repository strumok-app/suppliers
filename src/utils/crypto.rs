use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use aes_gcm::{Aes256Gcm, Nonce, aead::Aead};
use anyhow::anyhow;
use rc4::KeyInit;

type AesCbc128Dec = cbc::Decryptor<aes::Aes128>;
type AesCbcDec = cbc::Decryptor<aes::Aes256>;
type AesCbcEnc = cbc::Encryptor<aes::Aes256>;

pub fn decrypt_aes128(key: &[u8], iv: &[u8], ct: &[u8]) -> anyhow::Result<Vec<u8>> {
    let cipher = AesCbc128Dec::new_from_slices(key, iv)
        .map_err(|e| anyhow!("aes cbc chiper init fails: {e}"))?;

    let pt = cipher
        .decrypt_padded_vec_mut::<Pkcs7>(ct)
        .map_err(|e| anyhow!("aes cbc decrypt fails {e}"))?;

    Ok(pt)
}

pub fn decrypt_aes(key: &[u8], iv: &[u8], ct: &[u8]) -> anyhow::Result<Vec<u8>> {
    let cipher = AesCbcDec::new_from_slices(key, iv)
        .map_err(|e| anyhow!("aes cbc chiper init fails: {e}"))?;

    let pt = cipher
        .decrypt_padded_vec_mut::<Pkcs7>(ct)
        .map_err(|e| anyhow!("aes cbc decrypt fails {e}"))?;

    Ok(pt)
}

pub fn decrypt_aes_gcm(key: &[u8], iv: &[u8], ct: &[u8]) -> anyhow::Result<Vec<u8>> {
    let nonce = Nonce::from_slice(iv);
    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(key);

    let cipher = Aes256Gcm::new(&key);

    let pt = cipher
        .decrypt(nonce, ct)
        .map_err(|e| anyhow!("aes gcm decrypt fails: {e:?}"))?;

    Ok(pt)
}

pub fn encrypt_aes(key: &[u8], iv: &[u8], pt: &[u8]) -> anyhow::Result<Vec<u8>> {
    let cipher = AesCbcEnc::new_from_slices(key, iv).map_err(|e| anyhow!(e))?;

    let ct = cipher.encrypt_padded_vec_mut::<Pkcs7>(pt);

    Ok(ct)
}

// pub fn decrypt_blowfish_ebc(key: &[u8], ct: &[u8]) -> anyhow::Result<Vec<u8>> {
//     let mut pt: Vec<u8> = vec![];
//
//     let cipher: Blowfish<byteorder::BE> = Blowfish::new_from_slice(key).map_err(|e| anyhow!(e))?;
//
//     const BLOCK_SIZE: usize = 8;
//     for s in (0..ct.len()).step_by(BLOCK_SIZE) {
//         let input = GenericArray::from_slice(&ct[s..s + BLOCK_SIZE]);
//
//         let buf = &mut [0u8; BLOCK_SIZE];
//         let out = GenericArray::from_mut_slice(buf);
//         cipher.decrypt_block_b2b(input, out);
//
//         pt.extend_from_slice(buf);
//     }
//
//     Ok(pt)
// }

// pub fn sha1_hex(t: &str) -> String {
//     let mut hasher = Sha1::new();
//
//     hasher.update(t);
//     let result = hasher.finalize();
//
//     hex::encode(&result[..])
// }
