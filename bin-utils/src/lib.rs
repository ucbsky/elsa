use std::str::FromStr;

#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "server")]
pub mod server;
pub enum InputSize {
    U8,
    U32,
}

impl InputSize {
    pub const fn num_bits(&self) -> usize {
        match self {
            InputSize::U8 => 8,
            InputSize::U32 => 32,
        }
    }
}

impl FromStr for InputSize {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "8" => Ok(InputSize::U8),
            "32" => Ok(InputSize::U32),
            _ => Err(format!("Unsupported input size: {}", s)),
        }
    }
}
