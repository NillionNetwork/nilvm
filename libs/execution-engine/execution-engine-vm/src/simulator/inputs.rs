//! Program input utilities.

use std::{cell::RefCell, collections::HashMap, ops::DerefMut};

use crate::vm::instructions::Instruction;
use anyhow::{anyhow, Error, Ok};
use basic_types::PartyId;
use jit_compiler::Program;
use math_lib::modular::SafePrime;
use nada_compiler_backend::program_contract::{Input as ProgramContractInput, ProgramContract};
use nada_value::{
    clear::Clear,
    encrypted::{nada_values_clear_to_nada_values_encrypted, Encrypted},
    NadaInt, NadaType, NadaUint, NadaValue,
};
use num_bigint::RandBigInt;
use rand::{rngs::StdRng, Rng, SeedableRng};
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
use threshold_keypair::{
    generic_ec::curves::{Ed25519, Secp256k1},
    privatekey::ThresholdPrivateKey,
};

/// The inputs for a program.
#[derive(Default, Debug)]
pub struct ProgramInputs<T: SafePrime> {
    /// Inputs for each party.
    pub party_inputs: HashMap<PartyId, HashMap<String, NadaValue<Encrypted<T>>>>,
}

impl<T> ProgramInputs<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Create inputs for the given program.
    pub fn from_program<I: Instruction<T>>(
        program: &Program<I>,
        input_generator: &InputGenerator,
        secret_sharer: &ShamirSecretSharer<T>,
    ) -> Result<Self, Error> {
        // Type checking for each input
        for input in program.contract.inputs.iter() {
            Self::check_input_type(input, input_generator)?;
        }
        let inputs = Self::build_inputs(&program.contract, input_generator)?;
        let party_inputs = Self::build_party_inputs(inputs, secret_sharer)?;
        Ok(ProgramInputs { party_inputs })
    }

    /// Check whether the type of an input matches the expected type (coming from the generator).
    #[allow(dead_code)]
    fn check_input_type(input: &ProgramContractInput, generator: &InputGenerator) -> Result<(), Error> {
        let input_name = &input.name;
        let input_type = &input.ty;
        let var_type = generator.get_type(input_name);
        if let Some(var) = var_type {
            if var != *input_type {
                let msg = format!("type mismatch for input \"{input_name}\": was {var}, expected {input_type}");
                return Err(anyhow!(msg));
            }
        }
        Ok(())
    }

    fn build_inputs(
        contract: &ProgramContract,
        input_generator: &InputGenerator,
    ) -> Result<HashMap<String, NadaValue<Clear>>, Error> {
        let mut inputs = HashMap::new();
        for input in &contract.inputs {
            inputs.insert(input.name.clone(), input_generator.create(&input.name, input.ty.clone())?);
        }
        Ok(inputs)
    }

    #[allow(clippy::type_complexity)]
    fn build_party_inputs(
        inputs: HashMap<String, NadaValue<Clear>>,
        secret_sharer: &ShamirSecretSharer<T>,
    ) -> Result<HashMap<PartyId, HashMap<String, NadaValue<Encrypted<T>>>>, Error> {
        let mut encoded_inputs = HashMap::new();
        for (input_name, input) in inputs {
            encoded_inputs.insert(input_name, input);
        }
        let partyjar_inputs_encrypted =
            nada_values_clear_to_nada_values_encrypted(encoded_inputs, secret_sharer)?.into_elements().collect();
        Ok(partyjar_inputs_encrypted)
    }
}

/// A type capable of generating inputs.
#[derive(Debug, Clone)]
pub enum InputGenerator {
    /// Generate random secrets.
    Random,

    /// Generate PRNG-generated random secrets.
    RandomPrng(Box<RefCell<StdRng>>),

    /// Return pre-generated secrets for every input.
    Static(HashMap<String, NadaValue<Clear>>),
}

impl InputGenerator {
    /// Create a new random PRNG based input generator.
    pub fn new_random_prng(seed: [u8; 32]) -> Self {
        let rng = RefCell::new(StdRng::from_seed(seed));
        Self::RandomPrng(Box::new(rng))
    }

    /// Create an input for the given type.
    pub fn create(&self, name: &str, ty: NadaType) -> Result<NadaValue<Clear>, Error> {
        use InputGenerator::*;
        match self {
            Random => Self::new_random_value(&ty, &mut rand::thread_rng()),
            RandomPrng(rng) => {
                let mut rng = rng.borrow_mut();
                Self::new_random_value(&ty, rng.deref_mut())
            }
            Static(mappings) => {
                mappings.get(name).cloned().ok_or_else(|| anyhow!("mapping for value {name} not defined"))
            }
        }
    }

    fn new_random_value<R>(ty: &NadaType, rng: &mut R) -> Result<NadaValue<Clear>, Error>
    where
        R: Rng,
    {
        let bool_probability = 0.5;
        match ty {
            NadaType::Integer => Ok(NadaValue::new_integer(rng.gen_bigint(8))),
            NadaType::UnsignedInteger => Ok(NadaValue::new_unsigned_integer(rng.gen_biguint(8))),
            NadaType::Boolean => Ok(NadaValue::new_boolean(rng.gen_bool(bool_probability))),
            NadaType::SecretInteger => Ok(NadaValue::new_secret_integer(rng.gen_bigint(8))),
            NadaType::SecretUnsignedInteger => Ok(NadaValue::new_secret_unsigned_integer(rng.gen_biguint(8))),
            NadaType::SecretBoolean => Ok(NadaValue::new_secret_boolean(rng.gen_bool(bool_probability))),
            NadaType::SecretBlob => Err(anyhow!("'blobs' are not supported during computation")),
            NadaType::EcdsaDigestMessage => Err(anyhow!("Ecdsa digest messages are not supported during computation")),
            NadaType::EcdsaPrivateKey => Err(anyhow!("Ecdsa private keys are not supported during computation")),
            NadaType::EcdsaSignature => Err(anyhow!("Ecdsa signatures are not supported during computation")),
            NadaType::EcdsaPublicKey => Err(anyhow!("Ecdsa public keys are not supported during computation")),
            NadaType::StoreId => Err(anyhow!("Store ids are not supported during computation")),
            NadaType::EddsaPrivateKey => Err(anyhow!("Eddsa private keys are not supported during computation")),
            NadaType::EddsaPublicKey => Err(anyhow!("Eddsa public keys are not supported during computation")),
            NadaType::EddsaSignature => Err(anyhow!("Eddsa signatures are not supported during computation")),
            NadaType::EddsaMessage => Err(anyhow!("Eddsa messages are not supported during computation")),
            NadaType::Array { inner_type, size } => {
                let mut inner_values = Vec::new();
                for _ in 0..*size {
                    inner_values.push(Self::new_random_value(inner_type, rng)?);
                }
                Ok(NadaValue::new_array(inner_type.as_ref().clone(), inner_values)?)
            }
            NadaType::Tuple { left_type, right_type } => Ok(NadaValue::new_tuple(
                Self::new_random_value(left_type, rng)?,
                Self::new_random_value(right_type, rng)?,
            )?),
            NadaType::NTuple { types } => {
                let mut inner_values = Vec::new();
                for inner_type in types {
                    inner_values.push(Self::new_random_value(inner_type, rng)?);
                }
                Ok(NadaValue::new_n_tuple(inner_values)?)
            }
            NadaType::Object { types } => {
                let mut inner_values = Vec::new();
                for inner_type in types.values() {
                    inner_values.push(Self::new_random_value(inner_type, rng)?);
                }
                Ok(NadaValue::new_object(types.keys().cloned().zip(inner_values.into_iter()).collect())?)
            }
            NadaType::ShamirShareInteger | NadaType::ShamirShareUnsignedInteger | NadaType::ShamirShareBoolean => {
                Err(anyhow!("value can't be generated from {ty:?}"))
            }
        }
    }

    #[allow(dead_code)]
    fn get_type(&self, name: &str) -> Option<NadaType> {
        match self {
            InputGenerator::Random | InputGenerator::RandomPrng(_) => None,
            InputGenerator::Static(secrets) => secrets.get(name).cloned().map(|value| value.to_type()),
        }
    }
}

impl From<InputGenerator> for HashMap<String, NadaValue<Clear>> {
    fn from(generator: InputGenerator) -> Self {
        match generator {
            InputGenerator::Random | InputGenerator::RandomPrng(_) => HashMap::default(),
            InputGenerator::Static(inputs) => inputs,
        }
    }
}

/// A builder for a `InputGenerator::Static`.
#[derive(Default)]
pub struct StaticInputGeneratorBuilder {
    inputs: HashMap<String, NadaValue<Clear>>,
}

impl StaticInputGeneratorBuilder {
    /// Adds a new integer secret.
    pub fn add_secret_integer<S, T>(self, name: S, value: T) -> Self
    where
        S: Into<String>,
        NadaInt: From<T>,
    {
        self.add(name, NadaValue::new_secret_integer(value))
    }

    /// Adds a new unsigned integer secret.
    pub fn add_secret_unsigned_integer<S, T>(self, name: S, value: T) -> Self
    where
        S: Into<String>,
        NadaUint: From<T>,
    {
        self.add(name, NadaValue::new_secret_unsigned_integer(value))
    }

    /// Adds a new integer.
    pub fn add_integer<S, T>(self, name: S, value: T) -> Self
    where
        S: Into<String>,
        NadaInt: From<T>,
    {
        self.add(name, NadaValue::new_integer(value))
    }

    /// Adds a new unsigned integer.
    pub fn add_unsigned_integer<S, T>(self, name: S, value: T) -> Self
    where
        S: Into<String>,
        NadaUint: From<T>,
    {
        self.add(name, NadaValue::new_unsigned_integer(value))
    }

    /// Adds a new ecdsa digest message.
    pub fn add_ecdsa_digest_message<S>(self, name: S, value: [u8; 32]) -> Self
    where
        S: Into<String>,
    {
        self.add(name, NadaValue::new_ecdsa_digest_message(value))
    }

    /// Adds a new ecdsa digest message.
    pub fn add_ecdsa_private_key<S>(self, name: S, value: ThresholdPrivateKey<Secp256k1>) -> Self
    where
        S: Into<String>,
    {
        self.add(name, NadaValue::new_ecdsa_private_key(value))
    }

    /// Adds a new eddsa private key.
    pub fn add_eddsa_private_key<S>(self, name: S, value: ThresholdPrivateKey<Ed25519>) -> Self
    where
        S: Into<String>,
    {
        self.add(name, NadaValue::new_eddsa_private_key(value))
    }

    /// Adds a new eddsa message.
    pub fn add_eddsa_message<S>(self, name: S, value: Vec<u8>) -> Self
    where
        S: Into<String>,
    {
        self.add(name, NadaValue::new_eddsa_message(value))
    }

    /// Adds an input.
    pub fn add<S>(mut self, name: S, input: NadaValue<Clear>) -> Self
    where
        S: Into<String>,
    {
        self.inputs.insert(name.into(), input);
        self
    }

    /// Adds many inputs.
    pub fn add_all<S>(mut self, inputs: Vec<(S, NadaValue<Clear>)>) -> Self
    where
        S: Into<String>,
    {
        for (name, input) in inputs {
            self.inputs.insert(name.into(), input);
        }
        self
    }

    /// Adds many inputs.
    pub fn extend<S>(&mut self, inputs: HashMap<S, NadaValue<Clear>>)
    where
        S: Into<String>,
    {
        self.inputs.extend(inputs.into_iter().map(|(s, input)| (s.into(), input)));
    }

    /// Insert a secret
    pub fn insert<S>(&mut self, name: S, secret: NadaValue<Clear>)
    where
        S: Into<String>,
    {
        self.inputs.insert(name.into(), secret);
    }

    /// Builds the output `SecretGenerator`.
    pub fn build(self) -> InputGenerator {
        InputGenerator::Static(self.inputs)
    }
}
