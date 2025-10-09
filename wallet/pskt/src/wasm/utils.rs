use vecno_consensus_core::constants::*;
use vecno_consensus_core::network::NetworkType;
use separator::{separated_float, separated_int, separated_uint_with_output, Separatable};

#[inline]
pub fn veni_to_vecno(veni: u64) -> f64 {
    veni as f64 / VENI_PER_VECNO as f64
}

#[inline]
pub fn vecno_to_veni(vecno: f64) -> u64 {
    (vecno * VENI_PER_VECNO as f64) as u64
}

#[inline]
pub fn veni_to_vecno_string(veni: u64) -> String {
    veni_to_vecno(veni).separated_string()
}

#[inline]
pub fn veni_to_vecno_string_with_trailing_zeroes(veni: u64) -> String {
    separated_float!(format!("{:.8}", veni_to_vecno(veni)))
}

pub fn vecno_suffix(network_type: &NetworkType) -> &'static str {
    match network_type {
        NetworkType::Mainnet => "VE",
        NetworkType::Testnet => "TVE",
        NetworkType::Simnet => "SVE",
    }
}

#[inline]
pub fn veni_to_vecno_string_with_suffix(veni: u64, network_type: &NetworkType) -> String {
    let ve = veni_to_vecno_string(veni);
    let suffix = vecno_suffix(network_type);
    format!("{ve} {suffix}")
}
