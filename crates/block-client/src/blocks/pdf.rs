use base64::{engine::general_purpose::STANDARD, Engine as _};
use block::{Block, NoHistory};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

const PDF_MAGIC: &[u8] = b"%PDF-";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Pdf {
    source_name: String,
    #[serde(
        serialize_with = "serialize_data",
        deserialize_with = "deserialize_data"
    )]
    data: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum PdfOperation {
    Replace { pdf: Pdf },
}

#[derive(Deserialize)]
struct PdfData {
    source_name: String,
    #[serde(deserialize_with = "deserialize_data")]
    data: Vec<u8>,
}

impl Pdf {
    pub fn new(source_name: impl Into<String>, data: Vec<u8>) -> Result<Self, String> {
        if !data.starts_with(PDF_MAGIC) {
            return Err("data is not a PDF file".into());
        }
        Ok(Self {
            source_name: source_name.into(),
            data,
        })
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl<'de> Deserialize<'de> for Pdf {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = PdfData::deserialize(deserializer)?;
        Self::new(data.source_name, data.data).map_err(D::Error::custom)
    }
}

impl Block for Pdf {
    type Operation = PdfOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7064_662d_626c_6f63_6b2d_7479_7065_2d01);

    fn apply_operation(pdf: &mut Self, operation: &Self::Operation) {
        match operation {
            PdfOperation::Replace { pdf: replacement } => *pdf = replacement.clone(),
        }
    }

    fn implicit_name(&self) -> Option<String> {
        let name = self.source_name.trim();
        (!name.is_empty()).then(|| name.to_owned())
    }
}

fn serialize_data<S>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&STANDARD.encode(data))
}

fn deserialize_data<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let encoded = String::deserialize(deserializer)?;
    STANDARD.decode(encoded).map_err(D::Error::custom)
}

#[cfg(test)]
mod tests;
