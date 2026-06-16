use core::fmt::Display;

pub struct HexU128(u128);

impl From<&str> for HexU128 {
    fn from(hex: &str) -> Self {
        Self(u128::from_str_radix(hex, 16).expect("should be valid base 16"))
    }
}

impl Display for HexU128 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}

impl<'de> serde::Deserialize<'de> for HexU128 {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = <&str>::deserialize(d)?;
        u128::from_str_radix(s, 16)
            .map(HexU128)
            .map_err(serde::de::Error::custom)
    }
}

impl PartialEq<u128> for HexU128 {
    fn eq(&self, other: &u128) -> bool {
        self.0 == *other
    }
}

impl HexU128 {
    pub fn to_bytea(&self) -> [u8; 16] {
        self.0.to_be_bytes()
    }
}
