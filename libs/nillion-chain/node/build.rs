use object_store::{aws::AmazonS3Builder, path::Path as ObjectStorePath, ObjectStore};
use std::{
    env, fs,
    fs::{set_permissions, File},
    io::Write,
    os::unix::fs::PermissionsExt,
    path::Path,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let version = "latest";
    let target = env::var("TARGET").expect("TARGET environment variable not set");
    let target_parts: Vec<&str> = target.split('-').collect();
    if target_parts.len() < 3 {
        panic!("Unexpected TARGET format: {}", target);
    }
    let target_arch = target_parts.first().expect("missing target arch");
    let target_os = target_parts.get(2).expect("missing target os");

    let target_arch = match *target_arch {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        target_arch => {
            eprintln!("Error: unsupported architecture: {}", target_arch);
            std::process::exit(1);
        }
    };

    if !["linux", "darwin"].contains(target_os) {
        eprintln!("Error: unsupported OS: {}", target_os);
        std::process::exit(1);
    }

    let runtime = tokio::runtime::Runtime::new()?;

    runtime.block_on(async {
        let bucket = "nilliond";
        let client = AmazonS3Builder::from_env()
            .with_bucket_name(bucket)
            .with_region("eu-west-1")
            .with_skip_signature(true)
            .build()
            .expect("Failed to create S3 client");

        let key = format!("{version}/{target_os}/{target_arch}/nilchaind");
        let path = ObjectStorePath::parse(&key).expect("Failed to parse path");
        let out_dir = env::var("OUT_DIR").unwrap();
        let target_dir = Path::new(&out_dir).join("node");

        if !target_dir.exists() {
            fs::create_dir_all(&target_dir)?;
        }

        let nilchaind_path = target_dir.join("nilchaind");

        let resp = client.get(&path).await.expect("Failed to download nilchaind");

        let data = resp.bytes().await.expect("Failed to read nilchaind");

        let mut file = File::create(&nilchaind_path)?;
        file.write_all(&data)?;

        let mut permissions = file.metadata()?.permissions();
        permissions.set_mode(0o755);
        set_permissions(&nilchaind_path, permissions)?;

        Ok::<(), Box<dyn std::error::Error>>(())
    })?;

    println!("cargo:rerun-if-env-changed=OUT_DIR");

    Ok(())
}
