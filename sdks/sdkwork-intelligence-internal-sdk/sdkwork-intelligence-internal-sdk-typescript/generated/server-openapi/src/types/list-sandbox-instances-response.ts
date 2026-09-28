import type { SdkWorkPageData } from './sdk-work-page-data';

export interface ListSandboxInstancesResponse {
  /** Numeric success result code; MUST be 0 on HTTP 2xx (API_SPEC.md section 15.3) */
  code: 0;
  data: unknown & SdkWorkPageData;
  /** Server-owned request correlation id; clients must not supply this value */
  traceId: string;
}
