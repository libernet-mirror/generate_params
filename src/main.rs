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
    /// Number of parameters to generate, defaulting to `u32::MAX+1`.
    #[arg(long, default_value = "4294967296")]
    count: usize,

    /// G1 file pattern.
    #[arg(long, default_value = "g1_{}.bin")]
    g1_pattern: String,

    /// G2 file path.
    #[arg(long, default_value = "g2.bin")]
    g2_path: String,

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
    println!("G1 file pattern: {}", args.g1_pattern);
    println!("G2 file path: {}", args.g2_path);

    let tau = get_random_scalar();

    {
        let g2 = G2Projective::generator() * tau;
        let hex = H768::from_slice(g2.to_bytes().as_ref());
        let mut file = File::create(args.g2_path.as_str())?;
        bincode::serde::encode_into_std_write(&hex, &mut file, bincode::config::standard())?;
        println!("{} written", args.g2_path.as_str());
    }

    println!("Generating {} G1 points...", args.count);

    let mut chunk = vec![H384::zero(); args.chunk_length];
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

    let mut g = G1Projective::generator();
    loop {
        let index = index.fetch_add(1, Ordering::AcqRel);
        if index >= MAX_COUNT {
            reporter.join().unwrap();
            return Ok(());
        }
        g *= tau;
        chunk[index % args.chunk_length] = H384::from_slice(g.to_bytes().as_ref());
        if index % args.chunk_length == args.chunk_length - 1 {
            let chunk_index = index / args.chunk_length;
            let path = args
                .g1_pattern
                .replace("{}", chunk_index.to_string().as_str());
            {
                let mut file = File::create(path.as_str())?;
                bincode::serde::encode_into_std_write(
                    &chunk,
                    &mut file,
                    bincode::config::standard(),
                )?;
            }
            println!("\n{} written", path);
        }
    }
}
