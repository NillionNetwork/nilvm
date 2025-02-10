use crate::error::IntoEyre;
use eyre::eyre;
use nada_value::{clear::Clear, json::nada_values_from_untyped_json_partial, NadaType, NadaValue};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

pub fn json_to_values(
    json_inputs: Option<String>,
    inputs_types: HashMap<String, NadaType>,
) -> eyre::Result<HashMap<String, NadaValue<Clear>>> {
    if let Some(json_inputs) = json_inputs {
        let json_values = serde_json::from_str(&json_inputs).map_err(|e| eyre!("{:?}", Box::new(e)))?;
        nada_values_from_untyped_json_partial(inputs_types, json_values).into_eyre()
    } else {
        Ok(HashMap::new())
    }
}

pub fn values_to_json(values: HashMap<String, NadaValue<Clear>>) -> eyre::Result<BTreeMap<String, Value>> {
    let mut json_values = BTreeMap::new();
    for (name, value) in values {
        json_values.insert(name, value.to_json_value().map_err(|e| eyre!(Box::new(e)))?);
    }
    Ok(json_values)
}
