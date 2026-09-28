import type { SdkWorkResourceData } from './sdk-work-resource-data';

export interface UpdateSandboxInstanceResponse {
  /** Numeric success result code; MUST be 0 on HTTP 2xx (API_SPEC.md section 15.3) */
  code: 0;
  data: unknown & SdkWorkResourceData;
  /** Server-owned request correlation id; clients must not supply this value */
  traceId: string;
}
