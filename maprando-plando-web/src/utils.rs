use rand::{rngs::StdRng, Rng, SeedableRng};

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