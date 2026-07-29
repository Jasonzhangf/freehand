use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde_json::Value;

const ONLINE_PROVENANCE_KEY_ID: &str = "freehand.openminis-online.v1";
const ONLINE_PROVENANCE_PUBLIC_KEY_HEX: &str =
    "a3f38bc624b925f8e99ff9c9f494ba301a25992eda38da0e729c0bfbffd5ad2e";

const ONLINE_GATES: &[&str] = &[
    "webui_online_e2e",
    "android_device_e2e",
    "openminis_ui_legacy_online_no_touch",
];

pub(super) fn verify_online_report_provenance(gate_id: &str, report: &Value) -> Result<(), String> {
    if !ONLINE_GATES.contains(&gate_id) {
        return Ok(());
    }
    if report.get("provenance_key_id").and_then(Value::as_str) != Some(ONLINE_PROVENANCE_KEY_ID) {
        return Err(format!(
            "OpenMinis UI online report for `{gate_id}` lacks trusted provenance_key_id"
        ));
    }
    let signature_hex = report
        .get("provenance_signature")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "OpenMinis UI online report for `{gate_id}` lacks external provenance_signature"
            )
        })?;
    let signature_bytes = decode_array::<64>(signature_hex, "provenance_signature")?;
    provenance_verifying_key()?
        .verify(
            &canonical_report_payload(report)?,
            &Signature::from_bytes(&signature_bytes),
        )
        .map_err(|_| {
            format!(
                "OpenMinis UI online report for `{gate_id}` has invalid external provenance_signature"
            )
        })
}

#[cfg(not(test))]
fn provenance_verifying_key() -> Result<VerifyingKey, String> {
    let public_key = decode_array::<32>(ONLINE_PROVENANCE_PUBLIC_KEY_HEX, "provenance public key")?;
    VerifyingKey::from_bytes(&public_key)
        .map_err(|err| format!("invalid OpenMinis UI provenance public key: {err}"))
}

#[cfg(test)]
fn provenance_verifying_key() -> Result<VerifyingKey, String> {
    use ed25519_dalek::SigningKey;

    Ok(SigningKey::from_bytes(&[7_u8; 32]).verifying_key())
}

pub(crate) fn canonical_report_payload(report: &Value) -> Result<Vec<u8>, String> {
    let mut payload = report.clone();
    payload
        .as_object_mut()
        .ok_or_else(|| "OpenMinis UI verifier report must be an object".to_owned())?
        .remove("provenance_signature");
    serde_json::to_vec(&payload)
        .map_err(|err| format!("encode OpenMinis UI provenance payload: {err}"))
}

fn decode_array<const N: usize>(value: &str, field: &str) -> Result<[u8; N], String> {
    let bytes = hex::decode(value).map_err(|err| format!("invalid {field} hex: {err}"))?;
    bytes
        .try_into()
        .map_err(|_| format!("{field} must contain exactly {N} bytes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_provenance_public_key_is_valid() {
        let bytes = decode_array::<32>(
            ONLINE_PROVENANCE_PUBLIC_KEY_HEX,
            "production provenance public key",
        )
        .expect("decode production public key");
        VerifyingKey::from_bytes(&bytes).expect("production public key must be valid Ed25519");
    }
}
