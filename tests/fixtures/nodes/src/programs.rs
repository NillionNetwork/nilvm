use crate::nodes::{Nodes, UploadedPrograms};
use anyhow::Context;
use futures::future;
use once_cell::sync::Lazy;
use program_builder::{program_package, PackagePrograms};
use std::{collections::HashMap, time::Instant};
use tracing::info;

/// The programs that are uploaded as part of this fixture.
pub static PROGRAMS: Lazy<PackagePrograms> = Lazy::new(|| program_package!("default"));

const MAX_CLIENTS: usize = 10;

pub(crate) async fn upload_programs(nodes: &Nodes) -> anyhow::Result<UploadedPrograms> {
    let now = Instant::now();
    let mut clients = Vec::new();
    // Create up to this number of clients and reuse them in all uploads.
    for _ in 0..MAX_CLIENTS {
        let client = nodes.build_client().await;
        clients.push(client);
    }
    let mut futs = Vec::new();
    for ((program_name, metadata), client) in PROGRAMS.metadata.iter().zip(clients.iter().cycle()) {
        info!("Uploading program {program_name}");
        let mir = metadata.raw_mir();
        futs.push(client.store_program().name(program_name).program(mir).build()?.invoke());
    }
    let mut ids = HashMap::new();
    let results = future::join_all(futs).await;
    for (result, program_name) in results.into_iter().zip(PROGRAMS.metadata.keys()) {
        let program_id = result.context("failed to upload program")?;
        ids.insert(program_name.clone(), program_id);
    }
    let elapsed = now.elapsed();
    info!("Uploaded {} programs in {:?}", PROGRAMS.metadata.len(), elapsed);

    let namespace = UploadedPrograms(ids);

    info!("Uploaded programs JSON:\n{}", serde_json::to_string(&namespace)?);

    Ok(namespace)
}
