use anyhow::anyhow;
use base64::{Engine, prelude::BASE64_STANDARD};
use blowfish::Blowfish;
use cipher::{
    BlockDecrypt, BlockDecryptMut, BlockEncryptMut, KeyInit, KeyIvInit, block_padding,
    generic_array::GenericArray,
};

type AesCbcDec = cbc::Decryptor<aes::Aes256>;
type AesCbcEnc = cbc::Encryptor<aes::Aes256>;

pub fn decrypt_base64_aes(key: &[u8], iv: &[u8], ct_base64: &[u8]) -> anyhow::Result<Vec<u8>> {
    let ct = BASE64_STANDARD.decode(ct_base64)?;
    decrypt_aes(key, iv, &ct)
}

pub fn decrypt_aes(key: &[u8], iv: &[u8], ct: &[u8]) -> anyhow::Result<Vec<u8>> {
    let cipher = AesCbcDec::new_from_slices(key, iv).map_err(|e| anyhow!(e))?;

    let pt = cipher
        .decrypt_padded_vec_mut::<block_padding::Pkcs7>(ct)
        .map_err(|e| anyhow!(e))?;

    Ok(pt)
}

pub fn encrypt_aes(key: &[u8], iv: &[u8], pt: &[u8]) -> anyhow::Result<Vec<u8>> {
    let cipher = AesCbcEnc::new_from_slices(key, iv).map_err(|e| anyhow!(e))?;

    let ct = cipher.encrypt_padded_vec_mut::<block_padding::Pkcs7>(pt);

    Ok(ct)
}

pub fn decrypt_base64_blowfish_ebc(key: &[u8], ct_base64: &[u8]) -> anyhow::Result<Vec<u8>> {
    let ct = BASE64_STANDARD.decode(ct_base64)?;
    let mut pt: Vec<u8> = vec![];

    let cipher: Blowfish<byteorder::BE> = Blowfish::new_from_slice(key).map_err(|e| anyhow!(e))?;

    const BLOC_SIZE: usize = 8;
    for s in (0..ct.len()).step_by(BLOC_SIZE) {
        let input = GenericArray::from_slice(&ct[s..s + BLOC_SIZE]);

        let buf = &mut [0u8; BLOC_SIZE];
        let out = GenericArray::from_mut_slice(buf);
        cipher.decrypt_block_b2b(input, out);

        pt.extend_from_slice(buf);
    }

    Ok(pt)
}

// pub fn sha1_hex(t: &str) -> String {
//     let mut hasher = Sha1::new();
//
//     hasher.update(t);
//     let result = hasher.finalize();
//
//     hex::encode(&result[..])
// }
