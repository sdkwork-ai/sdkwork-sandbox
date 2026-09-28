from .http_client import HttpClient, SdkConfig
from .api.sandbox_instances import SandboxInstancesApi


class SdkworkIntelligenceInternalClient:
    """sdkwork-intelligence-internal-api SDK Client."""

    def __init__(self, config: SdkConfig):
        self._client = HttpClient(config)
        self.sandbox_instances: SandboxInstancesApi

        # Initialize API modules
        self.sandbox_instances = SandboxInstancesApi(self._client)

    def set_api_key(self, api_key: str) -> 'SdkworkIntelligenceInternalClient':
        """Set API key for authentication."""
        self._client.set_api_key(api_key)
        return self

    def set_auth_token(self, token: str) -> 'SdkworkIntelligenceInternalClient':
        """Set auth token for authentication."""
        self._client.set_auth_token(token)
        return self

    def set_access_token(self, token: str) -> 'SdkworkIntelligenceInternalClient':
        """Set access token for authentication."""
        self._client.set_access_token(token)
        return self

    def set_header(self, key: str, value: str) -> 'SdkworkIntelligenceInternalClient':
        """Set custom header."""
        self._client.set_header(key, value)
        return self

    @property
    def http(self) -> HttpClient:
        """Get the underlying HTTP client."""
        return self._client


def create_client(config: SdkConfig) -> SdkworkIntelligenceInternalClient:
    """Create a new SDK client instance."""
    return SdkworkIntelligenceInternalClient(config)
