use std::time::Duration;

/// The limit covers the whole request, from connecting to the last byte read.
pub(crate) fn agent(limit_secs: u64) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(limit_secs)))
        .build()
        .into()
}

pub(crate) fn steam_agent() -> ureq::Agent {
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    AGENT.get_or_init(|| agent(120)).clone()
}

pub(crate) fn read_text(
    response: &mut ureq::http::Response<ureq::Body>,
    limit: u64,
) -> Result<String, ureq::Error> {
    response
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_string()
}

pub(crate) fn read_bytes(
    response: &mut ureq::http::Response<ureq::Body>,
    limit: u64,
) -> Result<Vec<u8>, ureq::Error> {
    response.body_mut().with_config().limit(limit).read_to_vec()
}

pub(crate) fn failed(e: ureq::Error) -> crate::base::error::Error {
    crate::base::error::Error::network(crate::said!("the download failed: {}", e))
}

pub(crate) fn steam_failed(e: impl std::fmt::Display) -> crate::base::error::Error {
    crate::base::error::Error::network(crate::said!("Steam could not be asked: {}", e))
}
