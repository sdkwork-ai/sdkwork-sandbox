export interface SdkWorkApiResponse {
  /** Numeric success result code; MUST be 0 on HTTP 2xx (API_SPEC.md section 15.3) */
  code: 0;
  /** Operation-specific payload; typed per operation through allOf */
  data: unknown;
  /** Server-owned request correlation id; clients must not supply this value */
  traceId: string;
}
