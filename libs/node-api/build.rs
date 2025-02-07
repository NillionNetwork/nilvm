fn main() {
    std::env::set_var("PROTOC", protobuf_src::protoc());
    let protos = [
        // auth
        "proto/nillion/auth/v1/token.proto",
        "proto/nillion/auth/v1/public_key.proto",
        "proto/nillion/auth/v1/user.proto",
        // compute
        "proto/nillion/compute/v1/invoke.proto",
        "proto/nillion/compute/v1/retrieve.proto",
        "proto/nillion/compute/v1/service.proto",
        "proto/nillion/compute/v1/stream.proto",
        // leader queries
        "proto/nillion/leader_queries/v1/service.proto",
        // membership
        "proto/nillion/membership/v1/cluster.proto",
        "proto/nillion/membership/v1/version.proto",
        "proto/nillion/membership/v1/service.proto",
        // payments
        "proto/nillion/payments/v1/balance.proto",
        "proto/nillion/payments/v1/config.proto",
        "proto/nillion/payments/v1/service.proto",
        // permissions
        "proto/nillion/permissions/v1/overwrite.proto",
        "proto/nillion/permissions/v1/permissions.proto",
        "proto/nillion/permissions/v1/retrieve.proto",
        "proto/nillion/permissions/v1/service.proto",
        "proto/nillion/permissions/v1/update.proto",
        // preprocessing
        "proto/nillion/preprocessing/v1/cleanup.proto",
        "proto/nillion/preprocessing/v1/element.proto",
        "proto/nillion/preprocessing/v1/generate.proto",
        "proto/nillion/preprocessing/v1/material.proto",
        "proto/nillion/preprocessing/v1/service.proto",
        "proto/nillion/preprocessing/v1/stream.proto",
        // programs
        "proto/nillion/programs/v1/service.proto",
        "proto/nillion/programs/v1/store.proto",
        // values
        "proto/nillion/values/v1/delete.proto",
        "proto/nillion/values/v1/retrieve.proto",
        "proto/nillion/values/v1/service.proto",
        "proto/nillion/values/v1/store.proto",
        "proto/nillion/values/v1/value.proto",
    ];
    tonic_build::configure()
        .protoc_arg("--fatal_warnings")
        .enum_attribute(
            "nillion.preprocessing.v1.element.PreprocessingElement",
            "#[derive(strum::EnumIter, strum::EnumString, strum::Display)]",
        )
        .enum_attribute(
            "nillion.preprocessing.v1.material.AuxiliaryMaterial",
            "#[derive(strum::EnumIter, strum::EnumString, strum::Display)]",
        )
        .compile_protos(&protos, &["./proto"])
        .expect("compilation failed");
}
