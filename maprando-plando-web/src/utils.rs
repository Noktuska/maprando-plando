use base64::{prelude::BASE64_STANDARD_NO_PAD, Engine};
use rand::{rngs::StdRng, Rng, RngCore, SeedableRng};
//use sha3::{Digest, Sha3_256};

/*pub fn hash(v: impl AsRef<[u8]>) -> Vec<u8> {
    let mut hasher = Sha3_256::new();
    hasher.update(v);
    hasher.finalize().to_vec()
}

pub fn hash_base64(v: impl AsRef<[u8]>) -> String {
    let hash = hash(v);
    BASE64_STANDARD_NO_PAD.encode(hash)
}*/

pub fn generate_byte_vec(len: usize) -> Vec<u8> {
    // StdRng should be a sufficient CSPRNG (according to rust-rand-cookbook anyways)
    let mut rng = StdRng::from_os_rng();
    let mut res = vec![0u8; len];
    rng.fill_bytes(&mut res);
    res
}

pub fn generate_random_base64(len: usize) -> String {
    let bytes = generate_byte_vec(len);
    BASE64_STANDARD_NO_PAD.encode(&bytes)
}

pub fn generate_seed_id(len: usize) -> String {
    let alphabet: Vec<char> = ('A'..'Z').chain('a'..'z').chain('0'..'9').collect();
    let mut res = String::with_capacity(len);
    let mut rng = StdRng::from_os_rng();
    for _ in 0..len {
        let idx = rng.random_range(0..alphabet.len());
        res.push(alphabet[idx]);
    }
    res
}