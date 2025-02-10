use crate::{error::IntoEyre, program::Program, publish::publish_program, test::TestCase};
use eyre::{eyre, Result};

fn get_program_parties(program: &Program) -> Result<(Vec<String>, Vec<String>)> {
    let program_contract = &program.program.contract;
    let input_parties = program_contract.input_parties()?;
    let output_parties = program_contract.output_parties()?;
    let input_party_names = input_parties.into_iter().map(|input_party| input_party.name.clone()).collect();
    let output_party_names: Vec<String> =
        output_parties.into_iter().map(|output_party| output_party.name.clone()).collect();

    Ok((input_party_names, output_party_names))
}

pub async fn compute_test(network: &String, test: Box<dyn TestCase>) -> Result<()> {
    let (input_parties, output_parties) = get_program_parties(test.program())?;
    let inputs = test.inputs()?;
    let program_conf = &test.program().conf;
    // Publish program
    println!("Publishing: {}", program_conf.name);
    let (program_id, client) = publish_program(network, program_conf).await?;

    // Compute program
    println!("Computing: {program_id}");

    let user_id = client.user_id();

    let mut builder = client.invoke_compute().program_id(program_id).add_values(inputs);
    for party in input_parties {
        builder = builder.bind_input_party(party, user_id);
    }
    for party in output_parties {
        builder = builder.bind_output_party(party, [user_id]);
    }
    let compute_id = builder.build()?.invoke().await.into_eyre()?;
    let outputs = client
        .retrieve_compute_results()
        .compute_id(compute_id)
        .build()?
        .invoke()
        .await
        .into_eyre()?
        .map_err(|e| eyre!("{e:?}"))?;
    for (output_name, value) in outputs {
        println!("Output ({output_name}): {value:?}");
    }

    Ok(())
}
