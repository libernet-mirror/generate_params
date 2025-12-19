use anyhow::{Result, anyhow};
use blstrs::{G1Projective, G2Projective, Scalar};
use clap::Parser;
use dusk_bls12_381::BlsScalar as DuskScalar;
use group::{Group, GroupEncoding};
use primitive_types::{H384, H768};
use std::fs::File;
use std::io::Write;
use std::pin::Pin;
use std::sync::{Arc, Mutex, atomic::AtomicUsize, atomic::Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const MAX_COUNT: usize = u32::MAX as usize + 1;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Number of BLS12-381 G1 points to generate, defaulting to `u32::MAX+1`.
    #[arg(long, default_value = "4294967296")]
    g1_count: usize,

    /// Number of BLS12-381 G2 points to generate, defaulting to `u32::MAX+1`.
    #[arg(long, default_value = "4294967296")]
    g2_count: usize,

    /// G1 file pattern (for BLS12-381 G1).
    #[arg(long, default_value = "g1_{}.bin")]
    g1_pattern: String,

    /// G2 file pattern (for BLS12-381 G2).
    #[arg(long, default_value = "g2_{}.bin")]
    g2_pattern: String,

    /// Number of G1 points in each chunk.
    #[arg(long, default_value = "65536")]
    g1_chunk_length: usize,

    /// Number of G2 points in each chunk.
    #[arg(long, default_value = "65536")]
    g2_chunk_length: usize,
}

fn get_random_scalar() -> Scalar {
    let mut bytes = [0u8; 64];
    getrandom::fill(&mut bytes).unwrap();
    let scalar = DuskScalar::from_bytes_wide(&bytes);
    Scalar::from_bytes_le(&scalar.to_bytes())
        .into_option()
        .unwrap()
}

#[derive(Debug)]
struct Generator {
    tau: Scalar,
    g1_count: AtomicUsize,
    g2_count: AtomicUsize,
    print_mutex: Mutex<()>,
    reporter_handle: Mutex<Option<JoinHandle<Result<()>>>>,
    g1_generator_handle: Mutex<Option<JoinHandle<Result<()>>>>,
    g2_generator_handle: Mutex<Option<JoinHandle<Result<()>>>>,
}

impl Generator {
    fn start_reporting(self: Pin<Arc<Self>>) {
        let generator = self.clone();
        let mut handle = generator.reporter_handle.lock().unwrap();
        *handle = Some(std::thread::spawn(move || {
            let start = Instant::now();
            loop {
                std::thread::sleep(Duration::from_secs(1));
                print!(
                    "\r{} G1 pts and {} G2 pts generated in {} seconds",
                    self.g1_count.load(Ordering::Acquire),
                    self.g2_count.load(Ordering::Acquire),
                    (Instant::now() - start).as_secs()
                );
                std::io::stdout().flush().unwrap();
            }
        }));
    }

    fn new() -> Pin<Arc<Self>> {
        let reporter = Arc::pin(Self {
            tau: get_random_scalar(),
            g1_count: AtomicUsize::new(0),
            g2_count: AtomicUsize::new(0),
            print_mutex: Mutex::default(),
            reporter_handle: Mutex::default(),
            g1_generator_handle: Mutex::default(),
            g2_generator_handle: Mutex::default(),
        });
        reporter.clone().start_reporting();
        reporter
    }

    fn println(&self, s: impl AsRef<str>) {
        let _lock = self.print_mutex.lock().unwrap();
        println!("{}", s.as_ref());
    }

    fn generate_g1(
        self: Pin<Arc<Self>>,
        count: usize,
        pattern: &str,
        chunk_length: usize,
    ) -> Result<()> {
        if count > MAX_COUNT {
            return Err(anyhow!(
                "invalid number of entries requested: {} (must be at most {})",
                count,
                MAX_COUNT
            ));
        }
        if chunk_length < 2 {
            return Err(anyhow!("each chunk must have at least 2 elements"));
        }

        self.println(format!("Generating {} G1 points...", count));

        let mut chunk = vec![H384::zero(); chunk_length];
        let mut g = G1Projective::generator();
        loop {
            let index = self.g1_count.fetch_add(1, Ordering::AcqRel);
            if index >= MAX_COUNT {
                return Ok(());
            }
            g *= self.tau;
            chunk[index % chunk_length] = H384::from_slice(g.to_bytes().as_ref());
            if index % chunk_length == chunk_length - 1 {
                let chunk_index = index / chunk_length;
                let path = pattern.replace("{}", chunk_index.to_string().as_str());
                {
                    let mut file = File::create(path.as_str())?;
                    bincode::serde::encode_into_std_write(
                        &chunk,
                        &mut file,
                        bincode::config::standard(),
                    )?;
                }
                self.println(format!("\n{} written", path));
            }
        }
    }

    fn start_generate_g1(self: Pin<Arc<Self>>, count: usize, pattern: String, chunk_length: usize) {
        let generator = self.clone();
        let mut handle = generator.g1_generator_handle.lock().unwrap();
        *handle = Some(std::thread::spawn(move || {
            self.generate_g1(count, pattern.as_str(), chunk_length)
        }));
    }

    fn generate_g2(
        self: Pin<Arc<Self>>,
        count: usize,
        pattern: &str,
        chunk_length: usize,
    ) -> Result<()> {
        if count > MAX_COUNT {
            return Err(anyhow!(
                "invalid number of entries requested: {} (must be at most {})",
                count,
                MAX_COUNT
            ));
        }
        if chunk_length < 2 {
            return Err(anyhow!("each chunk must have at least 2 elements"));
        }

        self.println(format!("Generating {} G2 points...", count));

        let mut chunk = vec![H768::zero(); chunk_length];
        let mut g = G2Projective::generator();
        loop {
            let index = self.g2_count.fetch_add(1, Ordering::AcqRel);
            if index >= MAX_COUNT {
                return Ok(());
            }
            g *= self.tau;
            chunk[index % chunk_length] = H768::from_slice(g.to_bytes().as_ref());
            if index % chunk_length == chunk_length - 1 {
                let chunk_index = index / chunk_length;
                let path = pattern.replace("{}", chunk_index.to_string().as_str());
                {
                    let mut file = File::create(path.as_str())?;
                    bincode::serde::encode_into_std_write(
                        &chunk,
                        &mut file,
                        bincode::config::standard(),
                    )?;
                }
                self.println(format!("\n{} written", path));
            }
        }
    }

    fn start_generate_g2(self: Pin<Arc<Self>>, count: usize, pattern: String, chunk_length: usize) {
        let generator = self.clone();
        let mut handle = generator.g2_generator_handle.lock().unwrap();
        *handle = Some(std::thread::spawn(move || {
            self.generate_g2(count, pattern.as_str(), chunk_length)
        }));
    }

    fn join_all(&self) {
        for handle in [
            &self.g1_generator_handle,
            &self.g2_generator_handle,
            &self.reporter_handle,
        ] {
            let mut handle = handle.lock().unwrap();
            if let Some(handle) = handle.take() {
                let _ = handle.join().unwrap();
            }
        }
    }
}

impl Drop for Generator {
    fn drop(&mut self) {
        self.join_all();
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    println!("G1 chunk length: {}", args.g1_chunk_length);
    println!("G2 chunk length: {}", args.g2_chunk_length);
    println!("G1 file pattern: {}", args.g1_pattern);
    println!("G2 file pattern: {}", args.g2_pattern);

    let generator = Generator::new();

    generator.clone().start_generate_g1(
        args.g1_count,
        args.g1_pattern.clone(),
        args.g1_chunk_length,
    );

    generator.clone().start_generate_g2(
        args.g2_count,
        args.g2_pattern.clone(),
        args.g2_chunk_length,
    );

    generator.join_all();

    Ok(())
}
