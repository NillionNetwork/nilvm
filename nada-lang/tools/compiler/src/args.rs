use clap::Parser;

#[derive(Parser, Debug)]
#[clap()]
pub struct Args {
    /// Directory where the input file is located. The output files will also be saved in this directory.
    #[clap(short, long, default_value = "./nada-lang/tests/resources/programs/target")]
    pub(crate) directory_path: String,
    /// Input model file name. It must be saved in binary format
    #[clap(short, long)]
    pub(crate) file_name: Option<String>,
    /// Indicates if the bytecode model will be saved as json
    #[clap(short, long)]
    pub(crate) json_format: bool,
}

pub fn args() -> Args {
    Args::parse()
}
