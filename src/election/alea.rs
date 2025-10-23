use rand::Rng;

pub struct AleaElection;

impl AleaElection {
    pub fn elect_random_server(server_count: usize) -> u32 {
        let mut rng = rand::thread_rng();
        rng.gen_range(1..=server_count as u32)
    }
}