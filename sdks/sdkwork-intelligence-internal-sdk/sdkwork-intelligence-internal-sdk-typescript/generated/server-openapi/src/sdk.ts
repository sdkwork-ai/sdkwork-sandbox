import { HttpClient, createHttpClient } from './http/client';
import type { SdkworkCustomConfig } from './types/common';

import { SandboxInstancesApi, createSandboxInstancesApi } from './api/sandbox-instances';

export class SdkworkIntelligenceInternalClient {
  private httpClient: HttpClient;

  public readonly sandboxInstances: SandboxInstancesApi;

  constructor(config: SdkworkCustomConfig) {
    this.httpClient = createHttpClient(config);
    this.sandboxInstances = createSandboxInstancesApi(this.httpClient);
  }

  setApiKey(apiKey: string): this {
    this.httpClient.setApiKey(apiKey);
    return this;
  }
  get http(): HttpClient {
    return this.httpClient;
  }
}

export function createClient(config: SdkworkCustomConfig): SdkworkIntelligenceInternalClient {
  return new SdkworkIntelligenceInternalClient(config);
}

export default SdkworkIntelligenceInternalClient;
