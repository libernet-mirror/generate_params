use anyhow::{Result, anyhow};
use clap::Parser;
use dusk_bls12_381::{BlsScalar as Scalar, G1Projective, G2Projective};
use group::GroupEncoding;
use primitive_types::{H384, H768};
use std::fs::File;
use std::io::Write;
use std::sync::{Arc, atomic::AtomicUsize, atomic::Ordering};
use std::time::{Duration, Instant};

const MAX_COUNT: usize = u32::MAX as usize + 1;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Number of parameters to generate, defaulting to `u32::MAX`.
    #[arg(long, default_value = "4294967296")]
    count: usize,

    /// Output file pattern.
    #[arg(long, default_value = "params{}.bin")]
    out_pattern: String,

    /// Number of generator pairs in each chunk.
    #[arg(long, default_value = "65536")]
    chunk_length: usize,
}

fn get_random_scalar() -> Scalar {
    let mut bytes = [0u8; 64];
    getrandom::fill(&mut bytes).unwrap();
    Scalar::from_bytes_wide(&bytes)
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.count > MAX_COUNT {
        return Err(anyhow!(
            "invalid number of entries requested: {} (must be at most {})",
            args.count,
            MAX_COUNT
        ));
    }
    if args.chunk_length < 2 {
        return Err(anyhow!("each chunk must have at least 2 elements"));
    }
    println!("Chunk length: {}", args.chunk_length);
    println!("Out file pattern: {}", args.out_pattern);

    let mut chunk = vec![(H384::zero(), H768::zero()); args.chunk_length];
    let tau = get_random_scalar();

    println!("Generating {} point pairs...", args.count);

    let mut g1 = G1Projective::generator();
    let mut g2 = G2Projective::generator();
    let mut hex1 = H384::from_slice(g1.to_bytes().as_ref());
    let mut hex2 = H768::from_slice(g2.to_bytes().as_ref());
    chunk[0] = (hex1, hex2);

    let index = Arc::pin(AtomicUsize::new(0));

    let index2 = index.clone();
    let reporter = std::thread::spawn(move || {
        let start = Instant::now();
        loop {
            std::thread::sleep(Duration::from_secs(1));
            print!(
                "\r{} generated in {} seconds",
                index2.load(Ordering::Acquire),
                (Instant::now() - start).as_secs()
            );
            std::io::stdout().flush().unwrap();
        }
    });

    loop {
        let index = index.fetch_add(1, Ordering::AcqRel);
        if index >= MAX_COUNT {
            reporter.join().unwrap();
            return Ok(());
        }
        g1 *= tau;
        g2 *= tau;
        hex1 = H384::from_slice(g1.to_bytes().as_ref());
        hex2 = H768::from_slice(g2.to_bytes().as_ref());
        chunk[index % args.chunk_length] = (hex1, hex2);
        if index % args.chunk_length == args.chunk_length - 1 {
            let chunk_index = index / args.chunk_length;
            let path = args
                .out_pattern
                .replace("{}", chunk_index.to_string().as_str());
            {
                let mut file = File::create(path.as_str())?;
                bincode::serde::encode_into_std_write(
                    &chunk,
                    &mut file,
                    bincode::config::standard(),
                )?;
            }
            println!("{} written", path);
        }
    }
}
