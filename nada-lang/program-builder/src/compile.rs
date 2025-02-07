//! Utilities to compile programs.

use anyhow::{anyhow, Context, Error, Result};
use fslock::LockFile;
use pynadac::{CompileOutput, Compiler, CompilerOptions, PersistOptions};
use std::{
    env,
    env::current_dir,
    fs::{self, create_dir_all, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::TempDir;

#[cfg(feature = "parallel-build")]
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

const SINGLE_FILE_SIZE_THRESHOLD: u64 = 1024 * 1024 * 2;
const ALL_FILES_SIZE_THRESHOLD: u64 = 1024 * 1024 * 10;

/// The build system.
///
/// This type compiles python programs by executing them, and generates metadata along the way that
/// can then be imported in crate code to have easy access to a program's mir.
pub(crate) struct BuildSystem {
    import_file: BufWriter<File>,
    compiler: Compiler,
}

impl BuildSystem {
    pub(crate) fn new(package: &str) -> Result<Self, Error> {
        let target_dir_paths = TargetDir::new(package)?;
        let target_dir = target_dir_paths.path;
        let options = CompilerOptions { persist: PersistOptions { mir_bin: true, ..Default::default() } };
        let compiler = Compiler::with_options(&target_dir, options);
        create_dir_all(&target_dir).with_context(|| format!("failed to create target directory: {}", &target_dir))?;

        #[cfg(target_family = "unix")]
        Self::create_friendly_target_dir(&target_dir, &target_dir_paths.friendly_path)?;

        let import_file_path = format!("{}/programs.rs", target_dir);
        let file = File::create(import_file_path).context("failed to create import file")?;
        let import_file = BufWriter::new(file);
        Ok(Self { import_file, compiler })
    }

    #[cfg(target_family = "unix")]
    fn create_friendly_target_dir(target_dir: &str, friendly_path: &str) -> Result<(), Error> {
        // This may fail and it's okay. If a real error occurred the next line will fail.
        let _ = fs::remove_dir_all(friendly_path);
        if let Some(path) = Path::new(friendly_path).parent() {
            create_dir_all(path).context("failed to create friendly target path")?;
        }
        std::os::unix::fs::symlink(target_dir, friendly_path).context("failed to link target dir")?;
        Ok(())
    }

    fn write_import_file_header(&mut self) -> Result<(), Error> {
        self.import_file.write_all(r#"program_builder::PackagePrograms::from(["#.as_bytes())?;
        Ok(())
    }

    fn write_import_file_footer(&mut self) -> Result<(), Error> {
        self.import_file.write_all(r#"])"#.as_bytes())?;
        Ok(())
    }

    pub(crate) fn compile(mut self, program_paths: &[String]) -> Result<(), Error> {
        self.write_import_file_header()?;
        #[cfg(feature = "parallel-build")]
        let program_paths = program_paths.par_iter();
        #[cfg(not(feature = "parallel-build"))]
        let program_paths = program_paths.iter();
        let compiler_outputs: Vec<_> = program_paths
            .map(|program_path| {
                self.compiler
                    .compile(program_path)
                    .with_context(|| anyhow!("failed compiling {program_path}"))
                    .map(|output| (program_path, output))
            })
            .collect::<Result<_, _>>()?;
        let mut total_size = 0;
        for (program_path, output) in compiler_outputs {
            self.add_to_import_file(&output)?;

            let meta = fs::metadata(output.mir_bin_file.as_ref().unwrap())?;
            if meta.len() > SINGLE_FILE_SIZE_THRESHOLD {
                println!("cargo::warning=program {program_path:?} is larger than {SINGLE_FILE_SIZE_THRESHOLD} bytes");
            }
            total_size += meta.len();
        }
        if total_size > ALL_FILES_SIZE_THRESHOLD {
            println!("cargo::warning=total compiled files exceeds {SINGLE_FILE_SIZE_THRESHOLD} bytes: {total_size}");
        }
        self.write_import_file_footer()?;
        Ok(())
    }

    fn add_to_import_file(&mut self, output: &CompileOutput) -> Result<(), Error> {
        let name = &output.program_name;
        let mir_path = output.mir_bin_file.as_ref().ok_or_else(|| anyhow!("no MIR path"))?.to_string_lossy();
        let definition = format!(
            r#"
                (
                    "{name}".to_string(),
                    program_builder::ProgramMetadata {{
                        raw_mir: include_bytes!("{mir_path}").to_vec(),
                    }}
                ),
            "#
        );
        self.import_file.write_all(definition.as_bytes())?;
        Ok(())
    }
}

struct TargetDir {
    path: String,
    friendly_path: String,
}

impl TargetDir {
    fn new(package: &str) -> Result<Self, Error> {
        let target_dir =
            env::var_os("OUT_DIR").ok_or_else(|| anyhow!("no OUT_DIR env variable"))?.to_string_lossy().to_string();
        let path = format!("{target_dir}/{package}");

        // This points to `target/debug/build/the-crate-that-imports-us-SOMEHASH`. This tries to strip
        // off the SOMEHASH part so it's easier to find the output directory for debug purposes.
        let friendly_path = target_dir.rsplit_once('-').map(|(base, _)| base).unwrap_or(&target_dir);
        let friendly_path = format!("{friendly_path}/{package}");
        Ok(Self { path, friendly_path })
    }
}

/// Compiles all programs in the given directory.
///
/// This will find all .py files in the provided directory and will compile them all, failing if
/// any of them fails to compile.
///
/// This function is meant to be used in a build.rs file. Inside your crate code, you should then
/// call the macro `include_package` which will import the contents of the generated metadata file
/// for the given package. The "package" parameter needs to be the same here as it is in the
/// `include_package` macro.
pub fn run_on_directory<P, I>(package: &str, paths: I, nada_dsl_path: &Path) -> Result<()>
where
    P: AsRef<Path>,
    I: IntoIterator<Item = P>,
{
    let cwd = current_dir()?;
    rebuild_if_modified_files(&nada_dsl_path.join("nada_dsl"));
    rebuild_if_modified_files(&nada_dsl_path.join("nada_mir"));

    let paths: Vec<PathBuf> = paths
        .into_iter()
        .map(|path| {
            let path = cwd.join(path);
            rebuild_if_modified_files(&path);
            path
        })
        .collect();
    let nada_mir_path = nada_dsl_path.join("nada_mir").to_string_lossy().to_string();
    let nada_dsl_path = nada_dsl_path.to_string_lossy().to_string();
    let virtual_env = TmpVirtualenv::new().context("creating virtualenv")?;

    // We need to lock using a file to prevent multiple processes from trying to install nada_dsl at the same time
    // as pip install will build the package and this can cause issues if multiple processes try to build it at the same time
    // because the build is not isolated nor atomic
    let mut file = LockFile::open("/tmp/program-builder-pip-install-nada_dsl.lock")?;
    file.lock()?;
    virtual_env.pip_install_path(nada_mir_path.as_ref()).context("pip install")?;
    virtual_env.pip_install_path(nada_dsl_path.as_ref()).context("pip install")?;
    file.unlock()?;
    let mut program_paths = Vec::new();
    for path in paths {
        for file in fs::read_dir(&path)? {
            let file = file?;
            let program_path = path.join(file.path());
            let program_path = program_path.to_string_lossy().to_string();
            if program_path.ends_with(".py") {
                program_paths.push(program_path);
            }
        }
    }
    let build_system = BuildSystem::new(package)?;
    build_system.compile(&program_paths)?;
    Ok(())
}

struct TmpVirtualenv {
    temp_dir: TempDir,
}

impl TmpVirtualenv {
    fn new() -> Result<TmpVirtualenv> {
        let temp_dir = tempfile::Builder::new().prefix("program-builder").tempdir()?;
        let path = temp_dir.path().to_string_lossy().to_string();
        let output = Command::new("python").args(["-m", "virtualenv", &path]).output().context("running python")?;
        eprintln!("{}", String::from_utf8(output.stdout).unwrap());
        eprintln!("{}", String::from_utf8(output.stderr).unwrap());

        let virtualenv_bin = temp_dir.path().join("bin").to_string_lossy().to_string();
        let path_env = env::var("PATH")?;
        let new_path = format!("{virtualenv_bin}:{path_env}");
        env::set_var("VIRTUAL_ENV", path);
        env::set_var("PATH", new_path);

        Ok(TmpVirtualenv { temp_dir })
    }

    fn pip_install_path(&self, path: &Path) -> Result<()> {
        let output = Command::new("pip").args(["install", &path.to_string_lossy()]).output()?;
        eprintln!("{}", String::from_utf8(output.stdout).unwrap());
        eprintln!("{}", String::from_utf8(output.stderr).unwrap());
        if !output.status.success() {
            return Err(anyhow!("pip install failed"));
        }
        Ok(())
    }
}

impl Drop for TmpVirtualenv {
    fn drop(&mut self) {
        let virtualenv_bin = self.temp_dir.path().join("bin").to_string_lossy().to_string();
        let Ok(path_env) = env::var("PATH") else {
            return;
        };
        let new_path = path_env.replace(&format!("{virtualenv_bin}:"), "");
        env::set_var("VIRTUAL_ENV", "");
        env::set_var("PATH", new_path);
    }
}

fn rebuild_if_modified_files(path: &Path) {
    let path = path.to_string_lossy();
    println!("cargo:rerun-if-changed={path}");
}
