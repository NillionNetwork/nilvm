#![allow(clippy::arithmetic_side_effects, clippy::panic, clippy::indexing_slicing)]

use super::{
    output::{EcdsaAuxInfo, EcdsaAuxInfoOutput},
    PregeneratedPrimesMode,
};
use crate::{
    simulator::symmetric::{InitializedProtocol, Protocol, SymmetricProtocolSimulator},
    threshold_ecdsa::auxiliary_information::EcdsaAuxInfoState,
};
use anyhow::{Error, Result};
use basic_types::PartyId;
use cggmp21::{key_share::Validate, rug::Integer};
use once_cell::sync::Lazy;
use rstest::rstest;

// Some hardcoded primes to hardcode the test. These are the primes generated in some test run,
// pasted here.
static PRIMES: Lazy<(Integer, Integer)> = Lazy::new(|| {
    let p = Integer::from_str_radix("1491384482818571944736148042575759250026964909986958734657493847083586570727423597703857074382468934077650769178411382382987368033753097228676924519651219903388689676418691727455463030420235028600472787207407079824752064135020222539389248600481776927172657145555720233563795737930658798284302155799349995007233713151518861275415249126071066658953556723161890972061305779678559139270588992799436727224879655163439934606552110992230706400959667770153309983572384863", 10).expect("invalid p prime");
    let q = Integer::from_str_radix("1849856642021601327689535942355561903873711121405925632576316941309744808658107232808587614701402269701882833650566836817465032408778854627095914864273656520615110574446626702643632234314556814084174347468452591945410366322776528543805630835928901598847605060003763444740096715776426238411607397895538259615811625992161916326429141334681995816640722873697524859690259817858189190679544627567696998158106319715893087043786242923004968121238303588771485732058599043", 10).expect("invalid q prime");
    (p, q)
});

struct EcdsaAuxInfoProtocol {
    eid: Vec<u8>,
}

impl EcdsaAuxInfoProtocol {
    fn new(eid: Vec<u8>) -> Self {
        Self { eid }
    }
}

impl Protocol for EcdsaAuxInfoProtocol {
    type State = EcdsaAuxInfoState;
    type PrepareOutput = EcdsaAuxInfoConfig;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        Ok(EcdsaAuxInfoConfig { eid: self.eid.clone(), parties: parties.to_vec() })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, anyhow::Error> {
        let mode = PregeneratedPrimesMode::Fixed { p: PRIMES.0.clone(), q: PRIMES.1.clone() };
        let (state, messages) = EcdsaAuxInfoState::new(config.eid.clone(), config.parties.clone(), party_id, mode)?;

        Ok(InitializedProtocol::new(state, messages))
    }
}

/// The internal configuration of a EcdsaAuxInfo protocol.
struct EcdsaAuxInfoConfig {
    eid: Vec<u8>,
    parties: Vec<PartyId>,
}

#[rstest]
fn end_to_end() {
    let max_rounds = 100;
    let network_size = 3;

    let eid = b"execution id, unique per protocol execution".to_vec();

    let protocol = EcdsaAuxInfoProtocol::new(eid);
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    for output in outputs {
        match output.output {
            EcdsaAuxInfoOutput::Success { element } => {
                let EcdsaAuxInfo { aux_info } = element;
                assert!(aux_info.is_valid().is_ok())
            }
            EcdsaAuxInfoOutput::Abort { reason } => {
                panic!("Aborted with reason: {:?}", reason);
            }
        }
    }
}
