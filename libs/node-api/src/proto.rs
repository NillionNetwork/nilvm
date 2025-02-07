//! Protobuf file imports.

pub(crate) mod auth {
    pub mod v1 {
        pub mod public_key {
            tonic::include_proto!("nillion.auth.v1.public_key");
        }

        pub mod token {
            tonic::include_proto!("nillion.auth.v1.token");
        }

        pub mod user {
            tonic::include_proto!("nillion.auth.v1.user");
        }
    }
}

pub(crate) mod leader_queries {
    pub mod v1 {
        tonic::include_proto!("nillion.leader_queries.v1");

        pub mod pool_status {
            tonic::include_proto!("nillion.preprocessing.v1.element");
            tonic::include_proto!("nillion.leader_queries.v1.pool_status");
        }
    }
}

pub(crate) mod compute {
    pub mod v1 {
        tonic::include_proto!("nillion.compute.v1");

        pub mod invoke {
            tonic::include_proto!("nillion.compute.v1.invoke");
        }

        pub mod retrieve {
            tonic::include_proto!("nillion.compute.v1.retrieve");
        }

        pub mod stream {
            tonic::include_proto!("nillion.compute.v1.stream");
        }
    }
}

pub(crate) mod membership {
    pub mod v1 {
        tonic::include_proto!("nillion.auth.v1.public_key");
        tonic::include_proto!("nillion.membership.v1");

        pub mod cluster {
            tonic::include_proto!("nillion.membership.v1.cluster");
        }

        pub mod version {
            tonic::include_proto!("nillion.membership.v1.version");
        }
    }
}

pub(crate) mod payments {
    pub mod v1 {
        tonic::include_proto!("nillion.payments.v1");

        pub mod config {
            tonic::include_proto!("nillion.payments.v1.config");
        }

        pub mod quote {
            tonic::include_proto!("nillion.payments.v1.quote");
        }

        pub mod receipt {
            tonic::include_proto!("nillion.payments.v1.receipt");
        }

        pub mod balance {
            tonic::include_proto!("nillion.payments.v1.balance");
        }
    }
}

pub(crate) mod permissions {
    pub mod v1 {
        tonic::include_proto!("nillion.permissions.v1");

        pub mod overwrite {
            tonic::include_proto!("nillion.permissions.v1.overwrite");
        }

        pub mod permissions {
            tonic::include_proto!("nillion.permissions.v1.permissions");
        }

        pub mod retrieve {
            tonic::include_proto!("nillion.permissions.v1.retrieve");
        }

        pub mod update {
            tonic::include_proto!("nillion.permissions.v1.update");
        }
    }
}

pub(crate) mod preprocessing {
    pub mod v1 {
        tonic::include_proto!("nillion.preprocessing.v1");

        pub mod cleanup {
            tonic::include_proto!("nillion.preprocessing.v1.cleanup");
        }

        pub mod element {
            tonic::include_proto!("nillion.preprocessing.v1.element");
        }

        pub mod generate {
            tonic::include_proto!("nillion.preprocessing.v1.generate");
        }

        pub mod material {
            tonic::include_proto!("nillion.preprocessing.v1.material");
        }

        pub mod stream {
            tonic::include_proto!("nillion.preprocessing.v1.stream");
        }
    }
}

pub(crate) mod programs {
    pub mod v1 {
        tonic::include_proto!("nillion.programs.v1");

        pub mod store {
            tonic::include_proto!("nillion.programs.v1.store");
        }
    }
}

pub(crate) mod values {
    pub mod v1 {
        tonic::include_proto!("nillion.values.v1");

        pub mod delete {
            tonic::include_proto!("nillion.values.v1.delete");
        }

        pub mod retrieve {
            tonic::include_proto!("nillion.values.v1.retrieve");
        }

        pub mod store {
            tonic::include_proto!("nillion.values.v1.store");
        }

        #[allow(clippy::module_inception)]
        pub mod value {
            tonic::include_proto!("nillion.values.v1.value");
        }
    }
}
