#![allow(clippy::doc_lazy_continuation)]

pub mod nillion {
    pub mod nada {
        pub mod operations {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/nillion.nada.operations.v1.rs"));
            }
        }
        pub mod mir {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/nillion.nada.mir.v1.rs"));
            }
        }
        pub mod types {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/nillion.nada.types.v1.rs"));
            }
        }
    }
}
