use std::time::Instant;

fn main() {
    const ACTIONS: usize = 1_000_000;
    let start = Instant::now();
    let mut checksum: u64 = 0;

    for i in 0..ACTIONS {
        let blended = (i as u64).rotate_left(13).wrapping_mul(6364136223846793005);
        checksum = checksum.wrapping_add(blended);
    }

    let duration = start.elapsed();
    println!("1M action benchmark complete");
    println!(" actions processed : {ACTIONS}");
    println!(" checksum          : {checksum}");
    println!(" duration          : {duration:?}");
}
