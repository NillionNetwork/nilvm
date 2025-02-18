use crate::args::CommandOutputFormat;
use anyhow::Result;
use erased_serde::serialize_trait_object;
use serde::Serialize;
use std::any::Any;

pub trait SerializeAsAny: erased_serde::Serialize + Any {}
impl<T: erased_serde::Serialize + Any> SerializeAsAny for T {}

serialize_trait_object!(SerializeAsAny);

#[derive(Serialize)]
pub struct NoOutput;

#[derive(Serialize)]
pub struct ErrorOutput {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cause: Option<Vec<String>>,
}

pub fn serialize_output(format: &CommandOutputFormat, data: &dyn SerializeAsAny) -> Result<String> {
    let mut buf = Vec::new();
    match format {
        CommandOutputFormat::Json => {
            let mut serializer = serde_json::Serializer::pretty(&mut buf);
            let mut erased_serializer = <dyn erased_serde::Serializer>::erase(&mut serializer);
            data.erased_serialize(&mut erased_serializer)?;
        }
        CommandOutputFormat::Yaml => {
            let mut serializer = serde_yaml::Serializer::new(&mut buf);
            let mut erased_serializer = <dyn erased_serde::Serializer>::erase(&mut serializer);
            data.erased_serialize(&mut erased_serializer)?;
        }
    }
    Ok(String::from_utf8(buf)?)
}

pub fn serialize_error(format: &CommandOutputFormat, e: &anyhow::Error) -> String {
    let main_error = e.to_string();
    let mut causes: Vec<String> = e.chain().skip(1).map(|cause| cause.to_string()).collect();

    let error = if causes.is_empty() {
        if let Some((first_part, second_part)) = main_error.split_once(':') {
            causes.push(second_part.trim().to_string());
            first_part.trim().to_string()
        } else {
            main_error
        }
    } else {
        main_error
    };

    let cause = (!causes.is_empty()).then_some(causes);

    let error_response = ErrorOutput { error, cause };
    serialize_output(format, &error_response).unwrap_or_else(|_| format!("{e:#}"))
}
