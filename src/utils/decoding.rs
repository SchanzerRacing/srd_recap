use anyhow::{Context, Result, bail};
use mcapdecode_core::{MessageDecoder, TopicDecoder, Value, DataTypeDef, FieldDefs};
use mcapdecode_ros2msg::Ros2MsgDecoder;
use std::collections::HashMap;

pub struct Decoder {
    decoder: Box<dyn TopicDecoder>,
    fields: HashMap<String, Vec<usize>>,
}

impl Decoder {
    pub fn new(schema_name: &str, schema_data: &[u8]) -> Result<Self> {
        let decoder = Ros2MsgDecoder::new().build_topic_decoder(schema_name, schema_data)?;

        let mut fields = HashMap::new();
        collect_fields(decoder.field_defs(), "", &[], &mut fields);

        Ok(Self { decoder, fields })
    }

    pub fn decode(&self, data: &[u8]) -> Result<DecodedMessage> {
        let value = self.decoder.decode(data)?;

        Ok(DecodedMessage {
            value,
            fields: self.fields.clone(),
        })
    }
}

pub struct DecodedMessage {
    value: Value,
    fields: HashMap<String, Vec<usize>>,
}

macro_rules! scalar_getters {
    ($($method:ident => $variant:ident : $ty:ty),* $(,)?) => {
        $(
            #[allow(dead_code)]
            pub fn $method(&self, name: &str) -> Result<$ty> {
                match self.field(name)? {
                    Value::$variant(value) => Ok(*value),
                    _ => bail!("Expected field {} to be {}", name, stringify!($ty)),
                }
            }
        )*
    };
}

fn collect_fields(
    definitions: &FieldDefs,
    prefix: &str,
    parent_indices: &[usize],
    fields: &mut HashMap<String, Vec<usize>>,
) {
    for (index, definition) in definitions.iter().enumerate() {
        let name = if prefix.is_empty() {
            definition.name.clone()
        } else {
            format!("{prefix}.{}", definition.name)
        };

        let mut indices = parent_indices.to_vec();
        indices.push(index);

        fields.insert(name.clone(), indices.clone());

        if let DataTypeDef::Struct(children) = &definition.element.data_type {
            collect_fields(children, &name, &indices, fields);
        }
    }
}

impl DecodedMessage {
    pub fn field(&self, name: &str) -> Result<&Value> {
        let indices = self
            .fields
            .get(name)
            .with_context(|| format!("Schema has no field named {name}"))?;

        let mut current = &self.value;

        for &index in indices {
            let Value::Struct(values) = current else {
                bail!("Expected a struct while accessing {name}");
            };

            current = values
                .get(index)
                .with_context(|| format!("Message is missing field {name}"))?;
        }

        Ok(current)
    }

    scalar_getters! {
        bool => Bool: bool,
        i8   => I8:   i8,
        i16  => I16:  i16,
        i32  => I32:  i32,
        i64  => I64:  i64,
        u8   => U8:   u8,
        u16  => U16:  u16,
        u32  => U32:  u32,
        u64  => U64:  u64,
        f32  => F32:  f32,
        f64  => F64:  f64,
    }

    #[allow(dead_code)]
    pub fn str(&self, name: &str) -> Result<&str> {
        match self.field(name)? {
            Value::String(value) => Ok(value.as_ref()),
            _ => bail!("Expected field {name} to be a string"),
        }
    }
}
