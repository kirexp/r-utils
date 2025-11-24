
#[cfg(feature = "sd")]
pub mod sd {
    use serde::{Deserialize, Serialize};
    use crate::base::GenericResult;

    pub fn safe_deserializer<TResponse> (data: String) -> GenericResult<TResponse>
    where
        TResponse: Serialize + for<'de> Deserialize<'de>,
    {

        use serde_path_to_error::deserialize;
        use serde_json::Deserializer;

        let de = &mut Deserializer::from_str(&data);
        let result: TResponse = deserialize(de)?;

        Ok(result)
    }

    pub fn safe_serializer<TResponse> (data: TResponse) -> GenericResult<String>
    where
        TResponse: Serialize + for<'de> Deserialize<'de>,
    {
        let serialized = serde_json::to_string(&data)?;
        Ok(serialized)
    }
}
