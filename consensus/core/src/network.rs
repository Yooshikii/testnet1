//!
//! # Network Types
//!
//! This module implements [`NetworkType`] (such as `mainnet`, `testnet`, and `simnet`)
//! and [`NetworkId`].
//!

#![allow(non_snake_case)]

use borsh::{BorshDeserialize, BorshSerialize};
use vecno_addresses::Prefix;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Debug, Display, Formatter};
use std::ops::Deref;
use std::str::FromStr;
use wasm_bindgen::convert::TryFromJsValue;
use wasm_bindgen::prelude::*;
use workflow_wasm::prelude::*;

#[derive(thiserror::Error, PartialEq, Eq, Debug, Clone)]
pub enum NetworkTypeError {
    #[error("Invalid network type: {0}")]
    InvalidNetworkType(String),
}

/// @category Consensus
#[derive(Clone, Copy, Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[serde(rename_all = "lowercase")]
#[wasm_bindgen]
pub enum NetworkType {
    Mainnet,
    Testnet,
    Simnet,
}

impl NetworkType {
    pub fn default_rpc_port(&self) -> u16 {
        match self {
            NetworkType::Mainnet => 7110,
            NetworkType::Testnet => 7210,
            NetworkType::Simnet => 7310,
        }
    }

    pub fn default_borsh_rpc_port(&self) -> u16 {
        match self {
            NetworkType::Mainnet => 8110,
            NetworkType::Testnet => 8210,
            NetworkType::Simnet => 8310,
        }
    }

    pub fn default_json_rpc_port(&self) -> u16 {
        match self {
            NetworkType::Mainnet => 9110,
            NetworkType::Testnet => 9210,
            NetworkType::Simnet => 9310,
        }
    }

    pub fn iter() -> impl Iterator<Item = Self> {
        static NETWORK_TYPES: [NetworkType; 3] =
            [NetworkType::Mainnet, NetworkType::Testnet, NetworkType::Simnet];
        NETWORK_TYPES.iter().copied()
    }
}

impl TryFrom<Prefix> for NetworkType {
    type Error = NetworkTypeError;
    fn try_from(prefix: Prefix) -> Result<Self, Self::Error> {
        match prefix {
            Prefix::Mainnet => Ok(NetworkType::Mainnet),
            Prefix::Testnet => Ok(NetworkType::Testnet),
            Prefix::Simnet => Ok(NetworkType::Simnet),
            #[allow(unreachable_patterns)]
            #[cfg(test)]
            _ => Err(NetworkTypeError::InvalidNetworkType(prefix.to_string())),
        }
    }
}

impl From<NetworkType> for Prefix {
    fn from(network_type: NetworkType) -> Self {
        match network_type {
            NetworkType::Mainnet => Prefix::Mainnet,
            NetworkType::Testnet => Prefix::Testnet,
            NetworkType::Simnet => Prefix::Simnet,
        }
    }
}

impl FromStr for NetworkType {
    type Err = NetworkTypeError;
    fn from_str(network_type: &str) -> Result<Self, Self::Err> {
        match network_type.to_lowercase().as_str() {
            "mainnet" => Ok(NetworkType::Mainnet),
            "testnet" => Ok(NetworkType::Testnet),
            "simnet" => Ok(NetworkType::Simnet),
            _ => Err(NetworkTypeError::InvalidNetworkType(network_type.to_string())),
        }
    }
}

impl Display for NetworkType {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            NetworkType::Mainnet => "mainnet",
            NetworkType::Testnet => "testnet",
            NetworkType::Simnet => "simnet",
        };
        f.write_str(s)
    }
}

impl TryFrom<&NetworkTypeT> for NetworkType {
    type Error = NetworkTypeError;
    fn try_from(value: &NetworkTypeT) -> Result<Self, Self::Error> {
        if let Ok(network_id) = NetworkId::try_cast_from(value) {
            Ok(network_id.network_type())
        } else if let Some(network_type) = value.as_string() {
            Self::from_str(&network_type)
        } else if let Ok(network_type) = NetworkType::try_from_js_value(JsValue::from(value)) {
            Ok(network_type)
        } else {
            Err(NetworkTypeError::InvalidNetworkType(format!("{value:?}")))
        }
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "Network", typescript_type = "NetworkType | NetworkId | string")]
    #[derive(Debug)]
    pub type NetworkTypeT;
}

impl TryFrom<&NetworkTypeT> for Prefix {
    type Error = NetworkIdError;
    fn try_from(value: &NetworkTypeT) -> Result<Self, Self::Error> {
        Ok(NetworkType::try_from(value)?.into())
    }
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum NetworkIdError {
    #[error("Invalid network name prefix: {0}. The expected prefix is 'vecno'.")]
    InvalidPrefix(String),

    #[error(transparent)]
    InvalidNetworkType(#[from] NetworkTypeError),

    #[error("Invalid network id: '{0}'")]
    InvalidNetworkId(String),

    #[error(transparent)]
    Wasm(#[from] workflow_wasm::error::Error),
}

impl From<NetworkIdError> for JsValue {
    fn from(err: NetworkIdError) -> Self {
        JsValue::from_str(&err.to_string())
    }
}

/// NetworkId is a unique identifier for a network instance.
/// It consists of a single network type.
///
/// @category Consensus
#[derive(Clone, Copy, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq, Hash, Ord, PartialOrd, CastFromJs)]
#[wasm_bindgen(inspectable)]
pub struct NetworkId {
    #[wasm_bindgen(js_name = "type")]
    pub network_type: NetworkType,
}

impl NetworkId {
    /// Create a new `NetworkId` from a `NetworkType`.
    pub const fn new(network_type: NetworkType) -> Self {
        Self { network_type }
    }

    pub fn network_type(&self) -> NetworkType {
        self.network_type
    }

    pub fn is_mainnet(&self) -> bool {
        self.network_type == NetworkType::Mainnet
    }

    /// P2P port is now fixed per network type.
    pub fn default_p2p_port(&self) -> u16 {
        match self.network_type {
            NetworkType::Mainnet => 7111,
            NetworkType::Testnet => 7211,
            NetworkType::Simnet => 7311,
        }
    }

    pub fn iter() -> impl Iterator<Item = Self> {
        static NETWORK_IDS: [NetworkId; 3] = [
            NetworkId::new(NetworkType::Mainnet),
            NetworkId::new(NetworkType::Testnet),
            NetworkId::new(NetworkType::Simnet),
        ];
        NETWORK_IDS.iter().copied()
    }

    /// Returns a textual description of the network prefixed with `vecno-`.
    pub fn to_prefixed(&self) -> String {
        format!("vecno-{}", self.network_type)
    }

    pub fn from_prefixed(prefixed: &str) -> Result<Self, NetworkIdError> {
        if let Some(stripped) = prefixed.strip_prefix("vecno-") {
            Self::from_str(stripped)
        } else {
            Err(NetworkIdError::InvalidPrefix(prefixed.to_string()))
        }
    }
}

impl Deref for NetworkId {
    type Target = NetworkType;

    fn deref(&self) -> &Self::Target {
        &self.network_type
    }
}

impl TryFrom<NetworkType> for NetworkId {
    type Error = NetworkIdError;
    fn try_from(value: NetworkType) -> Result<Self, Self::Error> {
        Ok(Self::new(value))
    }
}

impl From<NetworkId> for Prefix {
    fn from(net: NetworkId) -> Self {
        net.network_type.into()
    }
}

impl From<NetworkId> for NetworkType {
    fn from(net: NetworkId) -> Self {
        net.network_type
    }
}

impl FromStr for NetworkId {
    type Err = NetworkIdError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let network_type = NetworkType::from_str(s)?;
        Ok(Self { network_type })
    }
}

impl Display for NetworkId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.network_type)
    }
}

impl Serialize for NetworkId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

struct NetworkIdVisitor;

impl<'de> de::Visitor<'de> for NetworkIdVisitor {
    type Value = NetworkId;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a string containing a network type (mainnet|testnet|simnet)")
    }

    fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        NetworkId::from_str(value).map_err(|err| de::Error::custom(err.to_string()))
    }
}

impl<'de> Deserialize<'de> for NetworkId {
    fn deserialize<D>(deserializer: D) -> Result<NetworkId, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(NetworkIdVisitor)
    }
}

#[wasm_bindgen]
impl NetworkId {
    #[wasm_bindgen(constructor)]
    pub fn ctor(value: &JsValue) -> Result<NetworkId, NetworkIdError> {
        Ok(NetworkId::try_cast_from(value)?.into_owned())
    }

    #[wasm_bindgen(getter, js_name = "id")]
    pub fn js_id(&self) -> String {
        self.to_string()
    }

    #[wasm_bindgen(js_name = "toString")]
    pub fn js_to_string(&self) -> String {
        self.to_string()
    }

    #[wasm_bindgen(js_name = "addressPrefix")]
    pub fn js_address_prefix(&self) -> String {
        Prefix::from(self.network_type).to_string()
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "NetworkId | string")]
    pub type NetworkIdT;
}

impl TryFrom<&JsValue> for NetworkId {
    type Error = NetworkIdError;
    fn try_from(value: &JsValue) -> Result<Self, Self::Error> {
        Self::try_owned_from(value)
    }
}

impl TryFrom<JsValue> for NetworkId {
    type Error = NetworkIdError;
    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        Self::try_owned_from(value)
    }
}

impl TryCastFromJs for NetworkId {
    type Error = NetworkIdError;
    fn try_cast_from<'a, R>(value: &'a R) -> Result<Cast<'a, Self>, Self::Error>
    where
        R: AsRef<JsValue> + 'a,
    {
        Self::resolve(value, || {
            if let Some(s) = value.as_ref().as_string() {
                Ok(NetworkId::from_str(&s)?)
            } else {
                Err(NetworkIdError::InvalidNetworkId(format!("{:?}", value.as_ref())))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_id_parse_roundtrip() {
        for nt in NetworkType::iter() {
            let ni = NetworkId::new(nt);
            assert_eq!(nt, *NetworkId::from_str(&ni.to_string()).unwrap());
            assert_eq!(ni, NetworkId::from_str(&ni.to_string()).unwrap());
        }
    }

    #[test]
    fn test_network_id_parse() {
        struct Test {
            name: &'static str,
            expr: &'static str,
            expected: Result<NetworkId, NetworkIdError>,
        }

        let tests = vec![
            Test {
                name: "Valid mainnet",
                expr: "mainnet",
                expected: Ok(NetworkId::new(NetworkType::Mainnet)),
            },
            Test {
                name: "Valid testnet",
                expr: "testnet",
                expected: Ok(NetworkId::new(NetworkType::Testnet)),
            },
            Test {
                name: "Valid simnet",
                expr: "simnet",
                expected: Ok(NetworkId::new(NetworkType::Simnet)),
            },
            Test {
                name: "Missing network",
                expr: "",
                expected: Err(NetworkTypeError::InvalidNetworkType("".to_string()).into()),
            },
            Test {
                name: "Invalid network",
                expr: "gamenet",
                expected: Err(NetworkTypeError::InvalidNetworkType("gamenet".to_string()).into()),
            },
        ];

        for test in tests {
            let Test { name, expr, expected } = test;
            match NetworkId::from_str(expr) {
                Ok(nid) => assert_eq!(nid, expected.unwrap(), "{}: unexpected result", name),
                Err(err) => assert_eq!(
                    err.to_string(),
                    expected.unwrap_err().to_string(),
                    "{}: unexpected error",
                    name
                ),
            }
        }
    }
}