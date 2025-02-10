use std::io::Result;

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=../nada_dsl/nada_mir/proto");
    std::env::set_var("PROTOC", protobuf_src::protoc());
    let protos = ["nillion/nada/v1/mir.proto", "nillion/nada/v1/operations.proto", "nillion/nada/v1/types.proto"];
    prost_build::compile_protos(&protos, &["../nada_dsl/nada_mir/proto"])?;
    Ok(())
}
