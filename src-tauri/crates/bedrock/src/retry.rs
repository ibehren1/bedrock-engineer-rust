//! Throttling retry / region failover — port of `ConverseService.handleError` and
//! `tryAlternateRegion`, plus `getAlternateRegionOnThrottling` from `utils/awsUtils.ts`.
//!
//! State machine for one call (the SDK's own standard retries run inside each attempt, as the JS
//! SDK's did):
//!
//! ```text
//! attempt(primary region)
//!   ok                                  -> done
//!   error, not Throttling/ServiceUnavailable -> fail
//!   error, retries >= max_retries       -> fail with that error
//!   ThrottlingException + failover on + alternate region != current
//!       attempt(alternate)  ok -> done | error -> ignored
//!   sleep(delay); retries += 1; loop
//! ```
//!
//! Cancellation is checked around every attempt and during every sleep.

use crate::error::{Error, Result};
use std::collections::hash_map::RandomState;
use std::future::Future;
use std::hash::{BuildHasher, Hasher};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// `MAX_RETRIES = 30`, `RETRY_DELAY = 5000`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 30,
            delay: Duration::from_millis(5000),
        }
    }
}

/// Inputs to the region failover decision.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FailoverConfig {
    /// `bedrockSettings.enableRegionFailover`.
    pub enabled: bool,
    /// The configured client region.
    pub current_region: String,
    /// `bedrockSettings.availableFailoverRegions`.
    pub configured_regions: Vec<String>,
    /// The model's regions from the registry (`None` if the model is unknown).
    pub model_regions: Option<Vec<String>>,
}

/// `getAlternateRegionOnThrottling`: a random region, different from the current one, that the
/// model supports (restricted to the configured list when non-empty). Returns the current region
/// when there is no alternative. `pick(n)` must return an index in `0..n`.
pub fn alternate_region(
    current_region: &str,
    model_regions: Option<&[String]>,
    configured_regions: &[String],
    pick: impl FnOnce(usize) -> usize,
) -> String {
    let Some(model_regions) = model_regions else {
        return current_region.to_string();
    };
    let available: Vec<&String> = model_regions
        .iter()
        .filter(|r| configured_regions.is_empty() || configured_regions.contains(r))
        .filter(|r| r.as_str() != current_region)
        .collect();
    if available.is_empty() {
        return current_region.to_string();
    }
    let idx = pick(available.len()).min(available.len() - 1);
    available[idx].clone()
}

/// Process-random index in `0..n` (`Math.floor(Math.random() * n)`).
pub fn random_index(n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    let mut h = RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    (h.finish() % n as u64) as usize
}

impl FailoverConfig {
    /// `tryAlternateRegion`'s region choice: `None` when failover is off or no other region fits.
    pub fn pick_region(&self, pick: impl FnOnce(usize) -> usize) -> Option<String> {
        if !self.enabled {
            return None;
        }
        let region = alternate_region(
            &self.current_region,
            self.model_regions.as_deref(),
            &self.configured_regions,
            pick,
        );
        (region != self.current_region).then_some(region)
    }
}

async fn cancellable<T>(cancel: &CancellationToken, fut: impl Future<Output = T>) -> Result<T> {
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(Error::Cancelled),
        v = fut => Ok(v),
    }
}

/// Run `attempt` with the throttling retry / failover policy.
///
/// `attempt(None)` targets the configured region; `attempt(Some(region))` an alternate one.
pub async fn with_retries<T, F, Fut>(
    policy: RetryPolicy,
    failover: &FailoverConfig,
    cancel: &CancellationToken,
    mut pick: impl FnMut(usize) -> usize,
    mut attempt: F,
) -> Result<T>
where
    F: FnMut(Option<String>) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut retries: u32 = 0;
    loop {
        let err = match cancellable(cancel, attempt(None)).await? {
            Ok(v) => return Ok(v),
            Err(e) => e,
        };
        let Error::Service(service_err) = &err else {
            return Err(err);
        };
        if !service_err.is_retryable() {
            tracing::error!(name = %service_err.name, message = %service_err.message, "Bedrock request failed");
            return Err(err);
        }
        tracing::warn!(retry = retries, name = %service_err.name, "{} occurred - retrying", service_err.name);
        if retries >= policy.max_retries {
            tracing::error!(
                max_retries = policy.max_retries,
                "Maximum retries reached for Bedrock API request"
            );
            return Err(err);
        }
        if service_err.name == "ThrottlingException" {
            if let Some(region) = failover.pick_region(&mut pick) {
                tracing::info!(current = %failover.current_region, alternate = %region, "Switching to alternate region due to throttling");
                match cancellable(cancel, attempt(Some(region.clone()))).await? {
                    Ok(v) => return Ok(v),
                    Err(e) => {
                        tracing::error!(region = %region, error = %e, "Error in alternate region request");
                    }
                }
            }
        }
        cancellable(cancel, tokio::time::sleep(policy.delay)).await?;
        retries += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ServiceError;
    use std::sync::{Arc, Mutex};

    fn regions(rs: &[&str]) -> Vec<String> {
        rs.iter().map(|s| s.to_string()).collect()
    }

    fn throttle() -> Error {
        Error::Service(ServiceError::new("ThrottlingException", "slow down"))
    }

    fn unavailable() -> Error {
        Error::Service(ServiceError::new("ServiceUnavailableException", "down"))
    }

    fn fast(max: u32) -> RetryPolicy {
        RetryPolicy {
            max_retries: max,
            delay: Duration::from_millis(1),
        }
    }

    #[test]
    fn default_policy_matches_ts_constants() {
        let p = RetryPolicy::default();
        assert_eq!(p.max_retries, 30);
        assert_eq!(p.delay, Duration::from_secs(5));
    }

    #[test]
    fn alternate_region_filters_and_excludes_current() {
        let model = regions(&["us-east-1", "us-west-2", "eu-west-1"]);
        // no configured list -> any model region except current
        let r = alternate_region("us-west-2", Some(&model), &[], |n| {
            assert_eq!(n, 2);
            1
        });
        assert_eq!(r, "eu-west-1");
        // configured list intersects
        let r = alternate_region(
            "us-west-2",
            Some(&model),
            &regions(&["us-east-1", "ap-south-1"]),
            |n| {
                assert_eq!(n, 1);
                0
            },
        );
        assert_eq!(r, "us-east-1");
        // nothing left -> current
        assert_eq!(
            alternate_region("us-west-2", Some(&regions(&["us-west-2"])), &[], |_| 0),
            "us-west-2"
        );
        // unknown model -> current
        assert_eq!(alternate_region("us-west-2", None, &[], |_| 0), "us-west-2");
    }

    #[test]
    fn random_index_in_range() {
        for n in 1..20 {
            assert!(random_index(n) < n);
        }
        assert_eq!(random_index(0), 0);
    }

    #[test]
    fn pick_region_respects_enabled_flag() {
        let mut f = FailoverConfig {
            enabled: false,
            current_region: "us-west-2".into(),
            configured_regions: vec![],
            model_regions: Some(regions(&["us-west-2", "us-east-1"])),
        };
        assert_eq!(f.pick_region(|_| 0), None);
        f.enabled = true;
        assert_eq!(f.pick_region(|_| 0), Some("us-east-1".into()));
        f.model_regions = Some(regions(&["us-west-2"]));
        assert_eq!(f.pick_region(|_| 0), None);
    }

    type Calls = Arc<Mutex<Vec<Option<String>>>>;

    /// Scripted attempt: pops the next result for each call and records the region argument.
    fn scripted(
        script: Vec<Result<&'static str>>,
    ) -> (
        Calls,
        impl FnMut(Option<String>) -> std::future::Ready<Result<&'static str>>,
    ) {
        let calls: Calls = Arc::default();
        let script = Arc::new(Mutex::new(script.into_iter()));
        let c = calls.clone();
        (calls, move |region| {
            c.lock().unwrap().push(region);
            std::future::ready(
                script
                    .lock()
                    .unwrap()
                    .next()
                    .expect("unexpected extra attempt"),
            )
        })
    }

    #[tokio::test]
    async fn success_first_try() {
        let (calls, attempt) = scripted(vec![Ok("ok")]);
        let out = with_retries(
            fast(3),
            &FailoverConfig::default(),
            &CancellationToken::new(),
            |_| 0,
            attempt,
        )
        .await;
        assert_eq!(out.unwrap(), "ok");
        assert_eq!(calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn non_retryable_error_fails_immediately() {
        let (calls, attempt) = scripted(vec![Err(Error::Service(ServiceError::new(
            "ValidationException",
            "bad",
        )))]);
        let err = with_retries(
            fast(3),
            &FailoverConfig::default(),
            &CancellationToken::new(),
            |_| 0,
            attempt,
        )
        .await
        .unwrap_err();
        assert_eq!(err.service_name(), Some("ValidationException"));
        assert_eq!(calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn local_errors_are_not_retried() {
        let (calls, attempt) = scripted(vec![Err(Error::InvalidRequest("x".into()))]);
        let err = with_retries(
            fast(3),
            &FailoverConfig::default(),
            &CancellationToken::new(),
            |_| 0,
            attempt,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, Error::InvalidRequest(_)));
        assert_eq!(calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn throttling_retries_then_succeeds() {
        let (calls, attempt) = scripted(vec![Err(throttle()), Err(unavailable()), Ok("ok")]);
        let out = with_retries(
            fast(5),
            &FailoverConfig::default(),
            &CancellationToken::new(),
            |_| 0,
            attempt,
        )
        .await;
        assert_eq!(out.unwrap(), "ok");
        assert_eq!(*calls.lock().unwrap(), vec![None, None, None]);
    }

    #[tokio::test]
    async fn gives_up_after_max_retries() {
        // max 2 -> initial + 2 retries = 3 attempts, the third error is returned
        let (calls, attempt) = scripted(vec![Err(throttle()), Err(throttle()), Err(unavailable())]);
        let err = with_retries(
            fast(2),
            &FailoverConfig::default(),
            &CancellationToken::new(),
            |_| 0,
            attempt,
        )
        .await
        .unwrap_err();
        assert_eq!(err.service_name(), Some("ServiceUnavailableException"));
        assert_eq!(calls.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn throttling_fails_over_to_alternate_region() {
        let failover = FailoverConfig {
            enabled: true,
            current_region: "us-west-2".into(),
            configured_regions: vec![],
            model_regions: Some(regions(&["us-west-2", "us-east-1"])),
        };
        let (calls, attempt) = scripted(vec![Err(throttle()), Ok("from-alt")]);
        let out = with_retries(
            fast(5),
            &failover,
            &CancellationToken::new(),
            |_| 0,
            attempt,
        )
        .await;
        assert_eq!(out.unwrap(), "from-alt");
        assert_eq!(
            *calls.lock().unwrap(),
            vec![None, Some("us-east-1".to_string())]
        );
    }

    #[tokio::test]
    async fn failed_alternate_falls_back_to_normal_retry() {
        let failover = FailoverConfig {
            enabled: true,
            current_region: "us-west-2".into(),
            configured_regions: vec![],
            model_regions: Some(regions(&["us-west-2", "us-east-1"])),
        };
        let (calls, attempt) = scripted(vec![Err(throttle()), Err(throttle()), Ok("primary")]);
        let out = with_retries(
            fast(5),
            &failover,
            &CancellationToken::new(),
            |_| 0,
            attempt,
        )
        .await;
        assert_eq!(out.unwrap(), "primary");
        assert_eq!(
            *calls.lock().unwrap(),
            vec![None, Some("us-east-1".to_string()), None]
        );
    }

    #[tokio::test]
    async fn service_unavailable_does_not_fail_over() {
        let failover = FailoverConfig {
            enabled: true,
            current_region: "us-west-2".into(),
            configured_regions: vec![],
            model_regions: Some(regions(&["us-west-2", "us-east-1"])),
        };
        let (calls, attempt) = scripted(vec![Err(unavailable()), Ok("ok")]);
        with_retries(
            fast(5),
            &failover,
            &CancellationToken::new(),
            |_| 0,
            attempt,
        )
        .await
        .unwrap();
        assert_eq!(*calls.lock().unwrap(), vec![None, None]);
    }

    #[tokio::test]
    async fn max_retries_checked_before_failover() {
        // retries >= MAX -> throw without trying the alternate region
        let failover = FailoverConfig {
            enabled: true,
            current_region: "us-west-2".into(),
            configured_regions: vec![],
            model_regions: Some(regions(&["us-west-2", "us-east-1"])),
        };
        let (calls, attempt) = scripted(vec![Err(throttle())]);
        let err = with_retries(
            fast(0),
            &failover,
            &CancellationToken::new(),
            |_| 0,
            attempt,
        )
        .await
        .unwrap_err();
        assert_eq!(err.service_name(), Some("ThrottlingException"));
        assert_eq!(*calls.lock().unwrap(), vec![None]);
    }

    #[tokio::test(start_paused = true)]
    async fn cancel_during_backoff() {
        let cancel = CancellationToken::new();
        let c2 = cancel.clone();
        let (calls, attempt) = scripted(vec![Err(throttle())]);
        let policy = RetryPolicy {
            max_retries: 30,
            delay: Duration::from_secs(5),
        };
        let handle = tokio::spawn(async move {
            with_retries(policy, &FailoverConfig::default(), &c2, |_| 0, attempt).await
        });
        tokio::time::sleep(Duration::from_secs(1)).await;
        cancel.cancel();
        let out = handle.await.unwrap();
        assert!(matches!(out, Err(Error::Cancelled)));
        assert_eq!(calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn cancel_before_attempt() {
        let cancel = CancellationToken::new();
        cancel.cancel();
        let (calls, attempt) = scripted(vec![Ok("never")]);
        let out = with_retries(fast(1), &FailoverConfig::default(), &cancel, |_| 0, attempt).await;
        assert!(matches!(out, Err(Error::Cancelled)));
        // the future was created but never polled to completion
        assert!(calls.lock().unwrap().len() <= 1);
    }
}
