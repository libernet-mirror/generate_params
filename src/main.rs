use anyhow::{Result, anyhow};
use blstrs::{G1Projective, G2Projective, Scalar};
use clap::Parser;
use ff::PrimeField;
use group::{Group, GroupEncoding};
use primitive_types::{H384, H512, H768, U512};
use std::fs::File;
use std::io::Write;
use std::pin::Pin;
use std::sync::{Arc, LazyLock, Mutex, atomic::AtomicUsize, atomic::Ordering};
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

fn get_random_bytes() -> H512 {
    let mut bytes = [0u8; 64];
    getrandom::fill(&mut bytes).unwrap();
    H512::from_slice(&bytes)
}

fn h512_to_scalar(h512: H512) -> Scalar {
    static MODULUS: LazyLock<U512> = LazyLock::new(|| Scalar::MODULUS.parse().unwrap());
    let dividend = U512::from_little_endian(h512.as_bytes());
    let quotient = dividend / *MODULUS;
    let remainder = dividend - quotient * *MODULUS;
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&remainder.to_little_endian()[0..32]);
    Scalar::from_repr_vartime(bytes).unwrap()
}

fn get_random_scalar() -> Scalar {
    h512_to_scalar(get_random_bytes())
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
        let generator = Arc::pin(Self {
            tau: get_random_scalar(),
            g1_count: AtomicUsize::new(0),
            g2_count: AtomicUsize::new(0),
            print_mutex: Mutex::default(),
            reporter_handle: Mutex::default(),
            g1_generator_handle: Mutex::default(),
            g2_generator_handle: Mutex::default(),
        });
        generator.clone().start_reporting();
        generator
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
                    for point in chunk.as_slice() {
                        file.write(point.as_fixed_bytes())?;
                    }
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
                    for point in chunk.as_slice() {
                        file.write(point.as_fixed_bytes())?;
                    }
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
