macro_rules! build_client {
    (
        $client_ty:ty, $http_client:ident, $credentials:ident, $api_base_url:ident, $retry_pol:expr
    ) => {{
        let mut builder = <$client_ty>::builder();
        builder
            .http_client($http_client.clone())
            .retry_policy($retry_pol)
            .credentials($credentials.clone());
        if let Some(api_base_url) = $api_base_url {
            builder.api_base_url(api_base_url);
        }
        builder.build()?
    }};
}

#[derive(Debug, Copy, Clone)]
pub struct NeedsBackupInfo {
    pub needs_backup: bool,
    pub solution_exists: bool,
}
